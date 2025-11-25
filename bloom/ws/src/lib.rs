mod core_api;
mod handler;
mod mocks;
mod rate_limit;
mod sinks;

pub use core_api::CoreApi;
pub use handler::{HandshakeResponse, WsHandler};
pub use mocks::MockCore;
pub use rate_limit::{Clock, RateLimitDecision, RateLimiter};
pub use sinks::{BroadcastSink, NoopBroadcastSink, OutSink, RecordingBroadcastSink, RecordingSink, SharedBroadcastSink};

#[cfg(test)]
mod tests {
    use super::*;
    use bloom_api::{ErrorCode, ServerToClient};
    use bloom_core::{CreateRoomResult, JoinRoomError, ParticipantId, RoomId};

    fn new_room() -> (RoomId, ParticipantId) {
        (RoomId::new(), ParticipantId::new())
    }

    /// WS接続確立→CreateRoom送信で RoomCreated(self_id, room_id) が返ることを検証する。
    #[tokio::test]
    async fn create_room_returns_room_created_after_handshake() {
        let (room_id, self_id) = new_room();
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

        assert_eq!(handler.sink.sent.len(), 1, "CreateRoomに対するレスポンスが1件送られる");
        let sent = &handler.sink.sent[0];
        assert_eq!(
            sent,
            &ServerToClient::RoomCreated {
                room_id: room_id.to_string(),
                self_id: self_id.to_string(),
            }
        );

        let json = serde_json::to_string(sent).expect("serialize server message");
        let roundtrip: ServerToClient = serde_json::from_str(&json).expect("deserialize server message");
        assert_eq!(roundtrip, *sent);
    }

    /// JoinRoom要求でRoomParticipantsブロードキャストが全参加者（自分を含む）へ届くことを検証する。
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

        assert_eq!(handler.core.join_room_calls.len(), 1);
        assert_eq!(handler.core.join_room_calls[0].0, room_id);

