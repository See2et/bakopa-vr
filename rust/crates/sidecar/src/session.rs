use std::collections::VecDeque;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use chrono::Utc;
use tokio::net::UdpSocket;
use tokio::sync::Mutex as AsyncMutex;
use tokio::time::{sleep, timeout, Duration, Instant};

use shared::{
    decode_ping, decode_pong, encode_ping, encode_pong, PeerAddress, PingMessage, PongMessage,
    RttReport, SessionConfig,
};

use crate::error::PeerError;

const MAX_DATAGRAM_LEN: usize = 2048;

#[derive(Clone)]
pub struct PeerSession {
    role: Role,
    inner: Arc<SessionInner>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Listener,
    Dialer,
}

enum WireMessage {
    Ping(Vec<u8>),
    Pong(Vec<u8>),
}

#[derive(Debug, Clone)]
pub struct DialRetryEvent {
    pub peer: String,
    pub attempt: u8,
    pub max_attempts: u8,
    pub backoff_ms: u64,
    pub elapsed_ms: u128,
    pub error: String,
}

struct SessionInner {
    config: SessionConfig,
    socket: Arc<UdpSocket>,
    remote_addr: AsyncMutex<Option<SocketAddr>>,
    attempts: AtomicU8,
    events: AsyncMutex<VecDeque<PeerEvent>>,
}

impl SessionInner {
    fn new(
        config: SessionConfig,
        socket: Arc<UdpSocket>,
        remote_addr: Option<SocketAddr>,
        initial_events: VecDeque<PeerEvent>,
    ) -> Self {
        Self {
            config,
            socket,
            remote_addr: AsyncMutex::new(remote_addr),
            attempts: AtomicU8::new(0),
            events: AsyncMutex::new(initial_events),
        }
    }

    fn attempts(&self) -> u8 {
        self.attempts.load(Ordering::Relaxed)
    }

    fn set_attempts(&self, attempts: u8) {
        self.attempts.store(attempts, Ordering::Relaxed);
    }

    async fn pop_event(&self) -> Option<PeerEvent> {
        let mut guard = self.events.lock().await;
        guard.pop_front()
    }
}

impl PeerSession {
    pub async fn listen(mut config: SessionConfig) -> Result<(PeerSession, PeerAddress), PeerError> {
        let socket = Arc::new(UdpSocket::bind(config.listen_addr).await.map_err(map_io_error)?);
        let local_addr = socket.local_addr().map_err(map_io_error)?;
        config.listen_addr = local_addr;

        let inner = Arc::new(SessionInner::new(
            config.clone(),
            socket,
            None,
            VecDeque::new(),
        ));
        let session = PeerSession {
            role: Role::Listener,
            inner,
        };
        Ok((session, config.advertised_multiaddr()))
    }

    pub async fn dial(mut config: SessionConfig, peer_addr: &PeerAddress) -> Result<PeerSession, PeerError> {
        let remote_addr = peer_addr
            .to_socket_addr()
            .ok_or_else(|| PeerError::InvalidMultiaddr(peer_addr.as_str().to_string()))?;
        peer_addr
            .peer_id()
            .ok_or_else(|| PeerError::InvalidMultiaddr(peer_addr.as_str().to_string()))?;

        let mut pending_events = VecDeque::new();
        let (socket, local_addr, attempt) = retry_bind_and_connect(
            config.listen_addr,
            remote_addr,
            config.max_retries,
            config.retry_backoff_ms,
            peer_addr.as_str(),
            |event| pending_events.push_back(event),
        )
        .await?;
        config.listen_addr = local_addr;

        let inner = Arc::new(SessionInner::new(
            config.clone(),
            socket,
            Some(remote_addr),
            pending_events,
        ));
        inner.set_attempts(attempt);

        Ok(PeerSession {
            role: Role::Dialer,
            inner,
        })
    }

    pub async fn send_ping(&self, ping: &PingMessage) -> Result<(), PeerError> {
        let payload = encode_ping(ping)?;
        self.send_wire(WireMessage::Ping(payload)).await
    }

    pub async fn send_pong(&self, pong: &PongMessage) -> Result<(), PeerError> {
        let payload = encode_pong(pong)?;
        self.send_wire(WireMessage::Pong(payload)).await
    }

