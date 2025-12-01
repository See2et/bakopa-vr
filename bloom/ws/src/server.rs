use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex as StdMutex};

use bloom_api::ServerToClient;
use bloom_core::ParticipantId;
use futures_util::{SinkExt, StreamExt};
use std::str::FromStr;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;
use tokio::time::{interval, Duration, Instant};
use tokio_tungstenite::accept_hdr_async;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::protocol::{frame::coding::CloseCode, CloseFrame, Message};
use tokio_tungstenite::WebSocketStream;

use crate::core_api::CoreApi;
use crate::handler::WsHandler;
use crate::sinks::{BroadcastSink, OutSink};

type WsSink = futures_util::stream::SplitSink<WebSocketStream<TcpStream>, Message>;
type WsStream = futures_util::stream::SplitStream<WebSocketStream<TcpStream>>;
type SharedSink = Arc<Mutex<WsSink>>;
type PeerMap = Arc<Mutex<HashMap<ParticipantId, SharedSink>>>;

pub const ABNORMAL_DISCONNECT_GRACE: Duration = Duration::from_secs(5);

#[derive(Clone, Debug)]
struct PingConfig {
    interval: Duration,
    miss_allowed: u32,
}

impl Default for PingConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(30),
            miss_allowed: 2,
        }
    }
}

/// Shared CoreApi wrapper using a blocking mutex so that CoreApi remains synchronous.
pub struct SharedCore<C> {
    inner: Arc<StdMutex<C>>,
}

impl<C> SharedCore<C> {
    pub fn new(inner: C) -> Self {
        Self {
            inner: Arc::new(StdMutex::new(inner)),
        }
    }

    /// Construct from an existing Arc<Mutex<C>> (mainly for tests to inspect state).
    pub fn from_arc(inner: Arc<StdMutex<C>>) -> Self {
        Self { inner }
    }

    pub fn inner_arc(&self) -> Arc<StdMutex<C>> {
        self.inner.clone()
    }
}

impl<C> Clone for SharedCore<C> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<C: CoreApi> CoreApi for SharedCore<C> {
    fn create_room(&mut self, room_owner: ParticipantId) -> bloom_core::CreateRoomResult {
        self.inner
            .lock()
            .expect("core lock poisoned")
            .create_room(room_owner)
    }

    fn join_room(
        &mut self,
        room_id: &bloom_core::RoomId,
        participant: ParticipantId,
    ) -> Option<Result<Vec<ParticipantId>, bloom_core::JoinRoomError>> {
        self.inner
            .lock()
            .expect("core lock poisoned")
            .join_room(room_id, participant)
    }

    fn leave_room(
        &mut self,
        room_id: &bloom_core::RoomId,
        participant: &ParticipantId,
    ) -> Option<Vec<ParticipantId>> {
        self.inner
            .lock()
            .expect("core lock poisoned")
            .leave_room(room_id, participant)
    }

    fn participants(&self, room_id: &bloom_core::RoomId) -> Option<Vec<ParticipantId>> {
        self.inner
            .lock()
            .expect("core lock poisoned")
            .participants(room_id)
    }

    fn relay_offer(
        &mut self,
        room_id: &bloom_core::RoomId,
        from: &ParticipantId,
        to: &ParticipantId,
        payload: bloom_api::RelaySdp,
    ) -> Result<(), bloom_api::ErrorCode> {
        self.inner
            .lock()
            .expect("core lock poisoned")
            .relay_offer(room_id, from, to, payload)
    }

    fn relay_answer(
        &mut self,
        room_id: &bloom_core::RoomId,
        from: &ParticipantId,
        to: &ParticipantId,
        payload: bloom_api::RelaySdp,
    ) -> Result<(), bloom_api::ErrorCode> {
        self.inner
            .lock()
            .expect("core lock poisoned")
            .relay_answer(room_id, from, to, payload)
    }

    fn relay_ice_candidate(
        &mut self,
        room_id: &bloom_core::RoomId,
        from: &ParticipantId,
        to: &ParticipantId,
        payload: bloom_api::RelayIce,
    ) -> Result<(), bloom_api::ErrorCode> {
        self.inner
            .lock()
            .expect("core lock poisoned")
            .relay_ice_candidate(room_id, from, to, payload)
    }
}

/// Real WebSocket out-sink that serializes JSON and sends over the WS connection.
pub struct WebSocketOutSink {
    sink: SharedSink,
}

impl WebSocketOutSink {
    pub fn new(sink: SharedSink) -> Self {
        Self { sink }
    }
}

