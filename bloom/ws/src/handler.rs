use std::str::FromStr;

use bloom_api::{ClientToServer, ErrorCode, RelayIce, RelaySdp, ServerToClient};
use bloom_core::{JoinRoomError, ParticipantId, RoomId};

use crate::core_api::CoreApi;
use crate::sinks::{BroadcastSink, OutSink};

/// Minimal handshake response used by tests.
#[derive(Debug, PartialEq, Eq)]
pub struct HandshakeResponse {
    pub status: u16,
}

/// Handler per WebSocket connection.
pub struct WsHandler<C, S, B> {
    pub(crate) core: C,
    pub(crate) participant_id: ParticipantId,
    /// 接続が属するroom（Create/Join後に設定）。
    pub(crate) room_id: Option<RoomId>,
    pub(crate) sink: S,
    pub(crate) broadcast: B,
}

impl<C, S, B> WsHandler<C, S, B> {
    pub fn new(core: C, participant_id: ParticipantId, sink: S, broadcast: B) -> Self {
        Self {
            core,
            participant_id,
            room_id: None,
            sink,
            broadcast,
        }
    }
}

impl<C, S, B> WsHandler<C, S, B>
where
    C: CoreApi,
    S: OutSink,
    B: BroadcastSink,
{
    /// Perform WebSocket handshake (HTTP 101 expected).
    pub async fn perform_handshake(&mut self) -> HandshakeResponse {
        HandshakeResponse { status: 101 }
    }

    /// Handle a single incoming text message from the client.
    pub async fn handle_text_message(&mut self, text: &str) {
        let message: ClientToServer = match serde_json::from_str(text) {
            Ok(m) => m,
            Err(_) => {
                self.send_error(ErrorCode::InvalidPayload, "invalid payload");
                return;
            }
        };

        match message {
            ClientToServer::CreateRoom => {
                let result = self.core.create_room(self.participant_id.clone());
                self.room_id = Some(result.room_id.clone());
                let response = ServerToClient::RoomCreated {
                    room_id: result.room_id.to_string(),
                    self_id: result.self_id.to_string(),
                };
                self.sink.send(response);
            }
            ClientToServer::JoinRoom { room_id } => {
                let room_id_parsed =
                    RoomId::from_str(&room_id).expect("room_id should be UUID string");
                self.room_id = Some(room_id_parsed.clone());
                match self
                    .core
                    .join_room(&room_id_parsed, self.participant_id.clone())
                {
                    Some(Ok(participants)) => {
                        self.broadcast_room_participants(&room_id, &participants);
                    }
                    Some(Err(JoinRoomError::RoomFull)) => {
                        self.send_error(ErrorCode::RoomFull, "room is full");
                    }
                    other => {
                        todo!("JoinRoom not handled yet: {:?}", other);
                    }
                }
            }
            ClientToServer::LeaveRoom => {
                let room_id = self
                    .room_id
                    .clone()
                    .expect("room_id must be set before LeaveRoom");

                match self.core.leave_room(&room_id, &self.participant_id) {
                    Some(remaining) => {
                        let participants_str: Vec<String> =
                            remaining.iter().map(ToString::to_string).collect();

                        // 1) PeerDisconnectedを残り全員へ
                        let disconnect_evt = ServerToClient::PeerDisconnected {
                            participant_id: self.participant_id.to_string(),
                        };
                        for p in remaining.iter() {
                            self.broadcast.send_to(p, disconnect_evt.clone());
                        }

                        // 2) 最新RoomParticipantsを残り全員へ
                        let participants_evt = ServerToClient::RoomParticipants {
                            room_id: room_id.to_string(),
                            participants: participants_str,
                        };
                        for p in remaining.iter() {
                            self.broadcast.send_to(p, participants_evt.clone());
                        }

                        // 3) 接続側のroom_idをクリア
                        self.room_id = None;
                    }
                    None => panic!("leave_room returned None (room not found)"),
                }
            }
            ClientToServer::Offer { to, payload } => {
                self.handle_signaling_offer(to, payload).await;
            }
            ClientToServer::Answer { to, payload } => {
                self.handle_signaling_answer(to, payload).await;
            }
            ClientToServer::IceCandidate { to, payload } => {
                self.handle_signaling_ice(to, payload).await;
            }
        }
    }

    async fn handle_signaling_offer(&mut self, to: String, payload: RelaySdp) {
        let room_id = self
            .room_id
            .clone()
            .expect("room must be set before signaling");
        let to_id = ParticipantId::from_str(&to).expect("to must be UUID");

        match self
            .core
            .relay_offer(&room_id, &self.participant_id, &to_id, payload.clone())
        {
            Ok(_) => {
                let event = ServerToClient::Offer {
                    from: self.participant_id.to_string(),
                    payload,
                };
                self.broadcast.send_to(&to_id, event);
            }
            Err(code) => {
                self.send_error(code, "failed to relay offer");
            }
        }
    }

    async fn handle_signaling_answer(&mut self, to: String, payload: RelaySdp) {
        let room_id = self
            .room_id
            .clone()
            .expect("room must be set before signaling");
        let to_id = ParticipantId::from_str(&to).expect("to must be UUID");

        match self
            .core
            .relay_answer(&room_id, &self.participant_id, &to_id, payload.clone())
        {
            Ok(_) => {
                let event = ServerToClient::Answer {
                    from: self.participant_id.to_string(),
                    payload,
                };
                self.broadcast.send_to(&to_id, event);
            }
            Err(code) => {
                self.send_error(code, "failed to relay answer");
            }
        }
    }

    async fn handle_signaling_ice(&mut self, to: String, payload: RelayIce) {
        let room_id = self
            .room_id
            .clone()
            .expect("room must be set before signaling");
        let to_id = ParticipantId::from_str(&to).expect("to must be UUID");

        match self.core.relay_ice_candidate(
            &room_id,
            &self.participant_id,
            &to_id,
            payload.clone(),
        ) {
            Ok(_) => {
                let event = ServerToClient::IceCandidate {
                    from: self.participant_id.to_string(),
                    payload,
                };
                self.broadcast.send_to(&to_id, event);
            }
            Err(code) => {
                self.send_error(code, "failed to relay ice");
            }
        }
    }

    /// Hook to forward peer connection events from core to all participants in the room.
    pub async fn broadcast_peer_connected(
        &mut self,
        participants: &[ParticipantId],
        joined: &ParticipantId,
    ) {
        let event = ServerToClient::PeerConnected {
            participant_id: joined.to_string(),
        };
        for p in participants {
            self.broadcast.send_to(p, event.clone());
        }
    }

    /// Hook to forward peer disconnection events from core to all participants in the room.
    pub async fn broadcast_peer_disconnected(
        &mut self,
        participants: &[ParticipantId],
        left: &ParticipantId,
    ) {
        let event = ServerToClient::PeerDisconnected {
            participant_id: left.to_string(),
        };
        for p in participants {
            self.broadcast.send_to(p, event.clone());
        }
    }

    /// Handle abnormal socket close (error path). Should trigger leave once and notify peers.
    pub async fn handle_abnormal_close(&mut self, participants: &[ParticipantId]) {
        if let Some(room_id) = self.room_id.clone() {
            let remaining = self.core.leave_room(&room_id, &self.participant_id);

            if let Some(rem) = remaining {
                let disconnect_evt = ServerToClient::PeerDisconnected {
                    participant_id: self.participant_id.to_string(),
                };
                for p in participants {
                    self.broadcast.send_to(p, disconnect_evt.clone());
                }
                let participants_evt = ServerToClient::RoomParticipants {
                    room_id: room_id.to_string(),
                    participants: rem.iter().map(ToString::to_string).collect(),
                };
                for p in participants {
                    self.broadcast.send_to(p, participants_evt.clone());
                }
            }

            self.room_id = None;
        }
    }

    fn broadcast_room_participants(&mut self, room_id_str: &str, participants: &[ParticipantId]) {
        let participants_str: Vec<String> = participants.iter().map(ToString::to_string).collect();
        let event = ServerToClient::RoomParticipants {
            room_id: room_id_str.to_string(),
            participants: participants_str,
        };
        for p in participants.iter() {
            self.broadcast.send_to(p, event.clone());
        }
    }

    fn send_error(&mut self, code: ErrorCode, message: &str) {
        self.sink.send(ServerToClient::Error {
            code,
            message: message.into(),
        });
    }
}

