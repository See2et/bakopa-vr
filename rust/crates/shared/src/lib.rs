#![deny(unsafe_code)]

/// Placeholder shared crate API.
pub fn crate_ready() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_is_ready() {
        assert!(crate_ready());
    }
}