    pub async fn next_event(&self, timeout_at: Duration) -> Result<PeerEvent, PeerError> {
        if let Some(event) = self.inner.pop_event().await {
            return Ok(event);
        }

        let mut buffer = vec![0u8; MAX_DATAGRAM_LEN];
        let message = match self.role {
            Role::Dialer => {
                let recv = timeout(timeout_at, self.inner.socket.recv(&mut buffer)).await;
                let len = match recv {
                    Ok(Ok(len)) => len,
                    Ok(Err(err)) => return Err(map_io_error(err)),
                    Err(_) => return Err(PeerError::Timeout(timeout_at)),
                };
                decode_wire(&buffer[..len])?
            }
            Role::Listener => {
                let recv = timeout(timeout_at, self.inner.socket.recv_from(&mut buffer)).await;
                let (len, remote) = match recv {
                    Ok(Ok(value)) => value,
                    Ok(Err(err)) => return Err(map_io_error(err)),
                    Err(_) => return Err(PeerError::Timeout(timeout_at)),
                };
                {
                    let mut guard = self.inner.remote_addr.lock().await;
                    *guard = Some(remote);
                }
                decode_wire(&buffer[..len])?
            }
        };

        match message {
            WireMessage::Ping(bytes) => {
                let ping = decode_ping(&bytes)?;
                Ok(PeerEvent::Ping(ping))
            }
            WireMessage::Pong(bytes) => {
                let pong = decode_pong(&bytes)?;
                Ok(PeerEvent::Pong(pong))
            }
        }
    }

    pub fn attempts(&self) -> u8 {
        self.inner.attempts()
    }

    pub fn config(&self) -> SessionConfig {
        self.inner.config.clone()
    }

    async fn send_wire(&self, message: WireMessage) -> Result<(), PeerError> {
        let mut frame = Vec::with_capacity(1 + match &message {
            WireMessage::Ping(bytes) | WireMessage::Pong(bytes) => bytes.len(),
        });
        let prefix = match &message {
            WireMessage::Ping(_) => 0u8,
            WireMessage::Pong(_) => 1u8,
        };
        frame.push(prefix);
        match message {
            WireMessage::Ping(bytes) | WireMessage::Pong(bytes) => frame.extend_from_slice(&bytes),
        }

        let send_result = match self.role {
            Role::Dialer => self.inner.socket.send(&frame).await,
            Role::Listener => {
                let remote = {
                    let guard = self.inner.remote_addr.lock().await;
                    *guard
                };
                let remote = remote.ok_or(PeerError::TransportNotReady)?;
                self.inner.socket.send_to(&frame, remote).await
            }
        };

        match send_result {
            Ok(_) => Ok(()),
            Err(err) => Err(map_io_error(err)),
        }
    }

    pub fn make_pong(ping: &PingMessage) -> PongMessage {
        PongMessage::new(ping.sequence, ping.sent_at, Utc::now())
    }

    pub fn rtt_report(&self, ping: &PingMessage, pong: &PongMessage) -> RttReport {
        let attempts = self.attempts().max(1);
        RttReport::from_messages(ping, pong, attempts)
    }
}

async fn retry_bind_and_connect<F>(
    listen_addr: SocketAddr,
    remote_addr: SocketAddr,
    max_retries: u8,
    backoff_ms: u64,
    peer_label: &str,
    mut emit: F,
) -> Result<(Arc<UdpSocket>, SocketAddr, u8), PeerError>
where
    F: FnMut(PeerEvent),
{
    let total_allowed = max_retries.saturating_add(1);
    let mut attempt: u8 = 0;
    let start = Instant::now();

    loop {
        attempt = attempt.saturating_add(1);
        match bind_and_connect(listen_addr, remote_addr).await {
            Ok((socket, local_addr)) => return Ok((socket, local_addr, attempt)),
            Err(err) => {
                if !should_retry_dial(&err) {
                    return Err(err);
                }

                if attempt >= total_allowed {
                    return Err(PeerError::DialAttemptsExhausted {
                        attempts: attempt,
                        last_error: Box::new(err),
                    });
                }

                emit(PeerEvent::DialRetry(DialRetryEvent {
                    peer: peer_label.to_string(),
                    attempt,
                    max_attempts: total_allowed,
                    backoff_ms,
                    elapsed_ms: start.elapsed().as_millis(),
                    error: err.to_string(),
                }));

                sleep(Duration::from_millis(backoff_ms)).await;
            }
        }
    }
}

