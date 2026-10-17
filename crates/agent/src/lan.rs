//! LAN endpoint and best-effort mDNS discovery for authenticated agent streams.
//!
//! Advertisements are untrusted hints only. Peer identity is established by the
//! signed pairing/reconnect handshake in `transport`, never by mDNS metadata.

use crate::protocol::DeviceId;
use crate::trust::DeviceIdentity;
use mdns_sd::{Receiver as MdnsReceiver, ServiceDaemon, ServiceEvent, ServiceInfo};
use std::collections::BTreeSet;
use std::io;
use std::net::{IpAddr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::time::Duration;
use thiserror::Error;
use tokio::net::{TcpListener, TcpStream};

const SERVICE_TYPE: &str = "_uwspace._tcp.local.";

#[derive(Debug, Error)]
pub enum LanError {
    #[error("TCP listener failed: {0}")]
    Io(#[from] io::Error),
    #[error("LAN service discovery failed: {0}")]
    Mdns(#[from] mdns_sd::Error),
    #[error("LAN advertisement is invalid")]
    InvalidAdvertisement,
}

/// A bound endpoint advertised on local network interfaces.
pub struct LanEndpoint {
    listener: TcpListener,
    daemon: ServiceDaemon,
    fullname: String,
    device_id: DeviceId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredPeer {
    pub device_id: DeviceId,
    pub addresses: Vec<SocketAddr>,
}

impl LanEndpoint {
    /// Binds an ephemeral TCP port and advertises it using addresses discovered
    /// from active interfaces. No address or device identity is hard-coded.
    pub async fn bind(identity: &DeviceIdentity, display_name: &str) -> Result<Self, LanError> {
        let listener =
            TcpListener::bind(SocketAddrV4::new(std::net::Ipv4Addr::UNSPECIFIED, 0)).await?;
        let port = listener.local_addr()?.port();
        let device_id = identity.device_id();
        let suffix = &device_id.0[..16];
        let instance = format!("UW-{suffix}");
        let host = format!("uw-{suffix}.local.");
        let properties = [
            ("protocol", "1"),
            ("device", device_id.0.as_str()),
            ("name", display_name),
        ];
        let info = ServiceInfo::new(
            SERVICE_TYPE,
            &instance,
            &host,
            "",
            port,
            properties.as_slice(),
        )?
        .enable_addr_auto();
        let daemon = ServiceDaemon::new()?;
        daemon.register(info.clone())?;
        Ok(Self {
            listener,
            daemon,
            fullname: info.get_fullname().to_owned(),
            device_id,
        })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    pub fn device_id(&self) -> &DeviceId {
        &self.device_id
    }

    pub async fn accept(&self) -> io::Result<(TcpStream, SocketAddr)> {
        self.listener.accept().await
    }

    /// Removes the advertisement and stops the discovery daemon.
    pub fn shutdown(self) -> Result<(), LanError> {
        let _ = self.daemon.unregister(&self.fullname)?;
        let _ = self.daemon.shutdown()?;
        Ok(())
    }
}

pub struct PeerBrowser {
    events: MdnsReceiver<ServiceEvent>,
    _daemon: ServiceDaemon,
    local_device: DeviceId,
}

impl PeerBrowser {
    pub fn start(local_device: DeviceId) -> Result<Self, LanError> {
        let daemon = ServiceDaemon::new()?;
        let events = daemon.browse(SERVICE_TYPE)?;
        Ok(Self {
            events,
            _daemon: daemon,
            local_device,
        })
    }

    /// Waits for one resolved peer. Invalid or self advertisements are skipped.
    pub fn next_peer(&self, timeout: Duration) -> Result<Option<DiscoveredPeer>, LanError> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return Ok(None);
            }
            match self.events.recv_timeout(remaining) {
                Ok(ServiceEvent::ServiceResolved(service)) => {
                    let Some(device) = service.get_property_val_str("device") else {
                        continue;
                    };
                    if device.len() != 64
                        || !device.bytes().all(|b| b.is_ascii_hexdigit())
                        || device.eq_ignore_ascii_case(&self.local_device.0)
                    {
                        continue;
                    }
                    let addresses: BTreeSet<_> = service
                        .get_addresses()
                        .iter()
                        .map(|ip| ip.to_ip_addr())
                        .filter(|ip| !ip.is_unspecified() && !ip.is_multicast())
                        .map(|ip| match ip {
                            IpAddr::V4(ip) => SocketAddr::V4(SocketAddrV4::new(ip, service.port)),
                            IpAddr::V6(ip) => {
                                SocketAddr::V6(SocketAddrV6::new(ip, service.port, 0, 0))
                            }
                        })
                        .collect();
                    if !addresses.is_empty() {
                        return Ok(Some(DiscoveredPeer {
                            device_id: DeviceId(device.to_ascii_lowercase()),
                            addresses: addresses.into_iter().collect(),
                        }));
                    }
                }
                Ok(_) => continue,
                Err(_) => return Ok(None),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::{pair_over_stream, HandshakeRole};
    use tokio::time::timeout;

    #[tokio::test]
    async fn tcp_endpoint_authenticates_two_devices_and_exchanges_ciphertext() {
        let (left_identity, right_identity) =
            (DeviceIdentity::generate(), DeviceIdentity::generate());
        let endpoint = LanEndpoint::bind(&left_identity, "Test device")
            .await
            .unwrap();
        let address = SocketAddr::V4(SocketAddrV4::new(
            std::net::Ipv4Addr::LOCALHOST,
            endpoint.local_addr().unwrap().port(),
        ));
        let (client_result, accepted_result) = tokio::join!(
            TcpStream::connect(address),
            timeout(Duration::from_secs(2), endpoint.accept())
        );
        let client = client_result.unwrap();
        let (server, peer_addr) = accepted_result.unwrap().unwrap();
        assert_eq!(peer_addr.ip(), IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));

        let (left, right) = tokio::join!(
            pair_over_stream(
                client,
                &left_identity,
                HandshakeRole::Initiator,
                |challenge| async move { challenge.verification_code.len() == 6 }
            ),
            pair_over_stream(
                server,
                &right_identity,
                HandshakeRole::Responder,
                |challenge| async move { challenge.verification_code.len() == 6 }
            )
        );
        let (mut left_sender, _, _) = left.unwrap().into_encrypted();
        let (_, mut right_receiver, _) = right.unwrap().into_encrypted();
        let (sent, received) = tokio::join!(
            left_sender.send(b"authenticated over TCP"),
            right_receiver.receive()
        );
        sent.unwrap();
        assert_eq!(received.unwrap(), b"authenticated over TCP");
        endpoint.shutdown().unwrap();
    }
}
