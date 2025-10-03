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
    }
}
