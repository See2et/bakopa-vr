#![deny(unsafe_code)]

use shared::{SessionConfig, Keypair};

/// Minimal API surface to confirm the crate compiles and can leverage the shared crate.
pub fn initialize() -> String {
    // Generate a lightweight session config to prove that the shared crate is wired in.
    let keypair = Keypair::generate();
    let config = SessionConfig::new("0.0.0.0:0".parse().expect("hardcoded addr"), keypair.clone());
    format!("sidecar-initialized-{}", config.keypair.peer_id())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_returns_message_with_peer_id() {
        let message = initialize();
        assert!(message.starts_with("sidecar-initialized-"));
    }
}
