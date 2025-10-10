use std::time::Duration;

use anyhow::{Context, Result};
use clap::Args;
use serde::Serialize;
use shared::{Keypair, PeerAddress, SessionConfig};
use sidecar::{PeerEvent, PeerSession};

use crate::output;

#[derive(Debug, Args)]
pub struct DialArgs {
    /// Peer multi-address to connect to (e.g., /ip4/127.0.0.1/udp/5000/quic-v1/p2p/<peer-id>).
    #[arg(long)]
    pub peer: String,

    /// Local socket address to bind before dialling.
    #[arg(long, default_value = "0.0.0.0:0")]
    pub addr: String,

    /// Maximum number of retries for connection establishment.
    #[arg(long, default_value_t = 3)]
    pub max_retries: u8,

    /// Retry backoff in milliseconds between connection attempts.
    #[arg(long, default_value_t = 500)]
    pub retry_backoff_ms: u64,

    /// Timeout in milliseconds while waiting for pong response.
    #[arg(long, default_value_t = 1_000)]
    pub receive_timeout_ms: u64,
}

#[derive(Debug, Serialize)]
struct DialReport {
    sequence: u32,
    rtt_ms: f64,
    attempts: u8,
    peer: String,
}

pub async fn run(args: DialArgs) -> Result<()> {
    let peer_addr = PeerAddress::new(args.peer);
    let local_addr = args.addr.parse().context("invalid local addr")?;

    let config = SessionConfig::new(local_addr, Keypair::generate())
        .with_max_retries(args.max_retries)
        .with_retry_backoff_ms(args.retry_backoff_ms);

    let session = PeerSession::dial(config.clone(), &peer_addr).await?;

    let ping = config.next_ping(1);
    session.send_ping(&ping).await?;
    output::print_ping(&ping);

    let timeout = Duration::from_millis(args.receive_timeout_ms);
    let pong = loop {
        match session.next_event(timeout).await? {
            PeerEvent::Pong(pong) => break pong,
            PeerEvent::Ping(ping) => {
                // Unexpected ping from listener; respond and wait again.
                let pong = PeerSession::make_pong(&ping);
                session.send_pong(&pong).await?;
                output::print_info("received ping while dialling, responded with pong");
            }
            PeerEvent::DialRetry(retry) => {
                output::print_retry(&retry)?;
            }
        }
    };
    output::print_pong(&pong);

    let report = session.rtt_report(&ping, &pong);
    output::print_json(&DialReport {
        sequence: report.sequence,
        rtt_ms: report.rtt_ms,
        attempts: report.attempts,
        peer: peer_addr.peer_id().unwrap_or_default().to_string(),
    })?;

    Ok(())
}
