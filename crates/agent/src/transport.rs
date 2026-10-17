//! Mutually approved, end-to-end encrypted session streams.
//!
//! The caller supplies an already-connected reliable byte stream. Network
//! discovery/listener management are deliberately separate from this protocol.

use crate::peer_store::TrustedPeer;
use crate::protocol::{DeviceId, PROTOCOL_VERSION};
use crate::trust::{
    DeviceIdentity, PairingApproval, PairingTranscript, SignatureProof, TrustError,
};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use hkdf::Hkdf;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::future::Future;
use std::io;
use std::time::Duration;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::time::timeout;
use x25519_dalek::{EphemeralSecret, PublicKey as X25519PublicKey};

const MAX_HANDSHAKE_MESSAGE: usize = 4096;
pub const MAX_ENCRYPTED_MESSAGE: usize = 1024 * 1024;
const HANDSHAKE_IO_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HandshakeRole {
    Initiator,
    Responder,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionChallenge {
    pub local_device: DeviceId,
    pub peer_device: DeviceId,
    pub verification_code: String,
}

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("transport I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("peer handshake message is invalid")]
    InvalidHandshake,
    #[error("peer uses an incompatible protocol version")]
    ProtocolVersionMismatch,
    #[error("peer rejected the pairing challenge")]
    PairingRejected,
    #[error("peer identity does not match the approved device")]
    IdentityMismatch,
    #[error("peer signature is invalid")]
    InvalidSignature,
    #[error("peer supplied a non-contributory ephemeral key")]
    InvalidEphemeralKey,
    #[error("key derivation failed")]
    KeyDerivation,
    #[error("handshake message exceeds the protocol limit")]
    HandshakeMessageTooLarge,
    #[error("encrypted message exceeds the protocol limit")]
    EncryptedMessageTooLarge,
    #[error("encrypted message authentication failed")]
    AuthenticationFailed,
    #[error("message sequence number is exhausted")]
    SequenceExhausted,
    #[error("handshake I/O timed out")]
    HandshakeTimeout,
    #[error("pairing verification failed: {0}")]
    Trust(#[from] TrustError),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Hello {
    protocol_version: u16,
    role: HandshakeRole,
    device_id: DeviceId,
    signing_public_key: [u8; 32],
    nonce: [u8; 32],
    ephemeral_public_key: [u8; 32],
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum HandshakeMessage {
    Hello(Hello),
    Decision { approved: bool },
    Proof(SignatureProof),
}

/// Symmetric outputs are ordered by protocol role. Each endpoint's send key is
/// the other endpoint's receive key. Key bytes are erased when this value drops.
pub struct SessionKeys {
    send_key: [u8; 32],
    receive_key: [u8; 32],
}

impl Drop for SessionKeys {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.send_key);
        zeroize::Zeroize::zeroize(&mut self.receive_key);
    }
}

pub struct PairedTransport<S> {
    stream: S,
    keys: SessionKeys,
    peer_device: DeviceId,
    approval: Option<PairingApproval>,
}

impl<S> PairedTransport<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    pub fn peer_device(&self) -> &DeviceId {
        &self.peer_device
    }

    pub fn approval(&self) -> Option<&PairingApproval> {
        self.approval.as_ref()
    }

    pub fn into_encrypted(
        self,
    ) -> (
        EncryptedSender<WriteHalf<S>>,
        EncryptedReceiver<ReadHalf<S>>,
        Option<PairingApproval>,
    ) {
        let Self {
            stream,
            keys,
            approval,
            ..
        } = self;
        let send_key = keys.send_key;
        let receive_key = keys.receive_key;
        let (reader, writer) = tokio::io::split(stream);
        (
            EncryptedSender::new(writer, send_key),
            EncryptedReceiver::new(reader, receive_key),
            approval,
        )
    }
}

