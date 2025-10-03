use std::{collections::HashMap, sync::{Arc, atomic::{AtomicU8, Ordering}}};

use chrono::Utc;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use tokio::sync::{mpsc, Mutex as AsyncMutex};
use tokio::time::{timeout, Duration};

use shared::{decode_ping, decode_pong, encode_ping, encode_pong, PeerAddress, PingMessage, PongMessage, RttReport, SessionConfig};

use crate::error::PeerError;

static LISTENERS: Lazy<Mutex<HashMap<String, Arc<SessionInner>>>> = Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Clone)]
struct PeerTransport {
    outbound: mpsc::Sender<WireMessage>,
    inbound: Arc<AsyncMutex<mpsc::Receiver<WireMessage>>>,
}

impl PeerTransport {
    fn new(outbound: mpsc::Sender<WireMessage>, inbound: mpsc::Receiver<WireMessage>) -> Self {
        Self {
            outbound,
            inbound: Arc::new(AsyncMutex::new(inbound)),
        }
    }
}

struct SessionInner {
    config: SessionConfig,
    peer_id: String,
    transport: AsyncMutex<Option<PeerTransport>>,
    attempts: AtomicU8,
}

impl SessionInner {
    fn new(config: SessionConfig, peer_id: String) -> Self {
        Self {
            config,
            peer_id,
            transport: AsyncMutex::new(None),
            attempts: AtomicU8::new(0),
        }
    }

    async fn set_transport(&self, transport: PeerTransport) {
        let mut guard = self.transport.lock().await;
        *guard = Some(transport);
        self.attempts.store(0, Ordering::Relaxed);
    }

    fn peer_id(&self) -> &str {
        &self.peer_id
    }

    fn attempts(&self) -> u8 {
        self.attempts.load(Ordering::Relaxed)
    }

    fn increment_attempts(&self) -> u8 {
        self.attempts.fetch_add(1, Ordering::Relaxed) + 1
    }
}

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

pub enum PeerEvent {
    Ping(PingMessage),
    Pong(PongMessage),
}

#[derive(Clone)]
enum WireMessage {
    Ping(Vec<u8>),
    Pong(Vec<u8>),
}

impl PeerSession {
    pub async fn listen(config: SessionConfig) -> Result<(PeerSession, PeerAddress), PeerError> {
        let peer_id = config.keypair.peer_id();
        let inner = Arc::new(SessionInner::new(config.clone(), peer_id.clone()));
        {
            let mut registry = LISTENERS.lock();
            if registry.contains_key(&peer_id) {
                return Err(PeerError::ListenerAlreadyRegistered { peer_id });
            }
            registry.insert(peer_id.clone(), inner.clone());
        }
        Ok((PeerSession { role: Role::Listener, inner }, config.advertised_multiaddr()))
    }

    pub async fn dial(config: SessionConfig, peer_addr: &PeerAddress) -> Result<PeerSession, PeerError> {
        let peer_id = peer_addr
            .peer_id()
            .ok_or_else(|| PeerError::InvalidMultiaddr(peer_addr.as_str().to_string()))?
            .to_string();
        let listener_inner = {
            let registry = LISTENERS.lock();
            registry.get(&peer_id).cloned().ok_or_else(|| PeerError::ListenerNotFound { peer_id: peer_id.clone() })?
        };

        let (listener_inbound_tx, dialer_inbound_rx) = mpsc::channel(64);
        let (dialer_inbound_tx, listener_inbound_rx) = mpsc::channel(64);

        let listener_transport = PeerTransport::new(listener_inbound_tx, listener_inbound_rx);
        listener_inner.set_transport(listener_transport).await;

        let dialer_inner = Arc::new(SessionInner {
            config,
            peer_id: listener_inner.peer_id().to_string(),
            transport: AsyncMutex::new(Some(PeerTransport::new(dialer_inbound_tx, dialer_inbound_rx))),
            attempts: AtomicU8::new(0),
        });

        Ok(PeerSession { role: Role::Dialer, inner: dialer_inner })
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
        let transport = self.transport().await?;
        let mut receiver = transport.inbound.lock().await;
        let result = timeout(timeout_at, receiver.recv()).await.map_err(|_| PeerError::Timeout(timeout_at))?;
        match result {
            Some(WireMessage::Ping(bytes)) => {
                let ping = decode_ping(&bytes)?;
                Ok(PeerEvent::Ping(ping))
            }
            Some(WireMessage::Pong(bytes)) => {
                let pong = decode_pong(&bytes)?;
                Ok(PeerEvent::Pong(pong))
            }
            None => Err(PeerError::ChannelClosed),
        }
    }

