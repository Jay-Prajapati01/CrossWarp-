use crate::protocol::{DeviceId, PROTOCOL_VERSION};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Public transcript bound to both identities, fresh nonces, and ephemeral keys.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingTranscript {
    pub initiator: DeviceId,
    pub responder: DeviceId,
    pub initiator_nonce: [u8; 32],
    pub responder_nonce: [u8; 32],
    pub initiator_ephemeral: [u8; 32],
    pub responder_ephemeral: [u8; 32],
    pub displayed_code: String,
}

impl PairingTranscript {
    pub fn create(initiator: DeviceId, responder: DeviceId) -> Self {
        let mut initiator_nonce = [0u8; 32];
        let mut responder_nonce = [0u8; 32];
        let mut initiator_ephemeral = [0u8; 32];
        let mut responder_ephemeral = [0u8; 32];
        OsRng.fill_bytes(&mut initiator_nonce);
        OsRng.fill_bytes(&mut responder_nonce);
        OsRng.fill_bytes(&mut initiator_ephemeral);
        OsRng.fill_bytes(&mut responder_ephemeral);
        let mut transcript = Self {
            initiator,
            responder,
            initiator_nonce,
            responder_nonce,
            initiator_ephemeral,
            responder_ephemeral,
            displayed_code: String::new(),
        };
        transcript.displayed_code = transcript.derive_display_code();
        transcript
    }

    pub fn from_exchange(
        initiator: DeviceId,
        responder: DeviceId,
        initiator_nonce: [u8; 32],
        responder_nonce: [u8; 32],
        initiator_ephemeral: [u8; 32],
        responder_ephemeral: [u8; 32],
    ) -> Self {
        let mut transcript = Self {
            initiator,
            responder,
            initiator_nonce,
            responder_nonce,
            initiator_ephemeral,
            responder_ephemeral,
            displayed_code: String::new(),
        };
        transcript.displayed_code = transcript.derive_display_code();
        transcript
    }

    fn derive_display_code(&self) -> String {
        let mut bytes = Vec::with_capacity(128);
        bytes.extend_from_slice(b"unified-workspace-pairing-code-v1");
        append_field(&mut bytes, self.initiator.0.as_bytes());
        append_field(&mut bytes, self.responder.0.as_bytes());
        bytes.extend_from_slice(&self.initiator_nonce);
        bytes.extend_from_slice(&self.responder_nonce);
        bytes.extend_from_slice(&self.initiator_ephemeral);
        bytes.extend_from_slice(&self.responder_ephemeral);
        let digest = Sha256::digest(bytes);
        let number =
            u32::from_be_bytes(digest[..4].try_into().expect("fixed digest length")) % 1_000_000;
        format!("{number:06}")
    }

    pub fn has_valid_display_code(&self) -> bool {
        self.displayed_code == self.derive_display_code()
    }

    /// Stable, length-delimited bytes for signatures; no JSON/key ordering assumptions.
    pub fn signing_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(160);
        bytes.extend_from_slice(&PROTOCOL_VERSION.to_be_bytes());
        append_field(&mut bytes, self.initiator.0.as_bytes());
        append_field(&mut bytes, self.responder.0.as_bytes());
        bytes.extend_from_slice(&self.initiator_nonce);
        bytes.extend_from_slice(&self.responder_nonce);
        append_field(&mut bytes, self.displayed_code.as_bytes());
        bytes.extend_from_slice(&self.initiator_ephemeral);
        bytes.extend_from_slice(&self.responder_ephemeral);
        bytes
    }
}

