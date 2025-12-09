use bloom_api::payload::{RelayIce, RelaySdp};
use bloom_api::{ClientToServer, ServerToClient};
use bloom_core::ParticipantId;
use std::collections::HashSet;
use std::io;
use std::str::FromStr;

use crate::messages::{SignalingAnswer, SignalingIce, SignalingOffer};
use crate::messages::{SignalingMessage, SyncMessageEnvelope};
use crate::{SyncerEvent, TransportPayload};

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

/// PeerConnectionを破棄するためのフック。WebRTCアダプタ側で実装する。
pub trait PeerConnectionCloser {
    fn close(&mut self, participant: &ParticipantId);
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopCloser;

impl PeerConnectionCloser for NoopCloser {
    fn close(&mut self, _participant: &ParticipantId) {}
}

/// Bloom WebSocketのClientToServerモデルへトランスコードする最小アダプタ。
pub struct BloomSignalingAdapter<S, C = NoopCloser>
where
    C: PeerConnectionCloser,
{
    sender: S,
    closer: C,
    context: SignalingContext,
    inbox: Vec<ServerToClient>,
    active_participants: HashSet<ParticipantId>,
    events: Vec<SyncerEvent>,
}

impl<S> BloomSignalingAdapter<S>
where
    S: ClientToServerSender,
{
    pub fn new(sender: S) -> Self {
        Self {
            sender,
            closer: NoopCloser,
            context: SignalingContext::default(),
            inbox: Vec::new(),
            active_participants: HashSet::new(),
            events: Vec::new(),
        }
    }
}

impl<S, C> BloomSignalingAdapter<S, C>
where
    S: ClientToServerSender,
    C: PeerConnectionCloser,
{
    pub fn with_context(sender: S, context: SignalingContext) -> Self
    where
        C: PeerConnectionCloser + Default,
    {
        Self {
            sender,
            closer: C::default(),
            context,
            inbox: Vec::new(),
            active_participants: HashSet::new(),
            events: Vec::new(),
        }
    }

    pub fn with_context_and_closer(sender: S, closer: C, context: SignalingContext) -> Self {
        Self {
            sender,
            closer,
            context,
            inbox: Vec::new(),
            active_participants: HashSet::new(),
            events: Vec::new(),
        }
    }

    pub fn into_inner(self) -> S {
        self.sender
    }

    pub fn into_inner_closer(self) -> C {
        self.closer
    }

    /// Bloom WebSocketからの受信メッセージをアダプタ内キューへ積む。
    pub fn push_incoming(&mut self, message: ServerToClient) {
        self.inbox.push(message);
    }

    /// キューに溜まったServerToClientをSyncer側で扱えるペイロードとイベントへ変換する。
    pub fn poll(&mut self) -> (Vec<TransportPayload>, Vec<SyncerEvent>) {
        let messages = std::mem::take(&mut self.inbox);
        let mut payloads: Vec<TransportPayload> = Vec::new();

        for msg in messages {
            if let Ok(payload) = self.shape_incoming(msg) {
                payloads.push(payload);
            }
        }

        let events = std::mem::take(&mut self.events);
        (payloads, events)
    }

    fn shape_incoming(&mut self, message: ServerToClient) -> Result<TransportPayload, serde_json::Error> {
        let envelope = match message {
            ServerToClient::Offer { from, payload } => {
                self.on_reoffer(&from);
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

    fn on_reoffer(&mut self, participant: &str) {
        let Ok(pid) = ParticipantId::from_str(participant) else {
            return;
        };

        if !self.active_participants.insert(pid.clone()) {
            // 既存セッションがある場合はPeerLeftを発火させ、Closerに通知する。
            self.closer.close(&pid);
            self.events.push(SyncerEvent::PeerLeft {
                participant_id: pid.clone(),
            });
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SignalingContext {
    pub room_id: String,
    pub auth_token: String,
    pub ice_policy: String,
}

impl<S, C> SignalingAdapter for BloomSignalingAdapter<S, C>
where
    S: ClientToServerSender,
    C: PeerConnectionCloser,
{
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