        for p in &participants {
            let messages = handler
                .broadcast
                .messages_for(p)
                .expect("各参加者にメッセージが届くはず");
            assert!(matches!(
                messages.last(),
                Some(ServerToClient::RoomParticipants { participants: ps, .. }) if ps.len() == participants.len()
            ));
        }
    }

    /// JoinRoomでRoomFullが返った場合、Error(RoomFull)が送信されブロードキャストされないことを確認。
    #[tokio::test]
    async fn join_room_full_returns_error_without_broadcast() {
        let room_id = RoomId::new();
        let self_id = ParticipantId::new();

        let core_result = CreateRoomResult {
            room_id: room_id.clone(),
            self_id: self_id.clone(),
            participants: vec![self_id.clone()],
        };

        let core = MockCore::new(core_result).with_join_result(Some(Err(JoinRoomError::RoomFull)));
        let sink = RecordingSink::default();
        let broadcast = RecordingBroadcastSink::default();
        let mut handler = WsHandler::new(core, self_id.clone(), sink, broadcast);

        handler.perform_handshake().await;
        handler
            .handle_text_message(&format!(r#"{{"type":"JoinRoom","room_id":"{}"}}"#, room_id))
            .await;

        assert_eq!(handler.core.join_room_calls.len(), 1);
        assert_eq!(handler.sink.sent.len(), 1);
        assert!(matches!(
            handler.sink.sent[0],
            ServerToClient::Error { code: ErrorCode::RoomFull, .. }
        ));
        assert!(handler.broadcast.sent.is_empty());
    }

    /// LeaveRoom要求でcoreのleaveが呼ばれ、残り参加者にPeerDisconnectedと最新RoomParticipantsがブロードキャストされることを確認。
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
        handler.room_id = Some(room_id.clone());
        handler.handle_text_message(r#"{"type":"LeaveRoom"}"#).await;

        assert_eq!(handler.core.leave_room_calls.len(), 1);
        assert_eq!(handler.core.leave_room_calls[0].0, room_id);

        for p in &remaining {
            let msgs = handler
                .broadcast
                .messages_for(p)
                .expect("残り参加者へメッセージが届く");
            assert!(msgs.iter().any(|m| matches!(m, ServerToClient::PeerDisconnected { participant_id } if participant_id == &self_id.to_string())));
            assert!(msgs.iter().any(|m| matches!(m, ServerToClient::RoomParticipants { participants, .. } if participants.len() == remaining.len())));
        }
    }

    /// Offer/Answer/IceCandidate が宛先参加者にだけ配送されることを検証する。
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

        let cases = vec![
            r#"{"type":"Offer","to":"TARGET","sdp":"v=0 offer"}"#,
            r#"{"type":"Answer","to":"TARGET","sdp":"v=0 answer"}"#,
            r#"{"type":"IceCandidate","to":"TARGET","candidate":"cand1"}"#,
        ];

        for json_tpl in cases {
            let json = json_tpl.replace("TARGET", &receiver.to_string());
            handler.handle_text_message(&json).await;
        }

        let recv_msgs = handler
            .broadcast
            .messages_for(&receiver)
            .expect("receiver should get messages");
        assert_eq!(recv_msgs.len(), 3);
        assert!(matches!(recv_msgs[0], ServerToClient::Offer { .. }));
        assert!(matches!(recv_msgs[1], ServerToClient::Answer { .. }));
        assert!(matches!(recv_msgs[2], ServerToClient::IceCandidate { .. }));

        assert!(handler.broadcast.messages_for(&bystander).is_none());
        assert!(handler.broadcast.messages_for(&sender).is_none());
    }

    /// 宛先不在のOfferで、送信者にError(ParticipantNotFound)が返り、他参加者には何も届かないことを検証。
    #[tokio::test]
    async fn offer_to_missing_participant_returns_error_and_no_leak() {
        let room_id = RoomId::new();
        let sender = ParticipantId::new();
        let missing = ParticipantId::new();
        let receiver = ParticipantId::new();

        let core_result = CreateRoomResult {
            room_id: room_id.clone(),
            self_id: sender.clone(),
            participants: vec![sender.clone(), receiver.clone()],
        };

        let core = MockCore::new(core_result)
            .with_relay_offer_result(Err(ErrorCode::ParticipantNotFound));
        let sink = RecordingSink::default();
        let broadcast = RecordingBroadcastSink::default();
        let mut handler = WsHandler::new(core, sender.clone(), sink, broadcast);
        handler.perform_handshake().await;
        handler.room_id = Some(room_id.clone());

        handler
            .handle_text_message(&format!(
                r#"{{"type":"Offer","to":"{}","sdp":"v=0 offer"}}"#,
                missing
            ))
            .await;

        assert_eq!(handler.sink.sent.len(), 1);
        assert!(matches!(
            handler.sink.sent[0],
            ServerToClient::Error { code: ErrorCode::ParticipantNotFound, .. }
        ));
        assert!(handler.broadcast.messages_for(&missing).is_none());
        assert!(handler.broadcast.messages_for(&receiver).is_none());
    }

    /// 宛先不在のAnswerでも送信者にのみエラーを返し、他には送らない。
    #[tokio::test]
    async fn answer_to_missing_participant_returns_error_and_no_leak() {
        let room_id = RoomId::new();
        let sender = ParticipantId::new();
        let missing = ParticipantId::new();
        let receiver = ParticipantId::new();

        let core_result = CreateRoomResult {
            room_id: room_id.clone(),
            self_id: sender.clone(),
            participants: vec![sender.clone(), receiver.clone()],
        };

        let core = MockCore::new(core_result)
            .with_relay_answer_result(Err(ErrorCode::ParticipantNotFound));
        let sink = RecordingSink::default();
        let broadcast = RecordingBroadcastSink::default();
        let mut handler = WsHandler::new(core, sender.clone(), sink, broadcast);
        handler.perform_handshake().await;
        handler.room_id = Some(room_id.clone());

        handler
            .handle_text_message(&format!(
                r#"{{"type":"Answer","to":"{}","sdp":"v=0 answer"}}"#,
                missing
            ))
            .await;

        assert_eq!(handler.sink.sent.len(), 1);
        assert!(matches!(
            handler.sink.sent[0],
            ServerToClient::Error { code: ErrorCode::ParticipantNotFound, .. }
        ));
        assert!(handler.broadcast.messages_for(&missing).is_none());
        assert!(handler.broadcast.messages_for(&receiver).is_none());
    }

    /// 宛先不在のIceCandidateでも送信者にのみエラーを返し、他には送らない。
    #[tokio::test]
    async fn ice_to_missing_participant_returns_error_and_no_leak() {
        let room_id = RoomId::new();
        let sender = ParticipantId::new();
        let missing = ParticipantId::new();
        let receiver = ParticipantId::new();

        let core_result = CreateRoomResult {
            room_id: room_id.clone(),
            self_id: sender.clone(),
            participants: vec![sender.clone(), receiver.clone()],
        };

        let core = MockCore::new(core_result)
            .with_relay_ice_result(Err(ErrorCode::ParticipantNotFound));
        let sink = RecordingSink::default();
        let broadcast = RecordingBroadcastSink::default();
        let mut handler = WsHandler::new(core, sender.clone(), sink, broadcast);
        handler.perform_handshake().await;
        handler.room_id = Some(room_id.clone());

        handler
            .handle_text_message(&format!(
                r#"{{"type":"IceCandidate","to":"{}","candidate":"cand1"}}"#,
                missing
            ))
            .await;

        assert_eq!(handler.sink.sent.len(), 1);
        assert!(matches!(
            handler.sink.sent[0],
            ServerToClient::Error { code: ErrorCode::ParticipantNotFound, .. }
        ));
        assert!(handler.broadcast.messages_for(&missing).is_none());
        assert!(handler.broadcast.messages_for(&receiver).is_none());
    }

    /// 必須フィールド欠落のJSONはInvalidPayloadを返し、core呼び出しが一切発生しないこと。
    #[tokio::test]
    async fn invalid_payload_returns_error_and_skips_core_calls() {
        let (room_id, sender) = new_room();
        let core_result = CreateRoomResult {
            room_id: room_id.clone(),
            self_id: sender.clone(),
            participants: vec![sender.clone()],
        };

        let core = MockCore::new(core_result);
        let sink = RecordingSink::default();
        let broadcast = RecordingBroadcastSink::default();
        let mut handler = WsHandler::new(core, sender.clone(), sink, broadcast);
        handler.perform_handshake().await;

        handler.handle_text_message(r#"{"type":"JoinRoom"}"#).await;

        assert_eq!(handler.sink.sent.len(), 1);
        assert!(matches!(handler.sink.sent[0], ServerToClient::Error { code: ErrorCode::InvalidPayload, .. }));
        assert!(handler.core.join_room_calls.is_empty());
        assert!(handler.broadcast.sent.is_empty());
    }

    /// 未知フィールド付きメッセージはInvalidPayloadで弾かれ、その後の正常メッセージは処理される。
    #[tokio::test]
    async fn unknown_field_then_valid_message_keeps_state_intact() {
        let (room_id, self_id) = new_room();
        let other = ParticipantId::new();

        let core_result = CreateRoomResult {
            room_id: room_id.clone(),
            self_id: self_id.clone(),
            participants: vec![self_id.clone()],
        };

        let core = MockCore::new(core_result)
            .with_join_result(Some(Ok(vec![self_id.clone(), other.clone()])));
        let sink = RecordingSink::default();
        let broadcast = RecordingBroadcastSink::default();
        let mut handler = WsHandler::new(core, self_id.clone(), sink, broadcast);

        handler.perform_handshake().await;

        handler
            .handle_text_message(&format!(
                r#"{{"type":"JoinRoom","room_id":"{}","unknown":true}}"#,
                room_id
            ))
            .await;

        assert_eq!(handler.sink.sent.len(), 1);
        assert!(matches!(handler.sink.sent[0], ServerToClient::Error { code: ErrorCode::InvalidPayload, .. }));
        assert!(handler.core.join_room_calls.is_empty());
        assert!(handler.broadcast.sent.is_empty());

        handler
            .handle_text_message(&format!(
                r#"{{"type":"JoinRoom","room_id":"{}"}}"#,
                room_id
            ))
            .await;

        assert_eq!(handler.core.join_room_calls.len(), 1);
        for p in &[self_id, other] {
            let msgs = handler
                .broadcast
                .messages_for(p)
                .expect("participants should receive broadcast");
            assert!(msgs.iter().any(|m| matches!(m, ServerToClient::RoomParticipants { .. })));
        }
    }

    /// 異常終了時もleaveが1回だけ呼ばれ、残存参加者にPeerDisconnectedが届くことを確認。
    #[tokio::test]
    async fn abnormal_close_triggers_single_leave_and_disconnect_broadcast() {
        let room_id = RoomId::new();
        let self_id = ParticipantId::new();
        let remain_a = ParticipantId::new();
        let remain_b = ParticipantId::new();
        let remaining = vec![remain_a.clone(), remain_b.clone()];

        let core_result = CreateRoomResult {
            room_id: room_id.clone(),
            self_id: self_id.clone(),
            participants: vec![self_id.clone(), remain_a.clone(), remain_b.clone()],
        };

        let core = MockCore::new(core_result).with_leave_result(Some(remaining.clone()));
        let sink = RecordingSink::default();
        let broadcast = SharedBroadcastSink::default();
        let mut handler = WsHandler::new(core, self_id.clone(), sink, broadcast.clone());
        handler.room_id = Some(room_id.clone());

        handler
            .handle_abnormal_close(&remaining)
            .await;

        assert_eq!(handler.core.leave_room_calls.len(), 1);

        for p in &remaining {
            let msgs = broadcast
                .messages_for(p)
                .expect("peer should receive disconnect");
            assert!(msgs
                .iter()
                .any(|m| matches!(m, ServerToClient::PeerDisconnected { participant_id } if participant_id == &self_id.to_string())));
        }

        handler
            .handle_abnormal_close(&remaining)
            .await;
        assert_eq!(handler.core.leave_room_calls.len(), 1);
    }

    /// coreイベント経由のPeerConnected/PeerDisconnectedが同一roomの全接続に配送されることを確認。
    #[tokio::test]
    async fn core_peer_events_are_broadcast_to_all_connections() {
        let room_id = RoomId::new();
        let p1 = ParticipantId::new();
        let p2 = ParticipantId::new();
        let newcomer = ParticipantId::new();

        let core_result = CreateRoomResult {
            room_id: room_id.clone(),
            self_id: p1.clone(),
            participants: vec![p1.clone(), p2.clone()],
        };

        let shared_broadcast = SharedBroadcastSink::default();

        let core1 = MockCore::new(core_result.clone());
        let mut h1 = WsHandler::new(core1, p1.clone(), RecordingSink::default(), shared_broadcast.clone());
        h1.room_id = Some(room_id.clone());

        let core2 = MockCore::new(core_result);
        let mut h2 = WsHandler::new(core2, p2.clone(), RecordingSink::default(), shared_broadcast.clone());
        h2.room_id = Some(room_id.clone());

        h1.broadcast_peer_connected(&[p1.clone(), p2.clone()], &newcomer)
            .await;
        h2.broadcast_peer_connected(&[p1.clone(), p2.clone()], &newcomer)
            .await;

        let msgs_p1 = shared_broadcast
            .messages_for(&p1)
            .expect("p1 should receive broadcast");
        let msgs_p2 = shared_broadcast
            .messages_for(&p2)
            .expect("p2 should receive broadcast");

        assert!(msgs_p1.iter().any(|m| matches!(m, ServerToClient::PeerConnected { participant_id } if participant_id == &newcomer.to_string())));
        assert!(msgs_p2.iter().any(|m| matches!(m, ServerToClient::PeerConnected { participant_id } if participant_id == &newcomer.to_string())));

        h1.broadcast_peer_disconnected(&[p1.clone(), p2.clone()], &newcomer)
            .await;
        h2.broadcast_peer_disconnected(&[p1.clone(), p2.clone()], &newcomer)
            .await;

        let msgs_p1_after = shared_broadcast
            .messages_for(&p1)
            .expect("p1 should receive broadcast");
        let msgs_p2_after = shared_broadcast
            .messages_for(&p2)
            .expect("p2 should receive broadcast");

        assert!(msgs_p1_after
            .iter()
            .any(|m| matches!(m, ServerToClient::PeerDisconnected { participant_id } if participant_id == &newcomer.to_string())));
        assert!(msgs_p2_after
            .iter()
            .any(|m| matches!(m, ServerToClient::PeerDisconnected { participant_id } if participant_id == &newcomer.to_string())));
    }
}
