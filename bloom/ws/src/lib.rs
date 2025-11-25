use bloom_api::{ClientToServer, ErrorCode, RelayIce, RelaySdp, ServerToClient};
use bloom_core::{CreateRoomResult, JoinRoomError, ParticipantId, RoomId};
use std::collections::HashMap;
use std::str::FromStr;

/// Core domain API that the WebSocket layer depends on.
pub trait CoreApi {
    fn create_room(&mut self, room_owner: ParticipantId) -> CreateRoomResult;
    fn join_room(
        &mut self,
        room_id: &RoomId,
        participant: ParticipantId,
    ) -> Option<Result<Vec<ParticipantId>, bloom_core::JoinRoomError>>;
    fn leave_room(
        &mut self,
        room_id: &RoomId,
        participant: &ParticipantId,
    ) -> Option<Vec<ParticipantId>>;

    fn relay_offer(
        &mut self,
        room_id: &RoomId,
        from: &ParticipantId,
        to: &ParticipantId,
        payload: RelaySdp,
    ) -> Result<(), ErrorCode>;
    fn relay_answer(
        &mut self,
        room_id: &RoomId,
        from: &ParticipantId,
        to: &ParticipantId,
        payload: RelaySdp,
    ) -> Result<(), ErrorCode>;
    fn relay_ice_candidate(
        &mut self,
        room_id: &RoomId,
        from: &ParticipantId,
        to: &ParticipantId,
        payload: RelayIce,
    ) -> Result<(), ErrorCode>;
}

/// Outgoing sink abstraction (e.g., a WebSocket sender).
pub trait OutSink {
    fn send(&mut self, message: ServerToClient);
}

/// Broadcast sink that can deliver messages to specific participants.
pub trait BroadcastSink {
    fn send_to(&mut self, to: &ParticipantId, message: ServerToClient);
}

/// Minimal handshake response used by tests.
#[derive(Debug, PartialEq, Eq)]
pub struct HandshakeResponse {
    pub status: u16,
}

/// Handler per WebSocket connection.
pub struct WsHandler<C, S, B> {
    core: C,
    participant_id: ParticipantId,
    /// 接続が属するroom（Create/Join後に設定）。
    room_id: Option<RoomId>,
    sink: S,
    broadcast: B,
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
        // TODO: 実HTTPハンドシェイク処理を実装
        HandshakeResponse { status: 101 }
    }

    /// Handle a single incoming text message from the client.
    pub async fn handle_text_message(&mut self, text: &str) {
        // For simplicity, we directly parse the message here.
        let message: ClientToServer =
            serde_json::from_str(text).expect("deserialize client message");

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
                        let participants_str: Vec<String> =
                            participants.iter().map(ToString::to_string).collect();
                        let event = ServerToClient::RoomParticipants {
                            room_id: room_id.clone(),
                            participants: participants_str,
                        };
                        for p in participants.iter() {
                            self.broadcast.send_to(p, event.clone());
                        }
                    }
                    other => {
                        panic!("JoinRoom not handled yet: {:?}", other);
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
                let room_id = self
                    .room_id
                    .clone()
                    .expect("room must be set before signaling");
                let to_id = ParticipantId::from_str(&to).expect("to must be UUID");

                self.core
                    .relay_offer(&room_id, &self.participant_id, &to_id, payload.clone())
                    .expect("relay_offer should succeed in this test");

                let event = ServerToClient::Offer {
                    from: self.participant_id.to_string(),
                    payload,
                };
                self.broadcast.send_to(&to_id, event);
            }
            ClientToServer::Answer { to, payload } => {
                let room_id = self
                    .room_id
                    .clone()
                    .expect("room must be set before signaling");
                let to_id = ParticipantId::from_str(&to).expect("to must be UUID");

                self.core
                    .relay_answer(&room_id, &self.participant_id, &to_id, payload.clone())
                    .expect("relay_answer should succeed in this test");

                let event = ServerToClient::Answer {
                    from: self.participant_id.to_string(),
                    payload,
                };
                self.broadcast.send_to(&to_id, event);
            }
            ClientToServer::IceCandidate { to, payload } => {
                let room_id = self
                    .room_id
                    .clone()
                    .expect("room must be set before signaling");
                let to_id = ParticipantId::from_str(&to).expect("to must be UUID");

                self.core
                    .relay_ice_candidate(&room_id, &self.participant_id, &to_id, payload.clone())
                    .expect("relay_ice_candidate should succeed in this test");

                let event = ServerToClient::IceCandidate {
                    from: self.participant_id.to_string(),
                    payload,
                };
                self.broadcast.send_to(&to_id, event);
            }
        }
    }

    /// Handle abrupt WebSocket disconnect from client side.
    pub async fn handle_disconnect(&mut self) {
        // 冪等性のため、room_idがない場合は何もしない
        if let Some(room_id) = self.room_id.clone() {
            match self.core.leave_room(&room_id, &self.participant_id) {
                Some(remaining) => {
                    let participants_str: Vec<String> =
                        remaining.iter().map(ToString::to_string).collect();

                    let disconnect_evt = ServerToClient::PeerDisconnected {
                        participant_id: self.participant_id.to_string(),
                    };
                    let participants_evt = ServerToClient::RoomParticipants {
                        room_id: room_id.to_string(),
                        participants: participants_str,
                    };

                    for p in remaining.iter() {
                        self.broadcast.send_to(p, disconnect_evt.clone());
                        self.broadcast.send_to(p, participants_evt.clone());
                    }
                }
                None => {} // ルームが見つからない場合は黙って無視
            }

            // room_idをクリア
            self.room_id = None;
        }
    }
}

