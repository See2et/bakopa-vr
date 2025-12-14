pub mod messages;
pub mod participant_table;
pub mod rate_limiter;
pub mod router;
pub mod signaling_adapter;
pub mod transport_inbox;
pub mod webrtc_transport;

pub use crate::messages::{ChatMessage, ControlMessage, PoseMessage as Pose, PoseTransform};
pub use crate::router::{Outbound, OutboundPayload, Router};
pub use crate::signaling_adapter::SignalingAdapter;
pub use crate::transport_inbox::TransportInbox;
pub use crate::participant_table::ParticipantTable;

use crate::messages::{SyncMessage, SyncMessageEnvelope, SyncMessageError};
use bloom_core::{ParticipantId, RoomId};
use serde::{Deserialize, Serialize};
use serde_json;
use std::str::FromStr;

/// Syncer全体のファサード。1リクエストに対して複数イベントを返す契約。
pub trait Syncer {
    fn handle(&mut self, request: SyncerRequest) -> Vec<SyncerEvent>;
}

/// WebRTC/DataChannel等の下位トランスポートを抽象化するためのtrait。
pub trait Transport {
    fn register_participant(&mut self, participant: ParticipantId);
    fn send(&mut self, to: ParticipantId, payload: TransportPayload, params: TransportSendParams);
    fn poll(&mut self) -> Vec<TransportEvent>;
}

/// 送信前に登録されたparticipantにだけ配送し、自分自身には配送しないトランスポートの薄いラッパ。
#[derive(Debug)]
pub struct FilteringTransport<T> {
    inner: T,
    me: ParticipantId,
    registered: bool,
}

impl<T: Transport> FilteringTransport<T> {
    pub fn new(me: ParticipantId, inner: T) -> Self {
        Self {
            inner,
            me,
            registered: false,
        }
    }

    /// 明示的に登録を行う。未登録のまま send した場合は無視される。
    pub fn register(&mut self) {
        self.registered = true;
        self.inner.register_participant(self.me.clone());
    }

    pub fn send(&mut self, to: ParticipantId, payload: TransportPayload, params: TransportSendParams) {
        if !self.registered {
            return; // 未登録の送信はドロップ
        }

        // 宛先の最終フィルタ（自分除外など）は inner 実装に委譲する。
        self.inner.send(to, payload, params);
    }

    pub fn poll(&mut self) -> Vec<TransportEvent> {
        self.inner.poll()
    }
}

/// Router・TransportInbox を組み合わせた最小Syncer実装。
pub struct BasicSyncer<T: Transport> {
    me: ParticipantId,
    transport: FilteringTransport<T>,
    participants: ParticipantTable,
    router: Router,
    inbox: TransportInbox,
    room: Option<RoomId>,
}

impl<T: Transport> BasicSyncer<T> {
    pub fn new(me: ParticipantId, transport: T) -> Self {
        Self {
            me: me.clone(),
            transport: FilteringTransport::new(me, transport),
            participants: ParticipantTable::new(),
            router: Router::new(),
            inbox: TransportInbox::new(),
            room: None,
        }
    }

    /// テスト用: 生のTransportPayloadを送信する（音声プレースホルダなど）。
    pub fn send_transport_payload(&mut self, to: ParticipantId, payload: TransportPayload) {
        let params = TransportSendParams::for_payload(&payload);
        self.transport.send(to, payload, params);
    }

    fn drain_transport_events(&mut self) -> Vec<SyncerEvent> {
        let mut aggregated = Vec::new();
        if let Some(room) = &self.room {
            for ev in self.transport.poll() {
                self.inbox.push(ev);
            }

            aggregated.extend(self.inbox.drain_into_events(room, &mut self.participants));
        }
        aggregated
    }
}

impl<T: Transport> Syncer for BasicSyncer<T> {
    fn handle(&mut self, request: SyncerRequest) -> Vec<SyncerEvent> {
        // 先に受信を取り込んでおく
        if let Some(room) = &self.room {
            for ev in self.transport.poll() {
                self.inbox.push(ev);
            }

            let inbound = self.inbox.drain_into_events(room, &mut self.participants);
            if !inbound.is_empty() {
                // inbound events are returned along with those produced by request handling
                let mut events = inbound;
                events.extend(self.handle_request(request));
                return events;
            }
        }

        self.handle_request(request)
    }
}

