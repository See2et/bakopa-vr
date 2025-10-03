use std::time::Duration;

use chrono::Utc;
use std::io::ErrorKind;
use shared::{Keypair, PeerAddress, PingMessage, SessionConfig};
use sidecar::{PeerError, PeerEvent, PeerSession};

#[tokio::test]
async fn listener_lifecycle_and_ping_pong() {
    let listener_config = SessionConfig::new("127.0.0.1:0".parse().unwrap(), Keypair::generate());
    let (listener, advertised) = match PeerSession::listen(listener_config).await {
        Ok(value) => value,
        Err(PeerError::Io(err)) if err.kind() == ErrorKind::PermissionDenied => return,
        Err(err) => panic!("unexpected listen error: {err}"),
    };

    let dialer_config = SessionConfig::new("0.0.0.0:0".parse().unwrap(), Keypair::generate());
    let dialer = match PeerSession::dial(dialer_config, &advertised).await {
        Ok(session) => session,
        Err(PeerError::Io(err)) if err.kind() == ErrorKind::PermissionDenied => return,
        Err(err) => panic!("unexpected dial error: {err}"),
    };

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
async fn dial_unknown_peer_returns_timeout() {
    let config = SessionConfig::new("0.0.0.0:0".parse().unwrap(), Keypair::generate());
    let addr = PeerAddress::new("/ip4/127.0.0.1/udp/6001/quic-v1/p2p/unknown");
    let session = match PeerSession::dial(config, &addr).await {
        Ok(session) => session,
        Err(PeerError::Io(err)) if err.kind() == ErrorKind::PermissionDenied => return,
        Err(err) => panic!("unexpected dial error: {err}"),
    };
    let ping = session.config().next_ping(1);
    let _ = session.send_ping(&ping).await;

    let result = session.next_event(Duration::from_millis(200)).await;
    assert!(matches!(
        result,
        Err(PeerError::Timeout(_)) | Err(PeerError::ChannelClosed)
    ));
}