/// Test helper sink that records messages.
#[derive(Default, Debug)]
pub struct RecordingSink {
    pub sent: Vec<ServerToClient>,
}

impl OutSink for RecordingSink {
    fn send(&mut self, message: ServerToClient) {
        self.sent.push(message);
    }
}

/// Test helper broadcast sink that records messages per participant.
#[derive(Default, Debug)]
pub struct RecordingBroadcastSink {
    pub sent: HashMap<ParticipantId, Vec<ServerToClient>>,
}

impl RecordingBroadcastSink {
    pub fn messages_for(&self, participant: &ParticipantId) -> Option<&[ServerToClient]> {
        self.sent.get(participant).map(Vec::as_slice)
    }
}

impl BroadcastSink for RecordingBroadcastSink {
    fn send_to(&mut self, to: &ParticipantId, message: ServerToClient) {
        self.sent.entry(to.clone()).or_default().push(message);
    }
}

/// No-op broadcast sink for tests that don't assert broadcasts.
#[derive(Default)]
pub struct NoopBroadcastSink;

impl BroadcastSink for NoopBroadcastSink {
    fn send_to(&mut self, _to: &ParticipantId, _message: ServerToClient) {}
}

/// Test helper core that returns predetermined values.
#[derive(Clone, Debug)]
pub struct MockCore {
    pub create_room_result: CreateRoomResult,
    pub create_room_calls: Vec<ParticipantId>,
    pub join_room_result: Option<Result<Vec<ParticipantId>, JoinRoomError>>,
    pub join_room_calls: Vec<(RoomId, ParticipantId)>,
    pub leave_room_result: Option<Vec<ParticipantId>>,
    pub leave_room_calls: Vec<(RoomId, ParticipantId)>,
    pub relay_offer_calls: Vec<(RoomId, ParticipantId, ParticipantId, RelaySdp)>,
    pub relay_offer_result: Result<(), ErrorCode>,
    pub relay_answer_calls: Vec<(RoomId, ParticipantId, ParticipantId, RelaySdp)>,
    pub relay_answer_result: Result<(), ErrorCode>,
    pub relay_ice_calls: Vec<(RoomId, ParticipantId, ParticipantId, RelayIce)>,
    pub relay_ice_result: Result<(), ErrorCode>,
}

impl MockCore {
    pub fn new(create_room_result: CreateRoomResult) -> Self {
        Self {
            create_room_result,
            create_room_calls: Vec::new(),
            join_room_result: None,
            join_room_calls: Vec::new(),
            leave_room_result: None,
            leave_room_calls: Vec::new(),
            relay_offer_calls: Vec::new(),
            relay_offer_result: Ok(()),
            relay_answer_calls: Vec::new(),
            relay_answer_result: Ok(()),
            relay_ice_calls: Vec::new(),
            relay_ice_result: Ok(()),
        }
    }

    pub fn with_join_result(
        mut self,
        result: Option<Result<Vec<ParticipantId>, JoinRoomError>>,
    ) -> Self {
        self.join_room_result = result;
        self
    }

    pub fn with_leave_result(mut self, result: Option<Vec<ParticipantId>>) -> Self {
        self.leave_room_result = result;
        self
    }

    pub fn with_relay_offer_result(mut self, result: Result<(), ErrorCode>) -> Self {
        self.relay_offer_result = result;
        self
    }

    pub fn with_relay_answer_result(mut self, result: Result<(), ErrorCode>) -> Self {
        self.relay_answer_result = result;
        self
    }

    pub fn with_relay_ice_result(mut self, result: Result<(), ErrorCode>) -> Self {
        self.relay_ice_result = result;
        self
    }
}

impl CoreApi for MockCore {
    fn create_room(&mut self, room_owner: ParticipantId) -> CreateRoomResult {
        self.create_room_calls.push(room_owner);
        self.create_room_result.clone()
    }