/// Performs one pairing handshake over a reliable stream.
///
/// Both endpoints exchange fresh signing identities, nonces, and one-use X25519
/// public keys. The callback must display the same six-digit code locally and
/// return true only after this user explicitly approves it. Signatures bind the
/// complete transcript including both ephemeral keys before directional keys are
/// derived. The resulting stream is end-to-end encrypted independently of its
/// underlying transport.
pub async fn pair_over_stream<S, F, Fut>(
    mut stream: S,
    identity: &DeviceIdentity,
    role: HandshakeRole,
    confirm: F,
) -> Result<PairedTransport<S>, TransportError>
where
    S: AsyncRead + AsyncWrite + Unpin,
    F: FnOnce(SessionChallenge) -> Fut,
    Fut: Future<Output = bool>,
{
    let (hello, ephemeral_secret) = make_hello(identity, role);
    write_handshake(&mut stream, &HandshakeMessage::Hello(hello.clone())).await?;
    let peer_hello = match read_handshake(&mut stream).await? {
        HandshakeMessage::Hello(hello) => hello,
        _ => return Err(TransportError::InvalidHandshake),
    };
    validate_hello(&hello, &peer_hello)?;

    let transcript = make_transcript(&hello, &peer_hello, role);
    let local_device = hello.device_id.clone();
    let peer_device = peer_hello.device_id.clone();
    let challenge = SessionChallenge {
        local_device: local_device.clone(),
        peer_device: peer_device.clone(),
        verification_code: transcript.displayed_code.clone(),
    };
    let local_approved = confirm(challenge.clone()).await;
    write_handshake(
        &mut stream,
        &HandshakeMessage::Decision {
            approved: local_approved,
        },
    )
    .await?;
    let peer_approved = match read_handshake(&mut stream).await? {
        HandshakeMessage::Decision { approved } => approved,
        _ => return Err(TransportError::InvalidHandshake),
    };
    if !local_approved || !peer_approved {
        return Err(TransportError::PairingRejected);
    }

    let mut approval = PairingApproval::new(transcript.clone(), local_device, peer_device.clone())?;
    let local_proof = approval.approve_local(identity)?;
    write_handshake(&mut stream, &HandshakeMessage::Proof(local_proof)).await?;
    let peer_proof = match read_handshake(&mut stream).await? {
        HandshakeMessage::Proof(proof) => proof,
        _ => return Err(TransportError::InvalidHandshake),
    };
    approval
        .record_peer_approval(peer_proof)
        .map_err(|_| TransportError::InvalidSignature)?;

    let peer_public = X25519PublicKey::from(peer_hello.ephemeral_public_key);
    let shared_secret = ephemeral_secret.diffie_hellman(&peer_public);
    if !shared_secret.was_contributory() {
        return Err(TransportError::InvalidEphemeralKey);
    }
    let mut secret_bytes = *shared_secret.as_bytes();
    let keys = derive_session_keys(&secret_bytes, &transcript, role)?;
    zeroize::Zeroize::zeroize(&mut secret_bytes);

    Ok(PairedTransport {
        stream,
        keys,
        peer_device: peer_hello.device_id,
        approval: Some(approval),
    })
}

