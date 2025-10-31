use anyhow::Result;
use rand::rng;
use std::fs;
use std::path::PathBuf;

use crate::config::NodeConfig;
use iroh::{Endpoint, EndpointAddr};

/// Placeholder structure representing a running Syncer node.
#[derive(Clone, Debug)]
pub struct SyncerNode;

impl SyncerNode {
    /// Spawns a Syncer node according to the provided configuration.
    pub async fn start(config: &NodeConfig) -> Result<Self> {
        let _ = config;
        let key = load_or_generate_secret_key(&config.secret_key_path)?;
        todo!("SyncerNode::start is not implemented yet");
    }

    /// Returns advertised peer addresses for discovery.
    pub fn advertised_multiaddrs(&self) -> Vec<String> {
        todo!("SyncerNode::advertised_multiaddrs is not implemented yet");
    }

    /// Returns the endpoint address other peers can use to connect.
    pub fn endpoint_addr(&self) -> EndpointAddr {
        todo!("SyncerNode::endpoint_addr is not implemented yet");
    }
}

fn load_or_generate_secret_key(secret_key_path: &Option<PathBuf>) -> Result<iroh::SecretKey> {
    let secret_key = if let Some(path) = secret_key_path {
        let bytes = fs::read(path)?;
        let bytes: [u8; 32] = bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid secret key length"))?;
        iroh::SecretKey::from_bytes(&bytes)
    } else {
        iroh::SecretKey::generate(&mut rng())
    };
    Ok(secret_key)
}