    fn join_room(
        &mut self,
        room_id: &RoomId,
        participant: ParticipantId,
    ) -> Option<Result<Vec<ParticipantId>, bloom_core::JoinRoomError>> {
        self.join_room_calls.push((room_id.clone(), participant));
        self.join_room_result.clone()
    }

    fn leave_room(
        &mut self,
        room_id: &RoomId,
        participant: &ParticipantId,
    ) -> Option<Vec<ParticipantId>> {
        self.leave_room_calls
            .push((room_id.clone(), participant.clone()));
        self.leave_room_result.clone()
    }

    fn relay_offer(
        &mut self,
        room_id: &RoomId,
        from: &ParticipantId,
        to: &ParticipantId,
        payload: RelaySdp,
    ) -> Result<(), ErrorCode> {
        self.relay_offer_calls
            .push((room_id.clone(), from.clone(), to.clone(), payload));
        self.relay_offer_result.clone()
    }

    fn relay_answer(
        &mut self,
        room_id: &RoomId,
        from: &ParticipantId,
        to: &ParticipantId,
        payload: RelaySdp,
    ) -> Result<(), ErrorCode> {
        self.relay_answer_calls
            .push((room_id.clone(), from.clone(), to.clone(), payload));
        self.relay_answer_result.clone()
    }

