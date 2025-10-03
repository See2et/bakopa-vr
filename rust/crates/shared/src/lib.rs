#![deny(unsafe_code)]

//! Shared data structures and helpers for the BakopaVR peer ping-pong prototype.

pub mod config;
pub mod keypair;
pub mod multiaddr;
pub mod pingpong;

pub use config::SessionConfig;
pub use keypair::Keypair;
pub use multiaddr::{format_multiaddr, PeerAddress};
pub use pingpong::{encode_ping, encode_pong, decode_ping, decode_pong, PingMessage, PongMessage, RttReport};
