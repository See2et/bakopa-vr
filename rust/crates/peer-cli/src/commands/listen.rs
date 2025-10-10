use std::time::Duration;

use anyhow::Result;
use clap::Args;
use shared::{Keypair, SessionConfig};
use sidecar::{PeerEvent, PeerSession};
use tokio::signal;
use tokio::time::sleep;

use crate::output;

#[derive(Debug, Args)]
pub struct ListenArgs {
    /// Socket address to bind and accept peer connections.
    #[arg(long, default_value = "0.0.0.0:5000")]
    pub addr: String,

    /// Maximum number of retries for connection establishment.
    #[arg(long, default_value_t = 3)]
    pub max_retries: u8,

    /// Retry backoff in milliseconds between connection attempts.
    #[arg(long, default_value_t = 500)]
    pub retry_backoff_ms: u64,

    /// Timeout in milliseconds to wait for inbound messages before looping.
    #[arg(long, default_value_t = 2_000)]
    pub receive_timeout_ms: u64,
}

pub async fn run(args: ListenArgs) -> Result<()> {
    let listen_addr = args.addr.parse()?;
    let keypair = Keypair::generate();
    let config = SessionConfig::new(listen_addr, keypair)
        .with_max_retries(args.max_retries)
        .with_retry_backoff_ms(args.retry_backoff_ms);

    let (session, advertised) = PeerSession::listen(config.clone()).await?;

    output::print_listen_ready(&advertised, &config);

    // Gracefully handle Ctrl+C and keep servicing ping/pong events.
    tokio::select! {
        _ = run_event_loop(session, Duration::from_millis(args.receive_timeout_ms)) => {}
        _ = signal::ctrl_c() => {
            output::print_info("shutting down listener");
        }
    }

    Ok(())
}

async fn run_event_loop(session: PeerSession, receive_timeout: Duration) {
    loop {
        match session.next_event(receive_timeout).await {
            Ok(PeerEvent::Ping(ping)) => {
                output::print_ping(&ping);
                let pong = PeerSession::make_pong(&ping);
                if let Err(err) = session.send_pong(&pong).await {
                    output::print_error(&err.to_string());
                } else {
                    output::print_pong(&pong);
                }
            }
            Ok(PeerEvent::Pong(pong)) => {
                output::print_pong(&pong);
            }
            Ok(PeerEvent::DialRetry(_)) => {
                // Listener connections should not emit dial retry events, so ignore safely.
            }
            Err(sidecar::PeerError::Timeout(_)) => {
                // No inbound traffic, idle briefly to avoid busy loop.
                sleep(Duration::from_millis(50)).await;
            }
            Err(err) => {
                output::print_error(&format!("listener error: {err}"));
                sleep(Duration::from_millis(250)).await;
            }
        }
    }
}