    fn relay_ice_candidate(
        &mut self,
        room_id: &RoomId,
        from: &ParticipantId,
        to: &ParticipantId,
        payload: RelayIce,
    ) -> Result<(), ErrorCode> {
        self.relay_ice_calls
            .push((room_id.clone(), from.clone(), to.clone(), payload));
        self.relay_ice_result.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bloom_api::ServerToClient;

    /// WS接続確立→CreateRoom送信で RoomCreated(self_id, room_id) が返ることを検証する。
    /// まだ実装が無いため RED になる。
    #[tokio::test]
    async fn create_room_returns_room_created_after_handshake() {
        let room_id = RoomId::new();
        let self_id = ParticipantId::new();
        let core_result = CreateRoomResult {
            room_id: room_id.clone(),
            self_id: self_id.clone(),
            participants: vec![self_id.clone()],
        };

        let core = MockCore::new(core_result.clone());
        let sink = RecordingSink::default();
        let broadcast = NoopBroadcastSink::default();
        let mut handler = WsHandler::new(core, self_id.clone(), sink, broadcast);

        let handshake = handler.perform_handshake().await;
        assert_eq!(handshake.status, 101, "HTTP 101 Switching Protocols を期待");

        handler
            .handle_text_message(r#"{"type":"CreateRoom"}"#)
            .await;

        // 期待: RoomCreatedイベントが送信され、JSONラウンドトリップできる
        assert_eq!(
            handler.sink.sent.len(),
            1,
            "CreateRoomに対するレスポンスが1件送られる"
        );
        let sent = &handler.sink.sent[0];
        assert_eq!(
            sent,
            &ServerToClient::RoomCreated {
                room_id: room_id.to_string(),
                self_id: self_id.to_string(),
            }
        );

        // JSONラウンドトリップ確認
        let json = serde_json::to_string(sent).expect("serialize server message");
        let roundtrip: ServerToClient =
            serde_json::from_str(&json).expect("deserialize server message");
        assert_eq!(roundtrip, *sent);
    }

    /// JoinRoom要求でRoomParticipantsブロードキャストが全参加者（自分を含む）へ届くことを検証する。
    /// 現時点で未実装のためRED。
    #[tokio::test]
    async fn join_room_broadcasts_room_participants_to_all_members() {
        let room_id = RoomId::new();
        let existing = ParticipantId::new();
        let self_id = ParticipantId::new();
        let participants = vec![existing.clone(), self_id.clone()];

        let core_result = CreateRoomResult {
            room_id: room_id.clone(),
            self_id: existing.clone(),
            participants: vec![existing.clone()],
        };

        let core = MockCore::new(core_result).with_join_result(Some(Ok(participants.clone())));
        let sink = RecordingSink::default();
        let broadcast = RecordingBroadcastSink::default();
        let mut handler = WsHandler::new(core, self_id.clone(), sink, broadcast);

        handler.perform_handshake().await;
        handler
            .handle_text_message(&format!(r#"{{"type":"JoinRoom","room_id":"{}"}}"#, room_id))
            .await;

        // core.join_roomが呼ばれること
        assert_eq!(
            handler.core.join_room_calls.len(),
            1,
            "join_roomが1度呼ばれる"
        );
        assert_eq!(
            handler.core.join_room_calls[0].0, room_id,
            "指定room_idで呼ばれる"
        );

        // 各参加者にRoomParticipantsが配信されること
        for p in &participants {
            let messages = handler
                .broadcast
                .messages_for(p)
                .expect("各参加者にメッセージが届くはず");
            assert!(
                matches!(
                    messages.last(),
                    Some(ServerToClient::RoomParticipants { room_id: _, participants: ps })
                        if ps.len() == participants.len()
                ),
                "RoomParticipantsが届く"
            );
        }
    }

    /// LeaveRoom要求でcoreのleaveが呼ばれ、残り参加者にPeerDisconnectedと最新RoomParticipantsがブロードキャストされることを確認（未実装のためRED）。
    #[tokio::test]
    async fn leave_room_broadcasts_disconnect_and_participants() {
        let room_id = RoomId::new();
        let self_id = ParticipantId::new();
        let remaining_a = ParticipantId::new();
        let remaining_b = ParticipantId::new();
        let remaining = vec![remaining_a.clone(), remaining_b.clone()];

        let core_result = CreateRoomResult {
            room_id: room_id.clone(),
            self_id: self_id.clone(),
            participants: vec![self_id.clone(), remaining_a.clone(), remaining_b.clone()],
        };

        let core = MockCore::new(core_result).with_leave_result(Some(remaining.clone()));
        let sink = RecordingSink::default();
        let broadcast = RecordingBroadcastSink::default();
        let mut handler = WsHandler::new(core, self_id.clone(), sink, broadcast);

        handler.perform_handshake().await;
        // 事前にroom_idを接続コンテキストへセット（Join/Create後を想定）
        handler.room_id = Some(room_id.clone());
        handler.handle_text_message(r#"{"type":"LeaveRoom"}"#).await;

        // core.leave_roomが呼ばれる
        assert_eq!(
            handler.core.leave_room_calls.len(),
            1,
            "leave_roomが1度呼ばれる"
        );
        assert_eq!(
            handler.core.leave_room_calls[0].0, room_id,
            "正しいroom_idで呼ばれる"
        );

        // 残り参加者へPeerDisconnectedとRoomParticipantsがブロードキャストされる
        for p in &remaining {
            let msgs = handler
                .broadcast
                .messages_for(p)
                .expect("残り参加者へメッセージが届く");
            assert!(
                msgs.iter()
                    .any(|m| matches!(m, ServerToClient::PeerDisconnected { participant_id } if participant_id == &self_id.to_string())),
                "PeerDisconnectedが届く"
            );
            assert!(
                msgs.iter().any(|m| match m {
                    ServerToClient::RoomParticipants {
                        room_id: rid,
                        participants,
                    } => {
                        rid == &room_id.to_string()
                            && participants.len() == remaining.len()
                            && participants.contains(&remaining_a.to_string())
                            && participants.contains(&remaining_b.to_string())
                    }
                    _ => false,
                }),
                "最新RoomParticipantsが届く"
            );
        }
    }

    /// Offer/Answer/IceCandidate が宛先参加者にだけ配送されることを検証する（未実装のためRED）。
    #[tokio::test]
    async fn signaling_messages_are_routed_only_to_target() {
        let room_id = RoomId::new();
        let sender = ParticipantId::new();
        let receiver = ParticipantId::new();
        let bystander = ParticipantId::new();

        let core_result = CreateRoomResult {
            room_id: room_id.clone(),
            self_id: sender.clone(),
            participants: vec![sender.clone(), receiver.clone(), bystander.clone()],
        };

        let core = MockCore::new(core_result);
        let sink = RecordingSink::default();
        let broadcast = RecordingBroadcastSink::default();
        let mut handler = WsHandler::new(core, sender.clone(), sink, broadcast);
        handler.perform_handshake().await;
        handler.room_id = Some(room_id.clone());

        // テーブル駆動でOffer/Answer/IceCandidateを送信
        let cases = vec![
            r#"{"type":"Offer","to":"TARGET","sdp":"v=0 offer"}"#,
            r#"{"type":"Answer","to":"TARGET","sdp":"v=0 answer"}"#,
            r#"{"type":"IceCandidate","to":"TARGET","candidate":"cand1"}"#,
        ];

        for json_tpl in cases {
            let json = json_tpl.replace("TARGET", &receiver.to_string());
            handler.handle_text_message(&json).await;
        }

        // 宛先のみが受信する
        let recv_msgs = handler
            .broadcast
            .messages_for(&receiver)
            .expect("receiver should get messages");
        assert_eq!(recv_msgs.len(), 3, "3種類のシグナリングが届く");
        assert!(matches!(recv_msgs[0], ServerToClient::Offer { .. }));
        assert!(matches!(recv_msgs[1], ServerToClient::Answer { .. }));
        assert!(matches!(recv_msgs[2], ServerToClient::IceCandidate { .. }));

        // 傍受者・送信者には届かない
        assert!(handler.broadcast.messages_for(&bystander).is_none());
        assert!(handler.broadcast.messages_for(&sender).is_none());
    }
}
