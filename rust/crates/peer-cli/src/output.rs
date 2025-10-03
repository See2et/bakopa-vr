use anyhow::Result;
use serde::Serialize;
use shared::{PeerAddress, PingMessage, PongMessage, SessionConfig};

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

pub fn print_json<T: Serialize>(value: &T) -> Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    println!("{json}");
    Ok(())
}