    pub fn attempts(&self) -> u8 {
        self.inner.attempts()
    }

    pub fn config(&self) -> SessionConfig {
        self.inner.config.clone()
    }

    async fn transport(&self) -> Result<PeerTransport, PeerError> {
        let guard = self.inner.transport.lock().await;
        guard.clone().ok_or(PeerError::TransportNotReady)
    }

    async fn send_wire(&self, message: WireMessage) -> Result<(), PeerError> {
        let transport = self.transport().await?;
        transport
            .outbound
            .send(message)
            .await
            .map_err(|_| PeerError::ChannelClosed)
    }

    pub fn make_pong(ping: &PingMessage) -> PongMessage {
        PongMessage::new(ping.sequence, ping.sent_at, Utc::now())
    }

    pub fn rtt_report(&self, ping: &PingMessage, pong: &PongMessage) -> RttReport {
        let attempts = self.attempts().max(1);
        RttReport::from_messages(ping, pong, attempts)
    }
}

impl Drop for PeerSession {
    fn drop(&mut self) {
        if matches!(self.role, Role::Listener) {
            let mut registry = LISTENERS.lock();
            registry.retain(|_, inner| !Arc::ptr_eq(inner, &self.inner));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use shared::{Keypair, PeerAddress, PingMessage, PongMessage, SessionConfig};
    use tokio::time::Duration;

    #[tokio::test]
    async fn dial_and_exchange_messages() {
        let mut listener_config = SessionConfig::default();
        listener_config.listen_addr = "127.0.0.1:4000".parse().unwrap();
        listener_config.keypair = Keypair::generate();
        let (listener, addr) = PeerSession::listen(listener_config).await.unwrap();

        let mut dialer_config = SessionConfig::default();
        dialer_config.keypair = Keypair::generate();
        let dialer = PeerSession::dial(dialer_config, &addr).await.unwrap();

        let ping = PingMessage::new(1, Utc::now());
        dialer.send_ping(&ping).await.unwrap();

        let event = listener.next_event(Duration::from_millis(100)).await.unwrap();
        let received_ping = match event {
            PeerEvent::Ping(p) => p,
            _ => panic!("expected ping"),
        };
        assert_eq!(received_ping.sequence, ping.sequence);

        let pong = PongMessage::new(ping.sequence, received_ping.sent_at, Utc::now());
        listener.send_pong(&pong).await.unwrap();

        let event = dialer.next_event(Duration::from_millis(100)).await.unwrap();
        let received_pong = match event {
            PeerEvent::Pong(p) => p,
            _ => panic!("expected pong"),
        };
        assert_eq!(received_pong.sequence, ping.sequence);

        let report = dialer.rtt_report(&ping, &received_pong);
        assert_eq!(report.sequence, ping.sequence);
    }

    #[tokio::test]
    async fn dial_unknown_listener_fails() {
        let config = SessionConfig::default();
        let addr = PeerAddress::new("/ip4/127.0.0.1/udp/4001/quic-v1/p2p/unknown");
        let result = PeerSession::dial(config, &addr).await;
        assert!(matches!(result, Err(PeerError::ListenerNotFound { .. })));
    }
}
