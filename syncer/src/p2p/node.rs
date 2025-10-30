use anyhow::Result;

use crate::config::NodeConfig;
use iroh::{Endpoint, EndpointAddr};

/// Placeholder structure representing a running Syncer node.
#[derive(Clone, Debug)]
pub struct SyncerNode;

impl SyncerNode {
    /// Spawns a Syncer node according to the provided configuration.
    pub async fn start(config: &NodeConfig) -> Result<Self> {
        let _ = config;
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
