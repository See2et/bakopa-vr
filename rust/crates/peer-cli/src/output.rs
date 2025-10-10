use anyhow::Result;
use serde::Serialize;
use serde_json::json;
use shared::{PeerAddress, PingMessage, PongMessage, SessionConfig};
use sidecar::DialRetryEvent;

pub fn print_listen_ready(addr: &PeerAddress, config: &SessionConfig) {
    println!("listening on {}", addr.as_str());
    println!("peer id: {}", config.keypair.peer_id());
}

pub fn print_info(message: &str) {
    println!("{message}");
}

pub fn print_error(message: &str) {
    eprintln!("error: {message}");
}

pub fn print_ping(message: &PingMessage) {
    println!(
        "ping sent sequence={} sent_at={}",
        message.sequence,
        message.sent_at
    );
}

pub fn print_pong(message: &PongMessage) {
    println!(
        "pong sequence={} sent_at={} received_ping_at={}",
        message.sequence,
        message.sent_at,
        message.received_ping_at
    );
}

pub fn print_retry(event: &DialRetryEvent) -> Result<()> {
    println!(
        "retry {}/{} for {} (elapsed {} ms) waiting {} ms before next attempt: {}",
        event.attempt,
        event.max_attempts,
        event.peer,
        event.elapsed_ms,
        event.backoff_ms,
        event.error
    );

    let log = json!({
        "event": "dial_retry",
        "peer": event.peer,
        "attempt": event.attempt,
        "max_attempts": event.max_attempts,
        "next_backoff_ms": event.backoff_ms,
        "elapsed_ms": event.elapsed_ms,
        "error": event.error,
    });
    println!("{}", serde_json::to_string(&log)?);
    Ok(())
}

pub fn print_json<T: Serialize>(value: &T) -> Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    println!("{json}");
    Ok(())
}
