use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
};

use chrono::Utc;
use tokio::{
    net::UdpSocket,
    sync::Mutex as AsyncMutex,
    time::{timeout, Duration},
};

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

struct SessionInner {
    config: SessionConfig,
    socket: Arc<UdpSocket>,
    remote_addr: AsyncMutex<Option<SocketAddr>>,
    attempts: AtomicU8,
}

impl SessionInner {
    fn new(config: SessionConfig, socket: Arc<UdpSocket>, remote_addr: Option<SocketAddr>) -> Self {
        Self {
            config,
            socket,
            remote_addr: AsyncMutex::new(remote_addr),
            attempts: AtomicU8::new(0),
        }
    }

    fn attempts(&self) -> u8 {
        self.attempts.load(Ordering::Relaxed)
    }

    fn increment_attempts(&self) -> u8 {
        self.attempts.fetch_add(1, Ordering::Relaxed) + 1
    }
}

impl PeerSession {
    pub async fn listen(mut config: SessionConfig) -> Result<(PeerSession, PeerAddress), PeerError> {
        let socket = Arc::new(UdpSocket::bind(config.listen_addr).await.map_err(map_io_error)?);
        let local_addr = socket.local_addr().map_err(map_io_error)?;
        config.listen_addr = local_addr;

        let inner = Arc::new(SessionInner::new(config.clone(), socket, None));
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

        let socket = Arc::new(UdpSocket::bind(config.listen_addr).await.map_err(map_io_error)?);
        let local_addr = socket.local_addr().map_err(map_io_error)?;
        config.listen_addr = local_addr;
        socket.connect(remote_addr).await.map_err(map_io_error)?;

        let inner = Arc::new(SessionInner::new(config.clone(), Arc::clone(&socket), Some(remote_addr)));

        Ok(PeerSession {
            role: Role::Dialer,
            inner,
        })
    }

    pub async fn send_ping(&self, ping: &PingMessage) -> Result<(), PeerError> {
        let payload = encode_ping(ping)?;
        self.inner.increment_attempts();
        self.send_wire(WireMessage::Ping(payload)).await
    }

    pub async fn send_pong(&self, pong: &PongMessage) -> Result<(), PeerError> {
        let payload = encode_pong(pong)?;
        self.send_wire(WireMessage::Pong(payload)).await
    }

    pub async fn next_event(&self, timeout_at: Duration) -> Result<PeerEvent, PeerError> {
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
