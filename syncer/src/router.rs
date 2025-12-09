use bloom_core::{ParticipantId, RoomId};

use crate::{
    messages::ChatMessage, participant_table::ParticipantTable, Pose, StreamKind, SyncerEvent,
    TracingContext,
};

#[derive(Debug, Clone, PartialEq)]
pub enum OutboundPayload {
    Pose(Pose),
    Chat(ChatMessage),
}

impl OutboundPayload {
    pub fn kind(&self) -> StreamKind {
        match self {
            OutboundPayload::Pose(_) => StreamKind::Pose,
            OutboundPayload::Chat(_) => StreamKind::Chat,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Outbound {
    pub from: ParticipantId,
    pub to: ParticipantId,
    pub stream_kind: StreamKind,
    pub payload: OutboundPayload,
}

impl Outbound {
    /// Convert Outbound into SyncerEvent with tracing context populated.
    pub fn into_event(self, room_id: &RoomId) -> SyncerEvent {
        let ctx = TracingContext {
            room_id: room_id.clone(),
            participant_id: self.from.clone(),
            stream_kind: self.stream_kind,
        };

        match self.payload {
            OutboundPayload::Pose(pose) => SyncerEvent::PoseReceived {
                from: self.from,
                pose,
                ctx,
            },
            OutboundPayload::Chat(chat) => SyncerEvent::ChatReceived { chat, ctx },
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct Router;

impl Router {
    pub fn new() -> Self {
        Self
    }

    /// Poseの配送先を計算する。送信者自身は除外し、他参加者全員分のOutbondを生成する。
    pub fn route_pose(
        &self,
        from: &ParticipantId,
        pose: Pose,
        participants: &ParticipantTable,
    ) -> Vec<Outbound> {
        self.route_common(from, participants, || OutboundPayload::Pose(pose.clone()))
    }

    /// Chatの配送先を計算する。送信者自身は除外し、残り参加者にチャットを配布する。
    pub fn route_chat(
        &self,
        from: &ParticipantId,
        chat: ChatMessage,
        participants: &ParticipantTable,
    ) -> Vec<Outbound> {
        self.route_common(from, participants, || OutboundPayload::Chat(chat.clone()))
    }

    fn route_common(
        &self,
        from: &ParticipantId,
        participants: &ParticipantTable,
        payload_builder: impl Fn() -> OutboundPayload,
    ) -> Vec<Outbound> {
        participants
            .participants()
            .into_iter()
            .filter(|p| p != from)
            .map(|to| {
                let payload = payload_builder();
                Outbound {
                    from: from.clone(),
                    to,
                    stream_kind: payload.kind(),
                    payload,
                }
            })
            .collect()
    }
}
