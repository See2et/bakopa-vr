use bloom_api::{ClientToServer, ServerToClient};
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
    sink: S,
    broadcast: B,
}

impl<C, S, B> WsHandler<C, S, B> {
    pub fn new(core: C, participant_id: ParticipantId, sink: S, broadcast: B) -> Self {
        Self {
            core,
            participant_id,
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
                let response = ServerToClient::RoomCreated {
                    room_id: result.room_id.to_string(),
                    self_id: result.self_id.to_string(),
                };
                self.sink.send(response);
            }
            ClientToServer::JoinRoom { room_id } => {
                let room_id_parsed =
                    RoomId::from_str(&room_id).expect("room_id should be UUID string");
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
            other => {
                unimplemented!("Handler for {other:?} not implemented");
            }
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
}

impl MockCore {
    pub fn new(create_room_result: CreateRoomResult) -> Self {
        Self {
            create_room_result,
            create_room_calls: Vec::new(),
            join_room_result: None,
            join_room_calls: Vec::new(),
        }
    }

    pub fn with_join_result(
        mut self,
        result: Option<Result<Vec<ParticipantId>, JoinRoomError>>,
    ) -> Self {
        self.join_room_result = result;
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
        _room_id: &RoomId,
        _participant: &ParticipantId,
    ) -> Option<Vec<ParticipantId>> {
        unimplemented!("leave_room not needed for this test yet")
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
}
