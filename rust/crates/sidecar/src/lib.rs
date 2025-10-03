#![deny(unsafe_code)]

/// Minimal API surface to confirm the crate compiles.
pub fn initialize() -> &'static str {
    if shared::crate_ready() {
        "sidecar-initialized"
    } else {
        "sidecar-initialization-failed"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_returns_expected_message() {
        assert_eq!(initialize(), "sidecar-initialized");
    }
}
