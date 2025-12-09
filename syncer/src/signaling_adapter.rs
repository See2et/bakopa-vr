use bloom_core::ParticipantId;

use crate::messages::{SignalingAnswer, SignalingIce, SignalingOffer};

/// Bloom WebSocketシグナリングとの境界を抽象化するための最小trait。
pub trait SignalingAdapter {
    fn send_offer(&mut self, to: ParticipantId, payload: SignalingOffer);
    fn send_answer(&mut self, to: ParticipantId, payload: SignalingAnswer);
    fn send_ice(&mut self, to: ParticipantId, payload: SignalingIce);
}
