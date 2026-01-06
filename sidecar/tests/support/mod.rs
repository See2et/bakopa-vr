use anyhow::Result;
use axum::Router;
use std::net::SocketAddr;
use tokio::{net::TcpListener, task::JoinHandle};

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
pub async fn spawn_axum(router: Router) -> Result<TestServer> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let handle = tokio::spawn(async move {
        // If the server errors, bubble up via panic to fail the test.
        axum::serve(listener, router.into_make_service())
            .await
            .expect("test server should run");
    });
    Ok(TestServer { addr, handle })
}
