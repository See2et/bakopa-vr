use anyhow::Result;
use rand::rng;
use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::config::NodeConfig;
use iroh::{Endpoint, EndpointAddr};

/// Placeholder structure representing a running Syncer node.
#[derive(Debug)]
pub struct SyncerNode {
    endpoint: Endpoint,
    accept_task: JoinHandle<()>,
    cancel: CancellationToken,
}

impl SyncerNode {
    /// Spawns a Syncer node according to the provided configuration.
    pub async fn start(config: &NodeConfig) -> Result<Self> {
        let secret_key = load_or_generate_secret_key(&config.secret_key_path)?;
        let builder = match config.listen_addr {
            SocketAddr::V4(v4) => Endpoint::builder()
                .secret_key(secret_key)
                .bind_addr_v4(*&v4),
            SocketAddr::V6(v6) => Endpoint::builder()
                .secret_key(secret_key)
                .bind_addr_v6(*&v6),
        };

        let endpoint = builder.bind().await?;

        let cancel = CancellationToken::new();
        let accept_task = tokio::spawn({
            let endpoint = endpoint.clone();
            let cancel = cancel.clone();
            async move {
                loop {
                    tokio::select! {
                        _ = cancel.cancelled() => break,
                        incoming = endpoint.accept() =>
                            match incoming {
                                Some(connecting) => match connecting.await {
                                    Ok(connection) => {
                                        todo!("successed to connect!")
                                    }
                                    Err(err) => {
                                        todo!("failed to connect: {}", err)
                                    }
                                },
                                _ => break,
                            }
                    }
                }
            }
        });

        Ok(SyncerNode {
            endpoint: endpoint,
            accept_task: accept_task,
            cancel: cancel,
        })
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
        match fs::read(path) {
            Ok(bytes) => {
                let bytes: [u8; 32] = bytes
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("Invalid secret key length"))?;
                iroh::SecretKey::from_bytes(&bytes)
            }
            Err(e) => {
                eprintln!(
                    "Failed to read secret key at {:?}: {}. Generating new one.",
                    path, e
                );
                iroh::SecretKey::generate(&mut rng())
            }
        }
    } else {
        iroh::SecretKey::generate(&mut rng())
    };
    Ok(secret_key)
}
