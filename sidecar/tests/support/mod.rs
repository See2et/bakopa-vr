use anyhow::Result;
use axum::Router;
use futures_util::StreamExt;
use std::net::SocketAddr;
use tokio::{net::TcpListener, task::JoinHandle};
use tokio_tungstenite::tungstenite::Message;

#[allow(dead_code)]
pub async fn wait_for_self_joined(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> serde_json::Value {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while std::time::Instant::now() < deadline {
        match tokio::time::timeout(std::time::Duration::from_millis(200), ws.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
                    continue;
                };
                if json.get("type").and_then(|v| v.as_str()) == Some("SelfJoined") {
                    return json;
                }
            }
            Ok(Some(Ok(_))) => {}
            Ok(Some(Err(err))) => panic!("ws error: {err:?}"),
            Ok(None) => break,
            Err(_) => {}
        }
    }
    panic!("expected SelfJoined within deadline");
}

pub mod bloom;

/// A spawned axum test server that lives for the duration of the handle.
pub struct TestServer {
    pub addr: SocketAddr,
    handle: JoinHandle<()>,
}

impl TestServer {
    #[allow(dead_code)]
    /// Convenience builder for ws:// URLs.
    pub fn ws_url(&self, path: &str) -> String {
        format!("ws://{}{}", self.addr, path)
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

/// Scoped environment variable setter for tests.
#[allow(dead_code)]
pub struct EnvGuard {
    key: String,
    prev: Option<String>,
}

impl EnvGuard {
    #[allow(dead_code)]
    pub fn set<K: Into<String>, V: Into<String>>(key: K, value: V) -> Self {
        let key = key.into();
        let prev = std::env::var(&key).ok();
        std::env::set_var(&key, value.into());
        EnvGuard { key, prev }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(prev) = self.prev.take() {
            std::env::set_var(&self.key, prev);
        } else {
            std::env::remove_var(&self.key);
        }
    }
}

/// Spawn an axum server bound to 127.0.0.1:0 for tests.
#[allow(dead_code)]
pub async fn spawn_axum(router: Router) -> Result<TestServer> {
    spawn_axum_on("127.0.0.1:0".parse().expect("default bind addr"), router).await
}

pub async fn spawn_axum_on(bind_addr: SocketAddr, router: Router) -> Result<TestServer> {
    let listener = TcpListener::bind(bind_addr).await?;
    let addr = listener.local_addr()?;
    let handle = tokio::spawn(async move {
        // If the server errors, bubble up via panic to fail the test.
        axum::serve(listener, router.into_make_service())
            .await
            .expect("test server should run");
    });
    Ok(TestServer { addr, handle })
}