/// Authenticates a returning peer against the public key stored after the original
/// two-sided approval. Fresh signatures and ephemeral DH keys prevent replay.
pub async fn connect_trusted_over_stream<S>(
    mut stream: S,
    identity: &DeviceIdentity,
    role: HandshakeRole,
    trusted_peer: &TrustedPeer,
) -> Result<PairedTransport<S>, TransportError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (hello, ephemeral_secret) = make_hello(identity, role);
    write_handshake(&mut stream, &HandshakeMessage::Hello(hello.clone())).await?;
    let peer_hello = match read_handshake(&mut stream).await? {
        HandshakeMessage::Hello(hello) => hello,
        _ => return Err(TransportError::InvalidHandshake),
    };
    validate_hello(&hello, &peer_hello)?;
    if peer_hello.device_id != trusted_peer.device_id
        || peer_hello.signing_public_key != trusted_peer.public_key
    {
        return Err(TransportError::IdentityMismatch);
    }

    let transcript = make_transcript(&hello, &peer_hello, role);
    let local_proof = identity.prove_bytes(transcript.signing_bytes());
    write_handshake(&mut stream, &HandshakeMessage::Proof(local_proof)).await?;
    let peer_proof = match read_handshake(&mut stream).await? {
        HandshakeMessage::Proof(proof) => proof,
        _ => return Err(TransportError::InvalidHandshake),
    };
    DeviceIdentity::verify(&trusted_peer.device_id, &transcript, &peer_proof)
        .map_err(|_| TransportError::InvalidSignature)?;

    let peer_public = X25519PublicKey::from(peer_hello.ephemeral_public_key);
    let shared_secret = ephemeral_secret.diffie_hellman(&peer_public);
    if !shared_secret.was_contributory() {
        return Err(TransportError::InvalidEphemeralKey);
    }
    let mut secret_bytes = *shared_secret.as_bytes();
    let keys = derive_session_keys(&secret_bytes, &transcript, role)?;
    zeroize::Zeroize::zeroize(&mut secret_bytes);
    Ok(PairedTransport {
        stream,
        keys,
        peer_device: peer_hello.device_id,
        approval: None,
    })
}

fn make_hello(identity: &DeviceIdentity, role: HandshakeRole) -> (Hello, EphemeralSecret) {
    let ephemeral_secret = EphemeralSecret::random_from_rng(OsRng);
    let ephemeral_public_key = X25519PublicKey::from(&ephemeral_secret).to_bytes();
    let mut nonce = [0u8; 32];
    OsRng.fill_bytes(&mut nonce);
    (
        Hello {
            protocol_version: PROTOCOL_VERSION,
            role,
            device_id: identity.device_id(),
            signing_public_key: identity.public_key(),
            nonce,
            ephemeral_public_key,
        },
        ephemeral_secret,
    )
}

fn validate_hello(local: &Hello, peer: &Hello) -> Result<(), TransportError> {
    if peer.protocol_version != PROTOCOL_VERSION {
        return Err(TransportError::ProtocolVersionMismatch);
    }
    if peer.role == local.role
        || peer.device_id == local.device_id
        || DeviceId::from_public_key(&peer.signing_public_key) != peer.device_id
    {
        return Err(TransportError::IdentityMismatch);
    }
    Ok(())
}

fn make_transcript(local: &Hello, peer: &Hello, local_role: HandshakeRole) -> PairingTranscript {
    let (initiator, responder) = match local_role {
        HandshakeRole::Initiator => (local, peer),
        HandshakeRole::Responder => (peer, local),
    };
    PairingTranscript::from_exchange(
        initiator.device_id.clone(),
        responder.device_id.clone(),
        initiator.nonce,
        responder.nonce,
        initiator.ephemeral_public_key,
        responder.ephemeral_public_key,
    )
}

fn derive_session_keys(
    shared_secret: &[u8; 32],
    transcript: &PairingTranscript,
    role: HandshakeRole,
) -> Result<SessionKeys, TransportError> {
    let transcript_bytes = transcript.signing_bytes();
    let transcript_hash = Sha256::digest(&transcript_bytes);
    let hkdf = Hkdf::<Sha256>::new(Some(&transcript_hash), shared_secret);
    let mut initiator_to_responder = [0u8; 32];
    let mut responder_to_initiator = [0u8; 32];
    hkdf.expand(
        b"unified-workspace-v1 initiator to responder",
        &mut initiator_to_responder,
    )
    .map_err(|_| TransportError::KeyDerivation)?;
    hkdf.expand(
        b"unified-workspace-v1 responder to initiator",
        &mut responder_to_initiator,
    )
    .map_err(|_| TransportError::KeyDerivation)?;
    let (send_key, receive_key) = match role {
        HandshakeRole::Initiator => (initiator_to_responder, responder_to_initiator),
        HandshakeRole::Responder => (responder_to_initiator, initiator_to_responder),
    };
    Ok(SessionKeys {
        send_key,
        receive_key,
    })
}

