use bloom_core::ParticipantId;

use crate::{messages::ChatMessage, Pose, StreamKind};

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
        participants: &[ParticipantId],
    ) -> Vec<Outbound> {
        participants
            .iter()
            .filter(|p| *p != from)
            .cloned()
            .map(|to| Outbound {
                to,
                stream_kind: StreamKind::Pose,
                payload: OutboundPayload::Pose(pose.clone()),
            })
            .collect()
    }
}
