use std::net::SocketAddr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{keypair::Keypair, multiaddr, pingpong::PingMessage};

/// Configuration values required to initialise a peer session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionConfig {
    /// Socket address used to listen for inbound connections.
    pub listen_addr: SocketAddr,
    /// Cryptographic identity for the peer.
    pub keypair: Keypair,
    /// Maximum number of connection retries.
    pub max_retries: u8,
    /// Backoff interval in milliseconds between retries.
    pub retry_backoff_ms: u64,
}

impl SessionConfig {
    /// Create a new `SessionConfig` with the provided address and keypair.
    pub fn new(listen_addr: SocketAddr, keypair: Keypair) -> Self {
        Self {
            listen_addr,
            keypair,
            max_retries: 3,
            retry_backoff_ms: 500,
        }
    }

    /// Builder-style update for retry count.
    pub fn with_max_retries(mut self, retries: u8) -> Self {
        self.max_retries = retries;
        self
    }

    /// Builder-style update for retry backoff.
    pub fn with_retry_backoff_ms(mut self, backoff_ms: u64) -> Self {
        self.retry_backoff_ms = backoff_ms;
        self
    }

    /// Convenience helper that returns the peer's advertised multi-address.
    pub fn advertised_multiaddr(&self) -> multiaddr::PeerAddress {
        multiaddr::PeerAddress::from_parts(self.listen_addr, &self.keypair)
    }

    /// Generate a ping payload for the given sequence number using the current clock.
    pub fn next_ping(&self, sequence: u32) -> PingMessage {
        PingMessage::new(sequence, Utc::now())
    }
}

impl Default for SessionConfig {
    fn default() -> Self {
        let listen_addr: SocketAddr = "0.0.0.0:0".parse().expect("valid default addr");
        Self::new(listen_addr, Keypair::generate())
    }
}

/// Snapshot of connection attempt metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConnectionAttempt {
    pub sequence: u32,
    pub started_at: DateTime<Utc>,
    pub retries_used: u8,
}

impl ConnectionAttempt {
    pub fn new(sequence: u32, retries_used: u8) -> Self {
        Self {
            sequence,
            started_at: Utc::now(),
            retries_used,
        }
    }
}
