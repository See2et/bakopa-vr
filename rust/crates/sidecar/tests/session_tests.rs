use std::time::Duration;

use chrono::Utc;
use shared::{Keypair, PeerAddress, PingMessage, SessionConfig};
use sidecar::{PeerError, PeerEvent, PeerSession};

#[tokio::test]
async fn listener_lifecycle_and_ping_pong() {
    let listener_config = SessionConfig::new("127.0.0.1:5001".parse().unwrap(), Keypair::generate());
    let (listener, advertised) = PeerSession::listen(listener_config).await.unwrap();

    let dialer_config = SessionConfig::new("0.0.0.0:0".parse().unwrap(), Keypair::generate());
    let dialer = PeerSession::dial(dialer_config, &advertised).await.unwrap();

    let ping = PingMessage::new(1, Utc::now());
    dialer.send_ping(&ping).await.unwrap();

    let event = listener
        .next_event(Duration::from_millis(200))
        .await
        .expect("listener receive ping");
    let received_ping = match event {
        PeerEvent::Ping(p) => p,
        _ => panic!("expected ping"),
    };

    let pong = PeerSession::make_pong(&received_ping);
    listener.send_pong(&pong).await.unwrap();

    let event = dialer
        .next_event(Duration::from_millis(200))
        .await
        .expect("dialer receive pong");
    matches!(event, PeerEvent::Pong(_));
}

#[tokio::test]
async fn dial_unknown_peer_returns_error() {
    let config = SessionConfig::new("0.0.0.0:0".parse().unwrap(), Keypair::generate());
    let addr = PeerAddress::new("/ip4/127.0.0.1/udp/6001/quic-v1/p2p/unknown");
    let result = PeerSession::dial(config, &addr).await;
    assert!(matches!(result, Err(PeerError::ListenerNotFound { .. })));
}
