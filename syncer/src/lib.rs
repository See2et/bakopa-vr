use bloom_core::{ParticipantId, RoomId};

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
pub struct Pose {
    pub dummy: (),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TracingContext {
    pub room_id: RoomId,
    pub participant_id: ParticipantId,
    pub stream_kind: StreamKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StreamKind {
    Pose,
    Chat,
    Voice,
    Signaling,
    Control,
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
