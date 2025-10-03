use std::time::Duration;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PeerError {
    #[error("listener already registered for peer id {peer_id}")]
    ListenerAlreadyRegistered { peer_id: String },
    #[error("listener not found for peer id {peer_id}")]
    ListenerNotFound { peer_id: String },
    #[error("invalid multiaddr: {0}")]
    InvalidMultiaddr(String),
    #[error("transport is not ready")]
    TransportNotReady,
    #[error("communication channel closed")]
    ChannelClosed,
    #[error("operation timed out after {0:?}")]
    Timeout(Duration),
    #[error("encoding error: {0}")]
    Encoding(String),
    #[error("decoding error: {0}")]
    Decoding(String),
}

impl From<rmp_serde::encode::Error> for PeerError {
    fn from(value: rmp_serde::encode::Error) -> Self {
        PeerError::Encoding(value.to_string())
    }
}

impl From<rmp_serde::decode::Error> for PeerError {
    fn from(value: rmp_serde::decode::Error) -> Self {
        PeerError::Decoding(value.to_string())
    }
}