impl OutSink for WebSocketOutSink {
    fn send(&mut self, message: ServerToClient) {
        let sink = self.sink.clone();
        if let Ok(text) = serde_json::to_string(&message) {
            tokio::spawn(async move {
                let mut guard = sink.lock().await;
                let _ = guard.send(Message::Text(text)).await;
            });
        }
    }
}

/// Broadcast hub backed by participant_id -> sink map.
#[derive(Clone)]
pub struct WebSocketBroadcast {
    peers: PeerMap,
}

impl WebSocketBroadcast {
    pub fn new(peers: PeerMap) -> Self {
        Self { peers }
    }

    pub async fn insert(&self, participant: ParticipantId, sink: SharedSink) {
        let mut map = self.peers.lock().await;
        if let Some(old) = map.insert(participant, sink.clone()) {
            // 仕様: 同一participantの多重接続時は旧接続を優先的に切断する
            tokio::spawn(async move {
                let mut guard = old.lock().await;
                let _ = guard
                    .send(Message::Close(Some(CloseFrame {
                        code: CloseCode::Normal,
                        reason: "duplicate connection".into(),
                    })))
                    .await;
            });
        }
    }

    pub async fn remove(&self, participant: &ParticipantId) {
        let mut map = self.peers.lock().await;
        map.remove(participant);
    }

    /// 現在登録されているsinkが指定sinkと同一の場合のみ削除する（重複接続の新旧判定に使用）。
    pub async fn remove_if_same(&self, participant: &ParticipantId, sink: &SharedSink) {
        let mut map = self.peers.lock().await;
        if let Some(current) = map.get(participant) {
            if Arc::ptr_eq(current, sink) {
                map.remove(participant);
            }
        }
    }
}

impl BroadcastSink for WebSocketBroadcast {
    fn send_to(&mut self, to: &ParticipantId, message: ServerToClient) {
        let peers = self.peers.clone();
        let to = to.clone();
        if let Ok(text) = serde_json::to_string(&message) {
            tokio::spawn(async move {
                if let Some(sink) = peers.lock().await.get(&to).cloned() {
                    let mut guard = sink.lock().await;
                    let _ = guard.send(Message::Text(text)).await;
                }
            });
        }
    }
}

pub struct WsServerHandle {
    pub addr: SocketAddr,
    shutdown_tx: oneshot::Sender<()>,
    join_handle: JoinHandle<()>,
}

impl WsServerHandle {
    pub async fn shutdown(self) {
        let _ = self.shutdown_tx.send(());
        let _ = self.join_handle.await;
    }
}

/// Start a WebSocket server bound to the given address. Returns the bound address and a handle for shutdown.
pub async fn start_ws_server<C>(
    bind_addr: SocketAddr,
    core: SharedCore<C>,
) -> anyhow::Result<WsServerHandle>
where
    C: CoreApi + Send + 'static,
{
    let listener = TcpListener::bind(bind_addr).await?;
    let local_addr = listener.local_addr()?;

    let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
    let shared_core = core;
    let peers: PeerMap = Arc::new(Mutex::new(HashMap::new()));

    let join_handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut shutdown_rx => {
                    break;
                }
                accept_res = listener.accept() => {
                    let (stream, _addr) = match accept_res {
                        Ok(s) => s,
                        Err(_) => continue,
                    };
                    let core = shared_core.clone();
                    let peers = peers.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(stream, core, peers).await {
                            tracing::warn!(error=%e, "ws connection error");
                        }
                    });
                }
            }
        }
    });

    Ok(WsServerHandle {
        addr: local_addr,
        shutdown_tx,
        join_handle,
    })
}

async fn handle_connection<C>(
    stream: TcpStream,
    core: SharedCore<C>,
    peers: PeerMap,
) -> anyhow::Result<()>
where
    C: CoreApi + Send + 'static,
{
    let participant_id = std::env::var("BLOOM_TEST_PARTICIPANT_ID")
        .ok()
        .and_then(|v| ParticipantId::from_str(&v).ok())
        .unwrap_or_else(ParticipantId::new);
    let span = tracing::info_span!("ws_handshake", participant_id = %participant_id);
    let _enter = span.enter();

    let callback = |req: &Request, resp: Response| {
        if req.uri().path() != "/ws" {
            let resp = Response::builder()
                .status(StatusCode::UPGRADE_REQUIRED)
                .header("Upgrade", "websocket")
                .header("Connection", "Upgrade")
                .body(None)
                .expect("build 426 response");
            Err(resp)
        } else {
            Ok(resp)
        }
    };

    // WS handshake (only /ws is allowed)
    let ws_stream = accept_hdr_async(stream, callback).await?;
    let (sink, mut stream) = ws_stream.split();
    let sink = Arc::new(Mutex::new(sink));

    let out_sink = WebSocketOutSink::new(sink.clone());
    let broadcast = WebSocketBroadcast::new(peers.clone());
    broadcast.insert(participant_id.clone(), sink.clone()).await;

    // room_id は CreateRoom/JoinRoom で設定される前提
    let mut handler = WsHandler::new(core, participant_id.clone(), out_sink, broadcast.clone());
    handler.perform_handshake().await;

    let reason = process_messages(
        &mut handler,
        sink.clone(),
        &mut stream,
        PingConfig::default(),
    )
    .await;
    handle_disconnect(
        &mut handler,
        &peers,
        &broadcast,
        &participant_id,
        reason,
        sink.clone(),
    )
    .await;

    Ok(())
}