async fn bind_and_connect(
    listen_addr: SocketAddr,
    remote_addr: SocketAddr,
) -> Result<(Arc<UdpSocket>, SocketAddr), PeerError> {
    let socket = UdpSocket::bind(listen_addr).await.map_err(map_io_error)?;

    if let Err(err) = socket.connect(remote_addr).await {
        return Err(map_io_error(err));
    }

    let socket = Arc::new(socket);
    let local_addr = socket.local_addr().map_err(map_io_error)?;

    Ok((socket, local_addr))
}

fn should_retry_dial(error: &PeerError) -> bool {
    match error {
        PeerError::ChannelClosed | PeerError::Timeout(_) => true,
        PeerError::Io(err) => matches!(
            err.kind(),
            ErrorKind::ConnectionRefused
                | ErrorKind::ConnectionReset
                | ErrorKind::ConnectionAborted
                | ErrorKind::TimedOut
                | ErrorKind::Interrupted
                | ErrorKind::NotConnected
                | ErrorKind::NetworkUnreachable
                | ErrorKind::HostUnreachable
                | ErrorKind::AddrInUse
                | ErrorKind::AddrNotAvailable
        ),
        _ => false,
    }
}

fn decode_wire(bytes: &[u8]) -> Result<WireMessage, PeerError> {
    let (prefix, payload) = bytes
        .split_first()
        .ok_or_else(|| PeerError::Decoding("empty datagram".to_string()))?;
    match prefix {
        0 => Ok(WireMessage::Ping(payload.to_vec())),
        1 => Ok(WireMessage::Pong(payload.to_vec())),
        _ => Err(PeerError::Decoding("unknown message prefix".to_string())),
    }
}

fn map_io_error(err: std::io::Error) -> PeerError {
    if err.kind() == std::io::ErrorKind::ConnectionRefused {
        PeerError::ChannelClosed
    } else {
        PeerError::Io(err)
    }
}

pub enum PeerEvent {
    DialRetry(DialRetryEvent),
    Ping(PingMessage),
    Pong(PongMessage),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::ErrorKind;
    use chrono::Utc;
    use shared::{Keypair, PeerAddress};
    use tokio::time::Duration;

    #[tokio::test]
    async fn dial_and_exchange_messages() {
        let mut listener_config = SessionConfig::default();
        listener_config.listen_addr = "127.0.0.1:0".parse().unwrap();
        listener_config.keypair = Keypair::generate();
        let (listener, addr) = match PeerSession::listen(listener_config).await {
            Ok(value) => value,
            Err(PeerError::Io(err)) if err.kind() == ErrorKind::PermissionDenied => return,
            Err(err) => panic!("unexpected listen error: {err}"),
        };

        let mut dialer_config = SessionConfig::default();
        dialer_config.listen_addr = "127.0.0.1:0".parse().unwrap();
        dialer_config.keypair = Keypair::generate();
        let dialer = match PeerSession::dial(dialer_config, &addr).await {
            Ok(session) => session,
            Err(PeerError::Io(err)) if err.kind() == ErrorKind::PermissionDenied => return,
            Err(err) => panic!("unexpected dial error: {err}"),
        };

        let ping = PingMessage::new(1, Utc::now());
        dialer.send_ping(&ping).await.unwrap();

        let event = listener
            .next_event(Duration::from_millis(500))
            .await
            .unwrap();
        let received_ping = match event {
            PeerEvent::Ping(p) => p,
            _ => panic!("expected ping"),
        };
        assert_eq!(received_ping.sequence, ping.sequence);

        let pong = PongMessage::new(ping.sequence, received_ping.sent_at, Utc::now());
        listener.send_pong(&pong).await.unwrap();

        let event = dialer
            .next_event(Duration::from_millis(500))
            .await
            .unwrap();
        let received_pong = match event {
            PeerEvent::Pong(p) => p,
            _ => panic!("expected pong"),
        };
        assert_eq!(received_pong.sequence, ping.sequence);

        let report = dialer.rtt_report(&ping, &received_pong);
        assert_eq!(report.sequence, ping.sequence);
    }

    #[tokio::test]
    async fn dial_unknown_listener_times_out() {
        let mut config = SessionConfig::default();
        config.listen_addr = "127.0.0.1:0".parse().unwrap();
        config.keypair = Keypair::generate();
        let addr = PeerAddress::new("/ip4/127.0.0.1/udp/59999/quic-v1/p2p/unknown");

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
}