async fn write_handshake<S: AsyncWrite + Unpin>(
    stream: &mut S,
    message: &HandshakeMessage,
) -> Result<(), TransportError> {
    let payload = serde_json::to_vec(message).map_err(|_| TransportError::InvalidHandshake)?;
    if payload.len() > MAX_HANDSHAKE_MESSAGE {
        return Err(TransportError::HandshakeMessageTooLarge);
    }
    timeout(HANDSHAKE_IO_TIMEOUT, async {
        stream.write_u32(payload.len() as u32).await?;
        stream.write_all(&payload).await?;
        stream.flush().await
    })
    .await
    .map_err(|_| TransportError::HandshakeTimeout)??;
    Ok(())
}

async fn read_handshake<S: AsyncRead + Unpin>(
    stream: &mut S,
) -> Result<HandshakeMessage, TransportError> {
    let payload = timeout(HANDSHAKE_IO_TIMEOUT, async {
        let size = stream.read_u32().await? as usize;
        if size == 0 || size > MAX_HANDSHAKE_MESSAGE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid handshake length",
            ));
        }
        let mut payload = vec![0u8; size];
        stream.read_exact(&mut payload).await?;
        Ok::<_, io::Error>(payload)
    })
    .await
    .map_err(|_| TransportError::HandshakeTimeout)??;
    serde_json::from_slice(&payload).map_err(|_| TransportError::InvalidHandshake)
}

pub struct EncryptedSender<W> {
    writer: W,
    cipher: ChaCha20Poly1305,
    sequence: u64,
}

impl<W: AsyncWrite + Unpin> EncryptedSender<W> {
    fn new(writer: W, mut key: [u8; 32]) -> Self {
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
        zeroize::Zeroize::zeroize(&mut key);
        Self {
            writer,
            cipher,
            sequence: 0,
        }
    }

    pub async fn send(&mut self, plaintext: &[u8]) -> Result<(), TransportError> {
        if plaintext.len() > MAX_ENCRYPTED_MESSAGE {
            return Err(TransportError::EncryptedMessageTooLarge);
        }
        let sequence = self.sequence;
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or(TransportError::SequenceExhausted)?;
        let nonce_bytes = message_nonce(sequence);
        let aad = sequence.to_be_bytes();
        let ciphertext = self
            .cipher
            .encrypt(
                Nonce::from_slice(&nonce_bytes),
                Payload {
                    msg: plaintext,
                    aad: &aad,
                },
            )
            .map_err(|_| TransportError::AuthenticationFailed)?;
        self.writer.write_u32(ciphertext.len() as u32).await?;
        self.writer.write_all(&ciphertext).await?;
        self.writer.flush().await?;
        Ok(())
    }
}

pub struct EncryptedReceiver<R> {
    reader: R,
    cipher: ChaCha20Poly1305,
    sequence: u64,
}

impl<R: AsyncRead + Unpin> EncryptedReceiver<R> {
    fn new(reader: R, mut key: [u8; 32]) -> Self {
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
        zeroize::Zeroize::zeroize(&mut key);
        Self {
            reader,
            cipher,
            sequence: 0,
        }
    }

