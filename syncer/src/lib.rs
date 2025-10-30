pub mod config;
pub mod p2p;

pub use config::NodeConfig;
pub use p2p::{MessageChannel, SyncerNode};

use anyhow::Result;

#[derive(Debug)]
pub struct SyncerHandle {
    node: SyncerNode,
    channel: MessageChannel,
}

impl SyncerHandle {
    /// Returns a cloneable channel handle for message exchange.
    pub fn channel(&self) -> &MessageChannel {
        &self.channel
    }

    /// Returns a reference to the underlying node for diagnostics.
    pub fn node(&self) -> &SyncerNode {
        &self.node
    }

    /// Gracefully shuts down the running node and associated resources.
    pub async fn shutdown(self) -> Result<()> {
        todo!("SyncerHandle::shutdown is not implemented yet");
    }
}

/// Bootstraps a Syncer node using the provided configuration.
pub async fn start_syncer(config: NodeConfig) -> Result<SyncerHandle> {
    let _ = config;
    todo!("start_syncer is not implemented yet");
}