impl<T: Transport> BasicSyncer<T> {
    fn broadcast_control_join(&mut self, participant_id: &ParticipantId) {
        use crate::messages::{ControlMessage, ControlPayload, SyncMessageEnvelope};

        let control = ControlMessage::Join(ControlPayload {
            participant_id: participant_id.to_string(),
            reconnect_token: None,
            reason: None,
        });

        if let Ok(envelope) = SyncMessageEnvelope::from_control(control) {
            if let Ok(bytes) = serde_json::to_vec(&envelope) {
                let payload = TransportPayload::Bytes(bytes);
                let params = TransportSendParams::for_stream(StreamKind::ControlJoin);
                // broadcast: WebrtcTransport/BusTransport ignore `to` and deliver to peer set
                self.transport
                    .send(self.me.clone(), payload, params);
            }
        }
    }

    fn handle_request(&mut self, request: SyncerRequest) -> Vec<SyncerEvent> {
        let mut events = Vec::new();

        match request {
            SyncerRequest::Join {
                room_id,
                participant_id,
            } => {
                self.room = Some(room_id.clone());
                self.transport.register();
                self.broadcast_control_join(&participant_id);
                events.extend(self.participants.apply_join(participant_id.clone()));
                events.push(SyncerEvent::SelfJoined {
                    room_id,
                    participant_id,
                });
                // 参加処理と同一トランザクションで受信キューを捌き、既存参加者のJoin通知を取り込む。
                events.extend(self.drain_transport_events());
            }
            SyncerRequest::SendPose { from, pose, ctx } => {
                // 未参加者からの送信は無視（ログはTransport層に任せる）。
                if !self.participants.is_registered(&from) {
                    events.extend(self.drain_transport_events());
                    return events;
                }

                let outs = self.router.route_pose(&from, pose.clone(), &self.participants);

                for outbound in outs {
                    if let Ok(payload) = outbound.into_transport_payload() {
                        let params = TransportSendParams::for_stream(outbound.stream_kind);
                        self.transport
                            .send(outbound.to.clone(), payload, params);
                    }
                }

                events.extend(self.drain_transport_events());
                let _ = ctx;
            }
            SyncerRequest::SendChat { chat, ctx } => {
                if !self.participants.is_registered(&ctx.participant_id) {
                    events.extend(self.drain_transport_events());
                    return events;
                }

                let outs = self
                    .router
                    .route_chat(&ctx.participant_id, chat.clone(), &self.participants);

                for outbound in outs {
                    if let Ok(payload) = outbound.into_transport_payload() {
                        let params = TransportSendParams::for_stream(outbound.stream_kind);
                        self.transport
                            .send(outbound.to.clone(), payload, params);
                    }
                }

                events.extend(self.drain_transport_events());
                let _ = ctx;
            }
        }

        events
    }
}

/// WebRTC送信用のチャネル設定をStreamKindから導出するための型。
#[derive(Debug, Clone, PartialEq)]
pub enum TransportSendParams {
    /// DataChannelで送る場合の設定。
    DataChannel {
        /// ordered=true なら順序保証。
        ordered: bool,
        /// reliable=true なら再送あり。
        reliable: bool,
        /// 使用するDataChannelのlabel。
        label: &'static str,
    },
    /// AudioTrackで送る場合（Voice専用）。
    AudioTrack,
}

impl TransportSendParams {
    /// StreamKindに応じた送信チャネル設定を返す。
    pub fn for_stream(kind: StreamKind) -> Self {
        match kind {
            StreamKind::Pose => Self::DataChannel {
                ordered: false,
                reliable: false,
                label: "sutera-data",
            },
            StreamKind::Chat
            | StreamKind::ControlJoin
            | StreamKind::ControlLeave
            | StreamKind::SignalingOffer
            | StreamKind::SignalingAnswer
            | StreamKind::SignalingIce => Self::DataChannel {
                ordered: true,
                reliable: true,
                label: "sutera-data",
            },
            StreamKind::Voice => Self::AudioTrack,
        }
    }

