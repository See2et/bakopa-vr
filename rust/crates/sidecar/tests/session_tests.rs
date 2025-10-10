use std::time::Duration;

use chrono::Utc;
use std::io::ErrorKind;
use std::net::UdpSocket;
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

fn reserve_loopback_port() -> std::io::Result<u16> {
    let socket = UdpSocket::bind("127.0.0.1:0")?;
    let port = socket.local_addr()?.port();
    Ok(port)
}

#[tokio::test]
async fn dial_emits_retry_before_success() {
    let local_port = match reserve_loopback_port() {
        Ok(value) => value,
        Err(err) if err.kind() == ErrorKind::PermissionDenied => return,
        Err(_) => return,
    };

    let remote_port = match reserve_loopback_port() {
        Ok(port) => port,
        Err(err) if err.kind() == ErrorKind::PermissionDenied => return,
        Err(_) => return,
    };

    let blocker = match UdpSocket::bind(("127.0.0.1", local_port)) {
        Ok(socket) => socket,
        Err(err) if err.kind() == ErrorKind::PermissionDenied => return,
        Err(_) => return,
    };

    let listener_addr = format!("127.0.0.1:{remote_port}").parse().unwrap();
    let listener_keypair = Keypair::generate();
    let advertised = PeerAddress::from_parts(listener_addr, &listener_keypair);

    let dial_config = SessionConfig::new(
        format!("127.0.0.1:{local_port}").parse().unwrap(),
        Keypair::generate(),
    )
    .with_max_retries(3)
    .with_retry_backoff_ms(20);

    let advertised_for_dial = advertised.clone();
    let dial_task = tokio::spawn(async move { PeerSession::dial(dial_config, &advertised_for_dial).await });

    tokio::time::sleep(Duration::from_millis(40)).await;
    drop(blocker);

    let listener_config = SessionConfig::new(listener_addr, listener_keypair);
    let (listener, _) = match PeerSession::listen(listener_config).await {
        Ok(value) => value,
        Err(PeerError::Io(err)) if err.kind() == ErrorKind::PermissionDenied => return,
        Err(err) => panic!("unexpected listen error: {err}"),
    };

    let session = match dial_task.await.unwrap() {
        Ok(session) => session,
        Err(PeerError::Io(err)) if err.kind() == ErrorKind::PermissionDenied => return,
        Err(err) => panic!("unexpected dial error: {err}"),
    };

    assert!(session.attempts() >= 2, "dial should retry before succeeding");

    let retry_event = session
        .next_event(Duration::from_millis(200))
        .await
        .expect("retry event enqueued");
    let retry_event = match retry_event {
        PeerEvent::DialRetry(event) => event,
        _ => panic!("expected DialRetry event"),
    };
    assert_eq!(retry_event.attempt, 1);
    assert_eq!(retry_event.max_attempts, 4);

    let ping = PingMessage::new(7, Utc::now());
    session.send_ping(&ping).await.unwrap();

    let incoming = listener
        .next_event(Duration::from_millis(200))
        .await
        .expect("listener receives ping");
    let received_ping = match incoming {
        PeerEvent::Ping(p) => p,
        _ => panic!("expected ping event"),
    };

    let pong = PeerSession::make_pong(&received_ping);
    listener.send_pong(&pong).await.unwrap();

    let mut attempts = 0;
    loop {
        attempts += 1;
        let event = session
            .next_event(Duration::from_millis(200))
            .await
            .expect("dialer receives event");
        match event {
            PeerEvent::Pong(_) => break,
            PeerEvent::Ping(ping) => {
                let response = PeerSession::make_pong(&ping);
                listener.send_pong(&response).await.unwrap();
            }
            PeerEvent::DialRetry(_) => continue,
        }
        assert!(attempts < 5, "unexpected number of events before pong");
    }
}

#[tokio::test]
async fn dial_exhausts_retries_on_unreachable_peer() {
    let local_port = match reserve_loopback_port() {
        Ok(port) => port,
        Err(err) if err.kind() == ErrorKind::PermissionDenied => return,
        Err(_) => return,
    };

    let blocker = match UdpSocket::bind(("127.0.0.1", local_port)) {
        Ok(socket) => socket,
        Err(err) if err.kind() == ErrorKind::PermissionDenied => return,
        Err(_) => return,
    };

    let dial_config = SessionConfig::new(
        format!("127.0.0.1:{local_port}").parse().unwrap(),
        Keypair::generate(),
    )
    .with_max_retries(2)
    .with_retry_backoff_ms(10);

    let unreachable = PeerAddress::new(format!(
        "/ip4/127.0.0.1/udp/65500/quic-v1/p2p/{}",
        Keypair::generate().peer_id()
    ));

    let result = PeerSession::dial(dial_config, &unreachable).await;
    drop(blocker);

    match result {
        Err(PeerError::DialAttemptsExhausted { attempts, last_error }) => {
            assert_eq!(attempts, 3);
            assert!(matches!(*last_error, PeerError::Io(ref err) if matches!(err.kind(), ErrorKind::AddrInUse | ErrorKind::AddrNotAvailable)));
        }
        Err(PeerError::Io(err)) if err.kind() == ErrorKind::PermissionDenied => return,
        Ok(_) => panic!("dial unexpectedly succeeded"),
        Err(err) => panic!("unexpected error: {err}"),
    }
}
