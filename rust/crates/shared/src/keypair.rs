use std::fmt;

use ed25519_dalek::{Keypair as DalekKeypair, PUBLIC_KEY_LENGTH, SECRET_KEY_LENGTH};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};

/// Length of the serialized Ed25519 keypair in bytes.
const KEYPAIR_LENGTH: usize = SECRET_KEY_LENGTH + PUBLIC_KEY_LENGTH;

/// Wrapper around an Ed25519 keypair used by the prototype.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Keypair {
    /// Concatenated secret (first 32 bytes) + public (last 32 bytes) key data.
    bytes: Vec<u8>,
}

impl Keypair {
    /// Generate a new Ed25519 keypair using the operating system RNG.
    pub fn generate() -> Self {
        let mut rng = OsRng;
        let kp = DalekKeypair::generate(&mut rng);
        Self { bytes: kp.to_bytes().to_vec() }
    }

    /// Construct a keypair from raw bytes.
    pub fn from_bytes(bytes: [u8; KEYPAIR_LENGTH]) -> Self {
        // `ed25519_dalek::Keypair::from_bytes` validates the key material.
        let _ = DalekKeypair::from_bytes(&bytes).expect("invalid keypair bytes");
        Self { bytes: bytes.to_vec() }
    }

    /// Return the raw secret key bytes.
    pub fn secret_key_bytes(&self) -> [u8; SECRET_KEY_LENGTH] {
        let mut secret = [0u8; SECRET_KEY_LENGTH];
        secret.copy_from_slice(&self.bytes[..SECRET_KEY_LENGTH]);
        secret
    }

    /// Return the raw public key bytes.
    pub fn public_key_bytes(&self) -> [u8; PUBLIC_KEY_LENGTH] {
        let mut public = [0u8; PUBLIC_KEY_LENGTH];
        public.copy_from_slice(&self.bytes[SECRET_KEY_LENGTH..]);
        public
    }

    /// Produce an `ed25519_dalek::Keypair` instance for cryptographic operations.
    pub fn to_ed25519(&self) -> DalekKeypair {
        let mut array = [0u8; KEYPAIR_LENGTH];
        array.copy_from_slice(&self.bytes);
        DalekKeypair::from_bytes(&array).expect("stored keypair must be valid")
    }

    /// Human-friendly peer identifier (base58-encoded public key bytes).
    pub fn peer_id(&self) -> String {
        bs58::encode(self.public_key_bytes()).into_string()
    }

    /// Expose the concatenated keypair bytes.
    pub fn to_bytes(&self) -> [u8; KEYPAIR_LENGTH] {
        let mut bytes = [0u8; KEYPAIR_LENGTH];
        bytes.copy_from_slice(&self.bytes);
        bytes
    }
}

impl Default for Keypair {
    fn default() -> Self {
        Self::generate()
    }
}

impl fmt::Debug for Keypair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Keypair")
            .field("public", &self.peer_id())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_unique_peer_ids() {
        let key_a = Keypair::generate();
        let key_b = Keypair::generate();
        assert_ne!(key_a.peer_id(), key_b.peer_id());
    }

    #[test]
    fn round_trips_through_bytes() {
        let original = Keypair::generate();
        let bytes = original.to_bytes();
        let restored = Keypair::from_bytes(bytes);
        assert_eq!(original.peer_id(), restored.peer_id());
        assert_eq!(original.secret_key_bytes(), restored.secret_key_bytes());
    }
}
