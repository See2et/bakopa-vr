#![deny(unsafe_code)]

pub mod error;
pub mod session;

use shared::{Keypair, SessionConfig};

pub use error::PeerError;
pub use session::{PeerEvent, PeerSession};

/// Minimal API surface to confirm the crate compiles and can leverage the shared crate.
pub fn initialize() -> String {
    let keypair = Keypair::generate();
    let config = SessionConfig::new("0.0.0.0:0".parse().expect("hardcoded addr"), keypair.clone());
    format!("sidecar-initialized-{}", config.keypair.peer_id())
}