fn append_field(target: &mut Vec<u8>, field: &[u8]) {
    target.extend_from_slice(&(field.len() as u32).to_be_bytes());
    target.extend_from_slice(field);
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SignatureProof {
    pub device_id: DeviceId,
    pub public_key: [u8; 32],
    pub signature: Vec<u8>,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum TrustError {
    #[error("device identity does not match the expected identity")]
    IdentityMismatch,
    #[error("public key is malformed")]
    InvalidPublicKey,
    #[error("signature is malformed or invalid")]
    InvalidSignature,
}

/// Ephemeral in-memory signing identity. Persistence must use Windows protected storage.
pub struct DeviceIdentity {
    signing_key: SigningKey,
}

/// Tracks the two separate human approvals required before a pairing may be persisted.
#[derive(Debug)]
pub struct PairingApproval {
    transcript: PairingTranscript,
    local_device: DeviceId,
    peer_device: DeviceId,
    local_proof: Option<SignatureProof>,
    peer_proof: Option<SignatureProof>,
}

impl PairingApproval {
    pub fn new(
        transcript: PairingTranscript,
        local_device: DeviceId,
        peer_device: DeviceId,
    ) -> Result<Self, TrustError> {
        let roles_match = (transcript.initiator == local_device
            && transcript.responder == peer_device)
            || (transcript.responder == local_device && transcript.initiator == peer_device);
        if !roles_match || !transcript.has_valid_display_code() {
            return Err(TrustError::IdentityMismatch);
        }
        Ok(Self {
            transcript,
            local_device,
            peer_device,
            local_proof: None,
            peer_proof: None,
        })
    }

    /// Call only after the local user explicitly approves the matching code.
    pub fn approve_local(
        &mut self,
        identity: &DeviceIdentity,
    ) -> Result<SignatureProof, TrustError> {
        if identity.device_id() != self.local_device {
            return Err(TrustError::IdentityMismatch);
        }
        let proof = identity.prove(&self.transcript);
        self.local_proof = Some(proof.clone());
        Ok(proof)
    }

    /// Accept a proof only after the peer user has explicitly approved the same code.
    pub fn record_peer_approval(&mut self, proof: SignatureProof) -> Result<(), TrustError> {
        DeviceIdentity::verify(&self.peer_device, &self.transcript, &proof)?;
        self.peer_proof = Some(proof);
        Ok(())
    }

    pub fn is_mutually_approved(&self) -> bool {
        self.local_proof.is_some() && self.peer_proof.is_some()
    }

    pub fn approved_peer_public_key(&self) -> Option<[u8; 32]> {
        self.is_mutually_approved()
            .then(|| self.peer_proof.as_ref().map(|proof| proof.public_key))
            .flatten()
    }
}

impl DeviceIdentity {
    pub fn generate() -> Self {
        Self {
            signing_key: SigningKey::generate(&mut OsRng),
        }
    }

    pub fn from_secret_bytes(mut secret: [u8; 32]) -> Self {
        let identity = Self {
            signing_key: SigningKey::from_bytes(&secret),
        };
        zeroize::Zeroize::zeroize(&mut secret);
        identity
    }

    pub fn device_id(&self) -> DeviceId {
        DeviceId::from_public_key(self.signing_key.verifying_key().as_bytes())
    }

    pub fn public_key(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }

    /// Export the secret only for the platform-protected identity store.
    pub(crate) fn secret_bytes(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }

    pub fn prove(&self, transcript: &PairingTranscript) -> SignatureProof {
        self.prove_bytes(transcript.signing_bytes())
    }

    pub(crate) fn prove_bytes(&self, message: Vec<u8>) -> SignatureProof {
        SignatureProof {
            device_id: self.device_id(),
            public_key: self.public_key(),
            signature: self.signing_key.sign(&message).to_bytes().to_vec(),
        }
    }

    pub fn verify(
        expected_device: &DeviceId,
        transcript: &PairingTranscript,
        proof: &SignatureProof,
    ) -> Result<(), TrustError> {
        Self::verify_bytes(expected_device, &transcript.signing_bytes(), proof)
    }

    pub(crate) fn verify_bytes(
        expected_device: &DeviceId,
        message: &[u8],
        proof: &SignatureProof,
    ) -> Result<(), TrustError> {
        if &proof.device_id != expected_device
            || DeviceId::from_public_key(&proof.public_key) != *expected_device
        {
            return Err(TrustError::IdentityMismatch);
        }
        let key = VerifyingKey::from_bytes(&proof.public_key)
            .map_err(|_| TrustError::InvalidPublicKey)?;
        let signature =
            Signature::from_slice(&proof.signature).map_err(|_| TrustError::InvalidSignature)?;
        key.verify_strict(message, &signature)
            .map_err(|_| TrustError::InvalidSignature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transcript(initiator: &DeviceIdentity, responder: &DeviceIdentity) -> PairingTranscript {
        PairingTranscript {
            initiator: initiator.device_id(),
            responder: responder.device_id(),
            initiator_nonce: [7; 32],
            responder_nonce: [9; 32],
            initiator_ephemeral: [11; 32],
            responder_ephemeral: [13; 32],
            displayed_code: "482193".into(),
        }
    }

    #[test]
    fn proof_binds_peer_identity_and_pairing_challenge() {
        let initiator = DeviceIdentity::generate();
        let responder = DeviceIdentity::generate();
        let challenge = transcript(&initiator, &responder);
        let proof = responder.prove(&challenge);
        assert_eq!(
            DeviceIdentity::verify(&responder.device_id(), &challenge, &proof),
            Ok(())
        );

        let mut altered = challenge;
        altered.displayed_code = "000000".into();
        assert_eq!(
            DeviceIdentity::verify(&responder.device_id(), &altered, &proof),
            Err(TrustError::InvalidSignature)
        );
    }

    #[test]
    fn public_key_cannot_be_relabelled_as_another_device() {
        let first = DeviceIdentity::generate();
        let second = DeviceIdentity::generate();
        let challenge = transcript(&first, &second);
        let proof = first.prove(&challenge);
        assert_eq!(
            DeviceIdentity::verify(&second.device_id(), &challenge, &proof),
            Err(TrustError::IdentityMismatch)
        );
    }

    #[test]
    fn challenge_encodes_all_fields_including_both_nonces() {
        let first = DeviceIdentity::generate();
        let second = DeviceIdentity::generate();
        let original = transcript(&first, &second);
        let mut changed_nonce = original.clone();
        changed_nonce.initiator_nonce[0] ^= 1;
        assert_ne!(original.signing_bytes(), changed_nonce.signing_bytes());
        let mut changed_ephemeral = original.clone();
        changed_ephemeral.responder_ephemeral[0] ^= 1;
        assert_ne!(original.signing_bytes(), changed_ephemeral.signing_bytes());
    }

    #[test]
    fn both_devices_must_approve_before_pairing_is_ready() {
        let first = DeviceIdentity::generate();
        let second = DeviceIdentity::generate();
        let challenge = PairingTranscript::create(first.device_id(), second.device_id());
        assert!(challenge.has_valid_display_code());

        let mut first_side =
            PairingApproval::new(challenge.clone(), first.device_id(), second.device_id()).unwrap();
        let mut second_side =
            PairingApproval::new(challenge, second.device_id(), first.device_id()).unwrap();
        assert!(!first_side.is_mutually_approved());

        let first_proof = first_side.approve_local(&first).unwrap();
        let second_proof = second_side.approve_local(&second).unwrap();
        first_side.record_peer_approval(second_proof).unwrap();
        second_side.record_peer_approval(first_proof).unwrap();
        assert!(first_side.is_mutually_approved());
        assert!(second_side.is_mutually_approved());
    }

    #[test]
    fn pairing_code_changes_with_identity_or_nonce() {
        let first = DeviceIdentity::generate();
        let second = DeviceIdentity::generate();
        let challenge = PairingTranscript::create(first.device_id(), second.device_id());
        let mut changed = challenge.clone();
        changed.responder_nonce[0] ^= 1;
        assert!(!changed.has_valid_display_code());
        assert_eq!(
            PairingApproval::new(changed, first.device_id(), second.device_id()).unwrap_err(),
            TrustError::IdentityMismatch
        );
    }
}
