use bloom_api::payload::{RelayIce, RelaySdp};
use bloom_api::ClientToServer;
use bloom_core::ParticipantId;

use crate::messages::{SignalingAnswer, SignalingIce, SignalingOffer};

/// Bloom WebSocketシグナリングとの境界を抽象化するための最小trait。
pub trait SignalingAdapter {
    fn send_offer(&mut self, to: ParticipantId, payload: SignalingOffer);
    fn send_answer(&mut self, to: ParticipantId, payload: SignalingAnswer);
    fn send_ice(&mut self, to: ParticipantId, payload: SignalingIce);
}

/// `ClientToServer` メッセージを送り出すためのシンク抽象。
pub trait ClientToServerSender {
    fn send(&mut self, message: ClientToServer);
}

/// Bloom WebSocketのClientToServerモデルへトランスコードする最小アダプタ。
pub struct BloomSignalingAdapter<S> {
    sender: S,
}

impl<S> BloomSignalingAdapter<S> {
    pub fn new(sender: S) -> Self {
        Self { sender }
    }

    pub fn into_inner(self) -> S {
        self.sender
    }
}

impl<S: ClientToServerSender> SignalingAdapter for BloomSignalingAdapter<S> {
    fn send_offer(&mut self, to: ParticipantId, payload: SignalingOffer) {
        let message = ClientToServer::Offer {
            to: to.to_string(),
            payload: RelaySdp { sdp: payload.sdp },
        };
        self.sender.send(message);
    }

    fn send_answer(&mut self, to: ParticipantId, payload: SignalingAnswer) {
        let message = ClientToServer::Answer {
            to: to.to_string(),
            payload: RelaySdp { sdp: payload.sdp },
        };
        self.sender.send(message);
    }

    fn send_ice(&mut self, to: ParticipantId, payload: SignalingIce) {
        let message = ClientToServer::IceCandidate {
            to: to.to_string(),
            payload: RelayIce {
                candidate: payload.candidate,
            },
        };
        self.sender.send(message);
    }
}
