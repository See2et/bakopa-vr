use bloom_core::ParticipantId;

use crate::{messages::ChatMessage, participant_table::ParticipantTable, Pose, StreamKind};

#[derive(Debug, Clone, PartialEq)]
pub enum OutboundPayload {
    Pose(Pose),
    Chat(ChatMessage),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Outbound {
    pub to: ParticipantId,
    pub stream_kind: StreamKind,
    pub payload: OutboundPayload,
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
        participants
            .participants()
            .into_iter()
            .filter(|p| p != from)
            .map(|to| Outbound {
                to,
                stream_kind: StreamKind::Pose,
                payload: OutboundPayload::Pose(pose.clone()),
            })
            .collect()
    }

    /// Chatの配送先を計算する。送信者自身は除外し、残り参加者にチャットを配布する。
    pub fn route_chat(
        &self,
        from: &ParticipantId,
        chat: ChatMessage,
        participants: &ParticipantTable,
    ) -> Vec<Outbound> {
        participants
            .participants()
            .into_iter()
            .filter(|p| p != from)
            .map(|to| Outbound {
                to,
                stream_kind: StreamKind::Chat,
                payload: OutboundPayload::Chat(chat.clone()),
            })
            .collect()
    }
}