    /// TransportPayloadから推定されるStreamKindに基づいてチャネル設定を返す。
    pub fn for_payload(payload: &TransportPayload) -> Self {
        match payload {
            TransportPayload::AudioFrame(_) => Self::AudioTrack,
            TransportPayload::Bytes(bytes) => {
                if let Ok(env) = SyncMessageEnvelope::from_slice(bytes) {
                    if let Ok(kind) = StreamKind::parse(env.kind.as_str()) {
                        return Self::for_stream(kind);
                    }
                }
                // フォールバックは安全側でordered/reliable
                Self::DataChannel {
                    ordered: true,
                    reliable: true,
                    label: "sutera-data",
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum TransportPayload {
    Bytes(Vec<u8>),
    AudioFrame(Vec<u8>),
}

impl TransportPayload {
    /// Parse the underlying bytes as a SyncMessageEnvelope.
    pub fn parse_envelope(&self) -> Result<SyncMessageEnvelope, SyncMessageError> {
        match self {
            TransportPayload::Bytes(bytes) => SyncMessageEnvelope::from_slice(bytes),
            TransportPayload::AudioFrame(_) => Err(SyncMessageError::UnknownKind {
                value: StreamKind::Voice.as_str().to_string(),
            }),
        }
    }

    pub fn parse_sync_message(&self) -> Result<SyncMessage, SyncMessageError> {
        match self {
            TransportPayload::Bytes(bytes) => {
                let envelope = SyncMessageEnvelope::from_slice(bytes)?;
                SyncMessage::from_envelope(envelope)
            }
            TransportPayload::AudioFrame(_) => Err(SyncMessageError::UnknownKind {
                value: StreamKind::Voice.as_str().to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone)]
pub enum TransportEvent {
    Received {
        from: ParticipantId,
        payload: TransportPayload,
    },
    /// 接続失敗やDTLS/ICEエラーなど、相手peerとの通信が成立しなかったことを示す。
    Failure {
        peer: ParticipantId,
    },
}

/// API入力モデル。
#[derive(Debug, Clone)]
pub enum SyncerRequest {
    Join {
        room_id: RoomId,
        participant_id: ParticipantId,
    },
    SendPose {
        from: ParticipantId,
        pose: Pose,
        ctx: TracingContext,
    },
    SendChat {
        chat: ChatMessage,
        ctx: TracingContext,
    },
}

/// API出力モデル。
#[derive(Debug, Clone, PartialEq)]
pub enum SyncerEvent {
    SelfJoined {
        room_id: RoomId,
        participant_id: ParticipantId,
    },
    PeerJoined {
        participant_id: ParticipantId,
    },
    PeerLeft {
        participant_id: ParticipantId,
    },
    PoseReceived {
        from: ParticipantId,
        pose: Pose,
        ctx: TracingContext,
    },
    ChatReceived {
        chat: ChatMessage,
        ctx: TracingContext,
    },
    VoiceFrameReceived {
        from: ParticipantId,
        frame: Vec<u8>,
        ctx: TracingContext,
    },
    Error {
        kind: SyncerError,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum SyncerError {
    InvalidParticipantId { raw_value: String },
    InvalidPayload(SyncMessageError),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TracingContext {
    pub room_id: RoomId,
    pub participant_id: ParticipantId,
    pub stream_kind: StreamKind,
}

impl TracingContext {
    /// Helper for chat events to ensure StreamKind::Chat is consistently applied.
    pub fn for_chat(room_id: &RoomId, participant_id: &ParticipantId) -> Self {
        Self {
            room_id: room_id.clone(),
            participant_id: participant_id.clone(),
            stream_kind: StreamKind::Chat,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamKind {
    #[serde(rename = "pose")]
    Pose,
    #[serde(rename = "chat")]
    Chat,
    #[serde(rename = "voice")]
    Voice,
    #[serde(rename = "control.join")]
    ControlJoin,
    #[serde(rename = "control.leave")]
    ControlLeave,
    #[serde(rename = "signaling.offer")]
    SignalingOffer,
    #[serde(rename = "signaling.answer")]
    SignalingAnswer,
    #[serde(rename = "signaling.ice")]
    SignalingIce,
}

impl StreamKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            StreamKind::Pose => "pose",
            StreamKind::Chat => "chat",
            StreamKind::Voice => "voice",
            StreamKind::ControlJoin => "control.join",
            StreamKind::ControlLeave => "control.leave",
            StreamKind::SignalingOffer => "signaling.offer",
            StreamKind::SignalingAnswer => "signaling.answer",
            StreamKind::SignalingIce => "signaling.ice",
        }
    }

    pub fn parse(value: &str) -> Result<Self, SyncMessageError> {
        match value {
            "pose" => Ok(StreamKind::Pose),
            "chat" => Ok(StreamKind::Chat),
            "voice" => Ok(StreamKind::Voice),
            "control.join" => Ok(StreamKind::ControlJoin),
            "control.leave" => Ok(StreamKind::ControlLeave),
            "signaling.offer" => Ok(StreamKind::SignalingOffer),
            "signaling.answer" => Ok(StreamKind::SignalingAnswer),
            "signaling.ice" => Ok(StreamKind::SignalingIce),
            other => Err(SyncMessageError::UnknownKind {
                value: other.to_string(),
            }),
        }
    }
}

pub struct StubSyncer;

impl Syncer for StubSyncer {
    fn handle(&mut self, request: SyncerRequest) -> Vec<SyncerEvent> {
        // 単純なメモリ内状態で「1リクエスト→複数イベント」を返す最小実装。
        match request {
            SyncerRequest::Join {
                room_id,
                participant_id,
            } => {
                let mut guard = ROOM_STATE.lock().unwrap();
                let room_entry = guard.entry(room_id.clone()).or_insert_with(Vec::new);

                // 既存参加者を PeerJoined として返す
                let mut events: Vec<SyncerEvent> = room_entry
                    .iter()
                    .cloned()
                    .map(|p| SyncerEvent::PeerJoined { participant_id: p })
                    .collect();

                // SelfJoined を追加
                events.push(SyncerEvent::SelfJoined {
                    room_id,
                    participant_id: participant_id.clone(),
                });

                // 新規参加者を状態に追加
                room_entry.push(participant_id);

                events
            }
            SyncerRequest::SendPose { from, pose, ctx } => {
                // 最小実装: PoseReceived をローカルエコーしない（テストで期待していない）
                // ここではイベントを返さない。
                let _ = (from, pose, ctx);
                Vec::new()
            }
            SyncerRequest::SendChat { chat, ctx } => {
                let _ = (chat, ctx);
                Vec::new()
            }
        }
    }
}

use std::collections::HashMap;
use std::sync::Mutex;

lazy_static::lazy_static! {
    static ref ROOM_STATE: Mutex<HashMap<RoomId, Vec<ParticipantId>>> = Mutex::new(HashMap::new());
}

/// ControlメッセージからPeerJoined/PeerLeftへ橋渡しするための中間表現。
#[derive(Debug, Clone, PartialEq)]
pub struct PendingPeerEvent {
    pub participant_id: String,
    pub reconnect_token: Option<String>,
    pub reason: Option<String>,
    pub kind: PendingPeerEventKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PendingPeerEventKind {
    Joined,
    Left,
}

impl From<ControlMessage> for PendingPeerEvent {
    fn from(msg: ControlMessage) -> Self {
        match msg {
            ControlMessage::Join(payload) => PendingPeerEvent {
                participant_id: payload.participant_id,
                reconnect_token: payload.reconnect_token,
                reason: payload.reason,
                kind: PendingPeerEventKind::Joined,
            },
            ControlMessage::Leave(payload) => PendingPeerEvent {
                participant_id: payload.participant_id,
                reconnect_token: payload.reconnect_token,
                reason: payload.reason,
                kind: PendingPeerEventKind::Left,
            },
        }
    }
}

impl PendingPeerEvent {
    pub fn into_syncer_event(self) -> Option<SyncerEvent> {
        let participant_id = ParticipantId::from_str(&self.participant_id).ok()?;
        let event = match self.kind {
            PendingPeerEventKind::Joined => SyncerEvent::PeerJoined { participant_id },
            PendingPeerEventKind::Left => SyncerEvent::PeerLeft { participant_id },
        };

        Some(event)
    }
}
