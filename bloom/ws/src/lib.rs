use bloom_api::{ClientToServer, ServerToClient};
use bloom_core::{CreateRoomResult, ParticipantId, RoomId};

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

/// Minimal handshake response used by tests.
#[derive(Debug, PartialEq, Eq)]
pub struct HandshakeResponse {
    pub status: u16,
}

/// Handler per WebSocket connection.
pub struct WsHandler<C, S> {
    core: C,
    participant_id: ParticipantId,
    sink: S,
}

impl<C, S> WsHandler<C, S> {
    pub fn new(core: C, participant_id: ParticipantId, sink: S) -> Self {
        Self {
            core,
            participant_id,
            sink,
        }
    }
}

impl<C, S> WsHandler<C, S>
where
    C: CoreApi,
    S: OutSink,
{
    /// Perform WebSocket handshake (HTTP 101 expected).
    pub async fn perform_handshake(&mut self) -> HandshakeResponse {
        // TODO: 実HTTPハンドシェイク処理を実装
        HandshakeResponse { status: 101 }
    }

    /// Handle a single incoming text message from the client.
    pub async fn handle_text_message(&mut self, _text: &str) {
        // For simplicity, we directly parse the message here.
        let message: ClientToServer =
            serde_json::from_str(_text).expect("deserialize client message");

        match message {
            ClientToServer::CreateRoom => {
                let result = self.core.create_room(self.participant_id.clone());
                let response = ServerToClient::RoomCreated {
                    room_id: result.room_id.to_string(),
                    self_id: result.self_id.to_string(),
                };
                self.sink.send(response);
            }
            _ => {
                unimplemented!("Only CreateRoom is implemented in this test handler");
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

/// Test helper core that returns predetermined values.
#[derive(Clone, Debug)]
pub struct MockCore {
    pub create_room_result: CreateRoomResult,
    pub create_room_calls: Vec<ParticipantId>,
}

impl MockCore {
    pub fn new(create_room_result: CreateRoomResult) -> Self {
        Self {
            create_room_result,
            create_room_calls: Vec::new(),
        }
    }
}

impl CoreApi for MockCore {
    fn create_room(&mut self, room_owner: ParticipantId) -> CreateRoomResult {
        self.create_room_calls.push(room_owner);
        self.create_room_result.clone()
    }

    fn join_room(
        &mut self,
        _room_id: &RoomId,
        _participant: ParticipantId,
    ) -> Option<Result<Vec<ParticipantId>, bloom_core::JoinRoomError>> {
        unimplemented!("join_room not needed for this test yet")
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
        let mut handler = WsHandler::new(core, self_id.clone(), sink);

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
}
