use bloom_api::payload::{RelayIce, RelaySdp};
use bloom_api::{ClientToServer, ServerToClient};
use bloom_core::ParticipantId;
use std::io;

use crate::messages::{SignalingAnswer, SignalingIce, SignalingOffer};
use crate::messages::{SignalingMessage, SyncMessageEnvelope};
use crate::TransportPayload;

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
    context: SignalingContext,
    inbox: Vec<ServerToClient>,
}

impl<S> BloomSignalingAdapter<S> {
    pub fn new(sender: S) -> Self {
        Self {
            sender,
            context: SignalingContext::default(),
            inbox: Vec::new(),
        }
    }

    pub fn with_context(sender: S, context: SignalingContext) -> Self {
        Self {
            sender,
            context,
            inbox: Vec::new(),
        }
    }

    pub fn into_inner(self) -> S {
        self.sender
    }

    /// Bloom WebSocketからの受信メッセージをアダプタ内キューへ積む。
    pub fn push_incoming(&mut self, message: ServerToClient) {
        self.inbox.push(message);
    }

    /// キューに溜まったServerToClientをSyncer側で扱えるペイロードへ変換する。
    pub fn poll(&mut self) -> Vec<TransportPayload> {
        let messages = std::mem::take(&mut self.inbox);
        messages
            .into_iter()
            .filter_map(|msg| self.shape_incoming(msg).ok())
            .collect()
    }

    fn shape_incoming(&self, message: ServerToClient) -> Result<TransportPayload, serde_json::Error> {
        let envelope = match message {
            ServerToClient::Offer { from, payload } => {
                let offer = SignalingOffer {
                    version: 1,
                    room_id: self.context.room_id.clone(),
                    participant_id: from,
                    auth_token: self.context.auth_token.clone(),
                    ice_policy: self.context.ice_policy.clone(),
                    sdp: payload.sdp,
                };
                SyncMessageEnvelope::from_signaling(SignalingMessage::Offer(offer)).expect("validate offer")
            }
            ServerToClient::Answer { from, payload } => {
                let answer = SignalingAnswer {
                    version: 1,
                    room_id: self.context.room_id.clone(),
                    participant_id: from,
                    auth_token: self.context.auth_token.clone(),
                    sdp: payload.sdp,
                };
                SyncMessageEnvelope::from_signaling(SignalingMessage::Answer(answer)).expect("validate answer")
            }
            ServerToClient::IceCandidate { from, payload } => {
                let ice = SignalingIce {
                    version: 1,
                    room_id: self.context.room_id.clone(),
                    participant_id: from,
                    auth_token: self.context.auth_token.clone(),
                    candidate: payload.candidate,
                    sdp_mid: None,
                    sdp_mline_index: None,
                };
                SyncMessageEnvelope::from_signaling(SignalingMessage::Ice(ice)).expect("validate ice")
            }
            _ => {
                return Err(serde_json::Error::io(io::Error::new(
                    io::ErrorKind::Other,
                    "unsupported signaling message",
                )))
            }
        };

        let bytes = serde_json::to_vec(&envelope)?;
        Ok(TransportPayload::Bytes(bytes))
    }
}

#[derive(Debug, Clone, Default)]
pub struct SignalingContext {
    pub room_id: String,
    pub auth_token: String,
    pub ice_policy: String,
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
