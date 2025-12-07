pub mod messages;

pub use crate::messages::{PoseMessage as Pose, PoseTransform};

use crate::messages::{SyncMessageEnvelope, SyncMessageError};
use bloom_core::{ParticipantId, RoomId};
use serde::{Deserialize, Serialize};

/// Syncer全体のファサード。1リクエストに対して複数イベントを返す契約。
pub trait Syncer {
    fn handle(&mut self, request: SyncerRequest) -> Vec<SyncerEvent>;
}

/// WebRTC/DataChannel等の下位トランスポートを抽象化するためのtrait。
pub trait Transport {
    fn register_participant(&mut self, participant: ParticipantId);
    fn send(&mut self, to: ParticipantId, payload: TransportPayload);
    fn poll(&mut self) -> Vec<TransportEvent>;
}

#[derive(Debug, Clone)]
pub enum TransportPayload {
    Bytes(Vec<u8>),
}

impl TransportPayload {
    /// Parse the underlying bytes as a SyncMessageEnvelope.
    pub fn as_message_envelope(&self) -> Result<SyncMessageEnvelope, SyncMessageError> {
        match self {
            TransportPayload::Bytes(bytes) => SyncMessageEnvelope::from_slice(bytes),
        }
    }
}

#[derive(Debug, Clone)]
pub enum TransportEvent {
    Received {
        from: ParticipantId,
        payload: TransportPayload,
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
    PoseReceived {
        from: ParticipantId,
        pose: Pose,
        ctx: TracingContext,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct TracingContext {
    pub room_id: RoomId,
    pub participant_id: ParticipantId,
    pub stream_kind: StreamKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
        }
    }
}

use std::collections::HashMap;
use std::sync::Mutex;

lazy_static::lazy_static! {
    static ref ROOM_STATE: Mutex<HashMap<RoomId, Vec<ParticipantId>>> = Mutex::new(HashMap::new());
}
