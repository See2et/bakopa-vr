use bloom_api::payload::{RelayIce, RelaySdp};
use bloom_api::{ClientToServer, ServerToClient};
use bloom_core::ParticipantId;
use std::collections::HashSet;
use std::io;
use std::str::FromStr;

use crate::messages::{SignalingAnswer, SignalingIce, SignalingOffer};
use crate::messages::{SignalingMessage, SyncMessageEnvelope, SyncMessageError};
use crate::{SyncerError, SyncerEvent, TransportPayload};

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
    pub fn poll(&mut self) -> PollResult {
        let messages = std::mem::take(&mut self.inbox);
        let mut payloads: Vec<TransportPayload> = Vec::new();

        for msg in messages {
            match self.shape_incoming(msg) {
                Ok(Some(payload)) => payloads.push(payload),
                Ok(None) => {}
                Err(_) => {
                    // 直列化失敗はここでは無視（重大ではない）。将来ログに出す場合はここにhook。
                }
            }
        }

        let events = std::mem::take(&mut self.events);
        PollResult { payloads, events }
    }

    fn shape_incoming(
        &mut self,
        message: ServerToClient,
    ) -> Result<Option<TransportPayload>, serde_json::Error> {
        let envelope_opt = match message {
            ServerToClient::Offer { from, payload } => {
                let (from_pid, existing) = self.track_participant(from.as_str())?;
                self.handle_env_result(
                    from_pid,
                    existing,
                    SyncMessageEnvelope::from_signaling(SignalingMessage::Offer(SignalingOffer {
                        version: 1,
                        room_id: self.context.room_id.clone(),
                        participant_id: from,
                        auth_token: self.context.auth_token.clone(),
                        ice_policy: self.context.ice_policy.clone(),
                        sdp: payload.sdp,
                    })),
                )?
            }
            ServerToClient::Answer { from, payload } => {
                let from_pid = self.parse_participant(from.as_str());
                self.handle_env_result(
                    from_pid,
                    false,
                    SyncMessageEnvelope::from_signaling(SignalingMessage::Answer(
                        SignalingAnswer {
                            version: 1,
                            room_id: self.context.room_id.clone(),
                            participant_id: from,
                            auth_token: self.context.auth_token.clone(),
                            sdp: payload.sdp,
                        },
                    )),
                )?
            }
            ServerToClient::IceCandidate { from, payload } => {
                let from_pid = self.parse_participant(from.as_str());
                self.handle_env_result(
                    from_pid,
                    false,
                    SyncMessageEnvelope::from_signaling(SignalingMessage::Ice(SignalingIce {
                        version: 1,
                        room_id: self.context.room_id.clone(),
                        participant_id: from,
                        auth_token: self.context.auth_token.clone(),
                        candidate: payload.candidate,
                        sdp_mid: None,
                        sdp_mline_index: None,
                    })),
                )?
            }
            _ => {
                return Err(serde_json::Error::io(io::Error::new(
                    io::ErrorKind::Other,
                    "unsupported signaling message",
                )))
            }
        };

        let envelope = match envelope_opt {
            Some(env) => env,
            None => return Ok(None),
        };

        let bytes = serde_json::to_vec(&envelope)?;
        Ok(Some(TransportPayload::Bytes(bytes)))
    }

    /// Track participant and determine whether it is a re-offer.
    /// Returns (Some(pid), true) when participant already existed, (Some(pid), false) when newly inserted.
    /// None if participant_id cannot be parsed.
    fn track_participant(
        &mut self,
        participant: &str,
    ) -> Result<(Option<ParticipantId>, bool), serde_json::Error> {
        if let Some(pid) = self.parse_participant(participant) {
            let existed = !self.active_participants.insert(pid.clone());
            Ok((Some(pid), existed))
        } else {
            Ok((None, false))
        }
    }

    fn close_and_emit_peer_left(&mut self, pid: &ParticipantId) {
        self.closer.close(pid);
        self.events.push(SyncerEvent::PeerLeft {
            participant_id: pid.clone(),
        });
    }

    fn emit_invalid(&mut self, pid: Option<ParticipantId>, error: SyncMessageError, existing: bool) {
        self.events.push(SyncerEvent::Error {
            kind: SyncerError::InvalidPayload(error.clone()),
        });

        if existing {
            if let Some(pid) = pid {
                if self.active_participants.remove(&pid) {
                    self.close_and_emit_peer_left(&pid);
                }
            }
        }
    }

    fn handle_env_result(
        &mut self,
        pid: Option<ParticipantId>,
        existing: bool,
        result: Result<SyncMessageEnvelope, SyncMessageError>,
    ) -> Result<Option<SyncMessageEnvelope>, serde_json::Error> {
        match result {
            Ok(env) => {
                if existing {
                    if let Some(ref pid) = pid {
                        self.close_and_emit_peer_left(pid);
                        self.events.push(SyncerEvent::PeerJoined {
                            participant_id: pid.clone(),
                        });
                    }
                }
                Ok(Some(env))
            }
            Err(err) => {
                self.emit_invalid(pid, err, existing);
                Ok(None)
            }
        }
    }

    fn parse_participant(&self, participant: &str) -> Option<ParticipantId> {
        ParticipantId::from_str(participant).ok()
    }
}

pub struct PollResult {
    pub payloads: Vec<TransportPayload>,
    pub events: Vec<SyncerEvent>,
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
