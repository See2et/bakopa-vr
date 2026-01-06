use anyhow::Result;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

/// A spawned Bloom WS server for tests.
#[allow(dead_code)]
pub struct BloomWsServer {
    pub addr: SocketAddr,
    handle: Option<bloom_ws::WsServerHandle>,
}

impl BloomWsServer {
    /// ws://{addr}/ws
    #[allow(dead_code)]
    pub fn ws_url(&self) -> String {
        format!("ws://{}/ws", self.addr)
    }
}

impl Drop for BloomWsServer {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            std::mem::drop(tokio::spawn(async move {
                handle.shutdown().await;
            }));
        }
    }
}

#[allow(dead_code)]
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

#[allow(dead_code)]
pub async fn spawn_bloom_ws_with_mock_core(
    leave_notify: Option<Arc<Notify>>,
) -> Result<(BloomWsServer, Arc<Mutex<bloom_ws::MockCore>>)> {
    let bind_addr: SocketAddr = "127.0.0.1:0".parse()?;
    let room_id = bloom_core::RoomId::new();
    let owner = bloom_core::ParticipantId::new();
    let mut core = bloom_ws::MockCore::new(bloom_core::CreateRoomResult {
        room_id,
        self_id: owner.clone(),
        participants: vec![owner],
    });
    if let Some(notify) = leave_notify {
        core = core.with_leave_notify(notify);
    }
    let core_arc = Arc::new(Mutex::new(core));
    let shared = bloom_ws::SharedCore::from_arc(core_arc.clone());
    let server = bloom_ws::start_ws_server(bind_addr, shared).await?;
    let addr = server.addr;
    Ok((
        BloomWsServer {
            addr,
            handle: Some(server),
        },
        core_arc,
    ))
}