async fn process_messages<C>(
    handler: &mut WsHandler<SharedCore<C>, WebSocketOutSink, WebSocketBroadcast>,
    sink: SharedSink,
    stream: &mut WsStream,
    ping_cfg: PingConfig,
) -> DisconnectReason
where
    C: CoreApi + Send + 'static,
{
    let mut last_pong = Instant::now();
    let mut ping_timer = interval(ping_cfg.interval);
    ping_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    let mut reason: Option<DisconnectReason> = None;
    loop {
        tokio::select! {
            maybe_msg = stream.next() => {
                match maybe_msg {
                    Some(Ok(Message::Close(_))) => {
                        reason = Some(DisconnectReason::Normal);
                        break;
                    }
                    Some(Ok(Message::Text(text))) => {
                        handler.handle_text_message(&text).await;
                    }
                    Some(Ok(Message::Pong(_))) => {
                        last_pong = Instant::now();
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        let _ = sink.lock().await.send(Message::Pong(payload)).await;
                    }
                    Some(Ok(_)) => {
                        // 非テキスト（バイナリ等）はシグナリング外=メディア誤送信扱い。
                        // leave_roomを発火させないためroom_idをクリアし、1003 Closeで拒否して中継しない。
                        handler.room_id = None;
                        let _ = sink
                            .lock()
                            .await
                            .send(Message::Close(Some(
                                tokio_tungstenite::tungstenite::protocol::CloseFrame {
                                    code: tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::Unsupported,
                                    reason: "unsupported data".into(),
                                },
                            )))
                            .await;
                        reason = Some(DisconnectReason::Abnormal);
                        break;
                    }
                    Some(Err(_)) => {
                        reason = Some(DisconnectReason::Abnormal);
                        break;
                    }
                    None => {
                        reason = Some(DisconnectReason::Abnormal);
                        break;
                    }
                }
            }
            _ = ping_timer.tick() => {
                let _ = sink.lock().await.send(Message::Ping(Vec::new())).await;
                if last_pong.elapsed() >= ping_cfg.interval * ping_cfg.miss_allowed {
                    let _ = sink
                        .lock()
                        .await
                        .send(Message::Close(Some(CloseFrame {
                            code: PING_TIMEOUT_CLOSE_CODE,
                            reason: "ping timeout".into(),
                        })))
                        .await;
                    reason = Some(DisconnectReason::Abnormal);
                    break;
                }
            }
        }
    }
    reason.unwrap_or(DisconnectReason::Abnormal)
}

async fn handle_disconnect<C>(
    handler: &mut WsHandler<SharedCore<C>, WebSocketOutSink, WebSocketBroadcast>,
    peers: &PeerMap,
    broadcast: &WebSocketBroadcast,
    participant_id: &ParticipantId,
    reason: DisconnectReason,
    sink: SharedSink,
) where
    C: CoreApi + Send + 'static,
{
    if matches!(reason, DisconnectReason::Abnormal) {
        tokio::time::sleep(ABNORMAL_DISCONNECT_GRACE).await;
    }
    let remaining = remaining_peers(peers, participant_id).await;
    handler.handle_abnormal_close(&remaining).await;
    broadcast.remove_if_same(participant_id, &sink).await;
}

async fn remaining_peers(peers: &PeerMap, exclude: &ParticipantId) -> Vec<ParticipantId> {
    let map = peers.lock().await;
    map.keys().filter(|pid| *pid != exclude).cloned().collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DisconnectReason {
    Normal,
    Abnormal,
}

/// Ping/Pong途絶時に送信するCloseCode。1006(Abnormal)は禁止されているためAway(1001)を用いる。
pub const PING_TIMEOUT_CLOSE_CODE: tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode =
    tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::Away;