    pub async fn receive(&mut self) -> Result<Vec<u8>, TransportError> {
        let size = self.reader.read_u32().await? as usize;
        if !(16..=MAX_ENCRYPTED_MESSAGE + 16).contains(&size) {
            return Err(TransportError::EncryptedMessageTooLarge);
        }
        let mut ciphertext = vec![0u8; size];
        self.reader.read_exact(&mut ciphertext).await?;
        let sequence = self.sequence;
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or(TransportError::SequenceExhausted)?;
        let nonce_bytes = message_nonce(sequence);
        let aad = sequence.to_be_bytes();
        self.cipher
            .decrypt(
                Nonce::from_slice(&nonce_bytes),
                Payload {
                    msg: ciphertext.as_slice(),
                    aad: &aad,
                },
            )
            .map_err(|_| TransportError::AuthenticationFailed)
    }
}

fn message_nonce(sequence: u64) -> [u8; 12] {
    let mut nonce = [0u8; 12];
    nonce[4..].copy_from_slice(&sequence.to_be_bytes());
    nonce
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trust::DeviceIdentity;
    use std::sync::{Arc, Mutex};
    use tokio::io::duplex;

    #[tokio::test]
    async fn two_sides_compare_code_authenticate_and_exchange_encrypted_data() {
        let (initiator_identity, responder_identity) =
            (DeviceIdentity::generate(), DeviceIdentity::generate());
        let (initiator_stream, responder_stream) = duplex(32 * 1024);
        let initiator_code = Arc::new(Mutex::new(None));
        let responder_code = Arc::new(Mutex::new(None));
        let initiator_code_copy = Arc::clone(&initiator_code);
        let responder_code_copy = Arc::clone(&responder_code);
        let (initiator, responder) = tokio::join!(
            pair_over_stream(
                initiator_stream,
                &initiator_identity,
                HandshakeRole::Initiator,
                move |challenge| {
                    *initiator_code_copy.lock().unwrap() =
                        Some(challenge.verification_code.clone());
                    async move { challenge.verification_code.len() == 6 }
                }
            ),
            pair_over_stream(
                responder_stream,
                &responder_identity,
                HandshakeRole::Responder,
                move |challenge| {
                    *responder_code_copy.lock().unwrap() =
                        Some(challenge.verification_code.clone());
                    async move { challenge.verification_code.len() == 6 }
                }
            )
        );
        let initiator = initiator.unwrap();
        let responder = responder.unwrap();
        assert_eq!(initiator.peer_device(), &responder_identity.device_id());
        assert_eq!(responder.peer_device(), &initiator_identity.device_id());
        assert!(initiator.approval().unwrap().is_mutually_approved());
        assert!(responder.approval().unwrap().is_mutually_approved());
        assert_eq!(
            initiator_code.lock().unwrap().as_ref(),
            responder_code.lock().unwrap().as_ref()
        );
        assert_eq!(
            initiator.approval().unwrap().approved_peer_public_key(),
            Some(responder_identity.public_key())
        );

        let (mut initiator_sender, mut initiator_receiver, _) = initiator.into_encrypted();
        let (mut responder_sender, mut responder_receiver, _) = responder.into_encrypted();
        let (sent, received) = tokio::join!(
            initiator_sender.send(b"private workspace control frame"),
            responder_receiver.receive()
        );
        sent.unwrap();
        assert_eq!(received.unwrap(), b"private workspace control frame");
        let (sent, received) = tokio::join!(
            responder_sender.send(b"authenticated reverse direction"),
            initiator_receiver.receive()
        );
        sent.unwrap();
        assert_eq!(received.unwrap(), b"authenticated reverse direction");
    }

    #[tokio::test]
    async fn trusted_peer_reconnect_uses_pinned_keys_without_pairing_code() {
        let (initiator_identity, responder_identity) =
            (DeviceIdentity::generate(), DeviceIdentity::generate());
        let initiator_peer = TrustedPeer {
            device_id: responder_identity.device_id(),
            public_key: responder_identity.public_key(),
        };
        let responder_peer = TrustedPeer {
            device_id: initiator_identity.device_id(),
            public_key: initiator_identity.public_key(),
        };
        let (initiator_stream, responder_stream) = duplex(32 * 1024);
        let (initiator, responder) = tokio::join!(
            connect_trusted_over_stream(
                initiator_stream,
                &initiator_identity,
                HandshakeRole::Initiator,
                &initiator_peer,
            ),
            connect_trusted_over_stream(
                responder_stream,
                &responder_identity,
                HandshakeRole::Responder,
                &responder_peer,
            )
        );
        let initiator = initiator.unwrap();
        let responder = responder.unwrap();
        assert!(initiator.approval().is_none());
        assert!(responder.approval().is_none());
        let (mut initiator_sender, _, _) = initiator.into_encrypted();
        let (_, mut responder_receiver, _) = responder.into_encrypted();
        let (sent, received) = tokio::join!(
            initiator_sender.send(b"reconnected under the stored key pin"),
            responder_receiver.receive()
        );
        sent.unwrap();
        assert_eq!(received.unwrap(), b"reconnected under the stored key pin");
    }

    #[tokio::test]
    async fn trusted_peer_rejects_a_changed_signing_key() {
        let left_identity = DeviceIdentity::generate();
        let right_identity = DeviceIdentity::generate();
        let unrelated_identity = DeviceIdentity::generate();
        let left_peer = TrustedPeer {
            device_id: unrelated_identity.device_id(),
            public_key: unrelated_identity.public_key(),
        };
        let right_peer = TrustedPeer {
            device_id: left_identity.device_id(),
            public_key: left_identity.public_key(),
        };
        let (left_stream, right_stream) = duplex(8192);
        let (left, right) = tokio::join!(
            connect_trusted_over_stream(
                left_stream,
                &left_identity,
                HandshakeRole::Initiator,
                &left_peer,
            ),
            connect_trusted_over_stream(
                right_stream,
                &right_identity,
                HandshakeRole::Responder,
                &right_peer,
            )
        );
        assert!(matches!(left, Err(TransportError::IdentityMismatch)));
        assert!(right.is_err());
    }

    #[tokio::test]
    async fn rejection_on_either_side_aborts_pairing() {
        let left_identity = DeviceIdentity::generate();
        let right_identity = DeviceIdentity::generate();
        let (left_stream, right_stream) = duplex(8192);
        let (left, right) = tokio::join!(
            pair_over_stream(
                left_stream,
                &left_identity,
                HandshakeRole::Initiator,
                |_| async { true }
            ),
            pair_over_stream(
                right_stream,
                &right_identity,
                HandshakeRole::Responder,
                |_| async { false }
            )
        );
        assert!(matches!(left, Err(TransportError::PairingRejected)));
        assert!(matches!(right, Err(TransportError::PairingRejected)));
    }

    #[tokio::test]
    async fn mismatched_roles_are_rejected_without_pairing() {
        let left_identity = DeviceIdentity::generate();
        let right_identity = DeviceIdentity::generate();
        let (left_stream, right_stream) = duplex(8192);
        let (left, right) = tokio::join!(
            pair_over_stream(
                left_stream,
                &left_identity,
                HandshakeRole::Initiator,
                |_| async { true }
            ),
            pair_over_stream(
                right_stream,
                &right_identity,
                HandshakeRole::Initiator,
                |_| async { true }
            )
        );
        assert!(matches!(left, Err(TransportError::IdentityMismatch)));
        assert!(matches!(right, Err(TransportError::IdentityMismatch)));
    }

    #[tokio::test]
    async fn encrypted_receiver_rejects_wrong_directional_key() {
        let (left, right) = duplex(8192);
        let mut sender = EncryptedSender::new(tokio::io::split(left).1, [1; 32]);
        let mut receiver = EncryptedReceiver::new(tokio::io::split(right).0, [2; 32]);
        let (sent, received) = tokio::join!(sender.send(b"secret"), receiver.receive());
        sent.unwrap();
        assert!(matches!(
            received,
            Err(TransportError::AuthenticationFailed)
        ));
    }
}
