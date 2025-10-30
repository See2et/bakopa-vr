use anyhow::Result;

use super::SyncerNode;

/// Marker type for sending and receiving application messages over the P2P link.
#[derive(Clone, Debug)]
pub struct MessageChannel;

impl MessageChannel {
    /// Constructs a message channel from a running node handle.
    pub fn new(node: SyncerNode) -> Self {
        let _ = node;
        MessageChannel
    }

    /// Sends a single application message to a remote peer.
    pub async fn send(&self, message: String) -> Result<()> {
        let _ = message;
        todo!("MessageChannel::send is not implemented yet");
    }

    /// Awaits the next incoming application message.
    pub async fn next(&self) -> Result<String> {
        todo!("MessageChannel::next is not implemented yet");
    }
}
