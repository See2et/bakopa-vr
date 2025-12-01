use std::net::SocketAddr;

use bloom_ws::{start_ws_server, RealCore, SharedCore};
use tracing_subscriber::{fmt, EnvFilter};

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    // logging
    let _ = fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .try_init();

    let addr: SocketAddr = std::env::var("BLOOM_WS_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:8080".to_string())
        .parse()
        .expect("invalid BLOOM_WS_ADDR");

    let core = SharedCore::new(RealCore::new());
    let handle = start_ws_server(addr, core).await?;
    tracing::info!(addr = %handle.addr, "Bloom WS listening");

    // wait for ctrl-c
    tokio::signal::ctrl_c().await?;
    tracing::info!("Shutting down...");
    handle.shutdown().await;
    Ok(())
}
