use anyhow::Result;
use std::net::SocketAddr;

/// A spawned Bloom WS server for tests.
pub struct BloomWsServer {
    pub addr: SocketAddr,
    handle: Option<bloom_ws::WsServerHandle>,
}

impl BloomWsServer {
    /// ws://{addr}/ws
    pub fn ws_url(&self) -> String {
        format!("ws://{}/ws", self.addr)
    }
}

impl Drop for BloomWsServer {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = tokio::spawn(async move {
                handle.shutdown().await;
            });
        }
    }
}

pub async fn spawn_bloom_ws() -> Result<BloomWsServer> {
    let bind_addr: SocketAddr = "127.0.0.1:0".parse()?;
    let core = bloom_ws::SharedCore::new(bloom_ws::RealCore::new());
    let server = bloom_ws::start_ws_server(bind_addr, core).await?;
    let addr = server.addr;
    Ok(BloomWsServer {
        addr,
        handle: Some(server),
    })
}
