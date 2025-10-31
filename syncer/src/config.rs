use std::{net::SocketAddr, path::PathBuf};

use iroh::EndpointAddr;

/// Configuration describing how a Syncer node should bootstrap itself.
#[derive(Clone, Debug)]
pub struct NodeConfig {
    pub listen_addr: SocketAddr,
    pub peers: Vec<EndpointAddr>,
    pub secret_key_path: Option<PathBuf>,
    pub bootstrap_message: Option<String>,
}

impl NodeConfig {
    /// Creates a new configuration with the given listening address.
    pub fn new(listen_addr: SocketAddr) -> Self {
        Self {
            listen_addr,
            peers: Vec::new(),
            secret_key_path: None,
            bootstrap_message: None,
        }
    }

    /// Adds a peer multi-address to the bootstrap list.
    pub fn with_peer(mut self, peer: EndpointAddr) -> Self {
        self.peers.push(peer);
        self
    }

    /// Sets an optional bootstrap message to advertise on connect.
    pub fn with_bootstrap_message(mut self, message: impl Into<String>) -> Self {
        self.bootstrap_message = Some(message.into());
        self
    }

    /// Overrides the location where the node keeps its private key material.
    pub fn with_secret_key_path(mut self, path: PathBuf) -> Self {
        self.secret_key_path = Some(path);
        self
    }
}
