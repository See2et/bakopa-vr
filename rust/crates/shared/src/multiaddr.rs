use std::net::{IpAddr, SocketAddr};

use serde::{Deserialize, Serialize};

use crate::Keypair;

/// Textual representation of a peer multi-address used for manual testing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct PeerAddress(String);

impl PeerAddress {
    /// Construct a new peer address from its raw string value.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Compose a peer address string from socket address and keypair.
    pub fn from_parts(addr: SocketAddr, keypair: &Keypair) -> Self {
        let peer_id = keypair.peer_id();
        Self(format_multiaddr(addr, &peer_id))
    }

    /// Access the string representation.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Extract the peer identifier segment from the multi-address.
    pub fn peer_id(&self) -> Option<&str> {
        self.0.split("/p2p/").nth(1)
    }

    /// Convert the multi-address into a socket address.
    pub fn to_socket_addr(&self) -> Option<SocketAddr> {
        let segments: Vec<&str> = self.0.split('/').filter(|s| !s.is_empty()).collect();
        let mut ip: Option<IpAddr> = None;
        let mut port: Option<u16> = None;

        let mut idx = 0;
        while idx < segments.len() {
            match segments[idx] {
                "ip4" => {
                    if idx + 1 < segments.len() {
                        if let Ok(parsed) = segments[idx + 1].parse::<std::net::Ipv4Addr>() {
                            ip = Some(IpAddr::V4(parsed));
                        }
                        idx += 2;
                        continue;
                    }
                }
                "ip6" => {
                    if idx + 1 < segments.len() {
                        if let Ok(parsed) = segments[idx + 1].parse::<std::net::Ipv6Addr>() {
                            ip = Some(IpAddr::V6(parsed));
                        }
                        idx += 2;
                        continue;
                    }
                }
                "udp" => {
                    if idx + 1 < segments.len() {
                        port = segments[idx + 1].parse().ok();
                        idx += 2;
                        continue;
                    }
                }
                _ => {}
            }
            idx += 1;
        }

        match (ip, port) {
            (Some(ip), Some(port)) => Some(SocketAddr::new(ip, port)),
            _ => None,
        }
    }
}

impl From<PeerAddress> for String {
    fn from(value: PeerAddress) -> Self {
        value.0
    }
}

impl AsRef<str> for PeerAddress {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// Formats a simplified multi-address string of the form `/ipX/<ip>/<transport>/<port>/p2p/<peer-id>`.
pub fn format_multiaddr(addr: SocketAddr, peer_id: &str) -> String {
    let ip_component = match addr.ip() {
        IpAddr::V4(ipv4) => format!("/ip4/{}", ipv4),
        IpAddr::V6(ipv6) => format!("/ip6/{}", ipv6),
    };
    let transport_component = format!("/udp/{}", addr.port());
    format!("{}{}{}{}{}",
        ip_component,
        transport_component,
        "/quic-v1",
        "/p2p/",
        peer_id
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use once_cell::sync::Lazy;

    static KEYPAIR: Lazy<Keypair> = Lazy::new(Keypair::generate);

    #[test]
    fn formats_ipv4_address() {
        let addr: SocketAddr = "127.0.0.1:9000".parse().unwrap();
        let formatted = format_multiaddr(addr, &KEYPAIR.peer_id());
        assert!(formatted.starts_with("/ip4/127.0.0.1/udp/9000/quic-v1/p2p/"));
    }

    #[test]
    fn peer_address_from_parts_contains_peer_id() {
        let addr: SocketAddr = "0.0.0.0:7000".parse().unwrap();
        let peer_addr = PeerAddress::from_parts(addr, &KEYPAIR);
        assert!(peer_addr.as_str().contains(&KEYPAIR.peer_id()));
        assert_eq!(peer_addr.peer_id().unwrap(), KEYPAIR.peer_id());
        assert_eq!(peer_addr.to_socket_addr().unwrap(), addr);
    }
}
