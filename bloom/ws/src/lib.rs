mod core_api;
mod handler;
mod mocks;
mod rate_limit;
mod real_core;
mod server;
mod sinks;

pub use core_api::{CoreApi, RelayAction};
pub use handler::{HandshakeResponse, WsHandler};
pub use mocks::MockCore;
pub use rate_limit::{
    Clock, DynClock, RateLimitConfig, RateLimitDecision, RateLimiter, SystemClock,
};
pub use real_core::RealCore;
pub use server::{
    start_ws_server, SharedCore, WebSocketBroadcast, WebSocketOutSink, WsServerHandle,
    ABNORMAL_DISCONNECT_GRACE, PING_TIMEOUT_CLOSE_CODE,
};
pub use sinks::{
    BroadcastSink, NoopBroadcastSink, OutSink, RecordingBroadcastSink, RecordingSink,
    SharedBroadcastSink,
};

#[cfg(test)]
mod tests {
    use super::*;
    use bloom_api::{ErrorCode, ServerToClient};
    use bloom_core::{CreateRoomResult, JoinRoomError, ParticipantId, RoomId};
    use std::sync::{Arc, Mutex, OnceLock};
    use std::time::{Duration, Instant};
    use std::{collections::HashMap, fmt};
    use tracing::Subscriber;
    use tracing_subscriber::{
        layer::{Context, Layer},
        prelude::*,
        registry::{LookupSpan, Registry},
    };

    fn ensure_global_tracing() -> RecordingLayer {
        static LAYER: OnceLock<RecordingLayer> = OnceLock::new();
        let layer = LAYER.get_or_init(RecordingLayer::default).clone();
        static DISPATCH: OnceLock<()> = OnceLock::new();
        DISPATCH.get_or_init(|| {
            let _ = tracing::subscriber::set_global_default(
                tracing_subscriber::registry()
                    .with(tracing_subscriber::filter::LevelFilter::TRACE)
                    .with(layer.clone()),
            );
        });
        layer
    }

    fn new_room() -> (RoomId, ParticipantId) {
        (RoomId::new(), ParticipantId::new())
    }

    /// handle_text_message前段のhandshakeでspanが生成され、participant_idフィールドを持つことを確認する（Red）。
    #[tokio::test]
    async fn handshake_emits_span_with_participant_id_field() {
        let (room_id, self_id) = new_room();
        let core_result = CreateRoomResult {
            room_id: room_id.clone(),
            self_id: self_id.clone(),
            participants: vec![self_id.clone()],
        };

        let core = MockCore::new(core_result);
        let sink = RecordingSink::default();
        let broadcast = NoopBroadcastSink::default();
        let mut handler = WsHandler::new(core, self_id.clone(), sink, broadcast);

        let layer = RecordingLayer::default();
        let subscriber = Registry::default().with(layer.clone());
        let _guard = tracing::subscriber::set_default(subscriber);

        handler.perform_handshake().await;

        drop(_guard);

        let spans = layer
            .spans
            .lock()
            .expect("span records should be collected");
        assert!(!spans.is_empty(), "handshake should emit at least one span");
        let has_participant_id = spans_have_field(&spans, "participant_id", &self_id.to_string());
        assert!(has_participant_id, "span must include participant_id field");
    }

    /// Offer処理のspanにparticipant_idとroom_idの両方が含まれることを検証する。
    #[tokio::test]
    async fn offer_span_includes_participant_and_room() {
        let room_id = RoomId::new();
        let sender = ParticipantId::new();
        let receiver = ParticipantId::new();

        let core_result = CreateRoomResult {
            room_id: room_id.clone(),
            self_id: sender.clone(),
            participants: vec![sender.clone(), receiver.clone()],
        };

        let core = MockCore::new(core_result);
        let sink = RecordingSink::default();
        let broadcast = RecordingBroadcastSink::default();
        let mut handler = WsHandler::new(core, sender.clone(), sink, broadcast);
        handler.room_id = Some(room_id.clone());

        let layer = RecordingLayer::default();
        let subscriber = Registry::default().with(layer.clone());
        let _guard = tracing::subscriber::set_default(subscriber);

        handler
            .handle_text_message(&format!(
                r#"{{\"type\":\"Offer\",\"to\":\"{}\",\"sdp\":\"v=0 offer\"}}"#,
                receiver
            ))
            .await;

        drop(_guard);

        let spans = layer
            .spans
            .lock()
            .expect("span records should be collected");
        assert!(!spans.is_empty(), "at least one span should be emitted");

        assert!(
            spans_have_field(&spans, "participant_id", &sender.to_string()),
            "span must include participant_id"
        );
        assert!(
            spans_have_field(&spans, "room_id", &room_id.to_string()),
            "span must include room_id field when handling offer"
        );
    }

    /// Answer処理のspanにparticipant_idとroom_idが含まれることを検証する。
    #[tokio::test]
    async fn answer_span_includes_participant_and_room() {
        let room_id = RoomId::new();
        let sender = ParticipantId::new();
        let receiver = ParticipantId::new();

        let core_result = CreateRoomResult {
            room_id: room_id.clone(),
            self_id: sender.clone(),
            participants: vec![sender.clone(), receiver.clone()],
        };

        let core = MockCore::new(core_result);
        let sink = RecordingSink::default();
        let broadcast = RecordingBroadcastSink::default();
        let mut handler = WsHandler::new(core, sender.clone(), sink, broadcast);
        handler.room_id = Some(room_id.clone());

        let layer = RecordingLayer::default();
        let subscriber = Registry::default().with(layer.clone());
        let _guard = tracing::subscriber::set_default(subscriber);

        handler
            .handle_text_message(&format!(
                r#"{{\"type\":\"Answer\",\"to\":\"{}\",\"sdp\":\"v=0 answer\"}}"#,
                receiver
            ))
            .await;

        drop(_guard);

        let spans = layer
            .spans
            .lock()
            .expect("span records should be collected");
        assert!(!spans.is_empty(), "at least one span should be emitted");

        assert!(
            spans_have_field(&spans, "participant_id", &sender.to_string()),
            "span must include participant_id"
        );
        assert!(
            spans_have_field(&spans, "room_id", &room_id.to_string()),
            "span must include room_id field when handling answer"
        );
    }

    /// IceCandidate処理のspanにparticipant_idとroom_idが含まれることを検証する。
    #[tokio::test]
    async fn ice_span_includes_participant_and_room() {
        let room_id = RoomId::new();
        let sender = ParticipantId::new();
        let receiver = ParticipantId::new();

        let core_result = CreateRoomResult {
            room_id: room_id.clone(),
            self_id: sender.clone(),
            participants: vec![sender.clone(), receiver.clone()],
        };

        let core = MockCore::new(core_result);
        let sink = RecordingSink::default();
        let broadcast = RecordingBroadcastSink::default();
        let mut handler = WsHandler::new(core, sender.clone(), sink, broadcast);
        handler.room_id = Some(room_id.clone());

        let layer = RecordingLayer::default();
        let subscriber = Registry::default().with(layer.clone());
        let _guard = tracing::subscriber::set_default(subscriber);

        handler
            .handle_text_message(&format!(
                r#"{{\"type\":\"IceCandidate\",\"to\":\"{}\",\"candidate\":\"cand1\"}}"#,
                receiver
            ))
            .await;

        drop(_guard);

        let spans = layer
            .spans
            .lock()
            .expect("span records should be collected");
        assert!(!spans.is_empty(), "at least one span should be emitted");

        assert!(
            spans_have_field(&spans, "participant_id", &sender.to_string()),
            "span must include participant_id"
        );
        assert!(
            spans_have_field(&spans, "room_id", &room_id.to_string()),
            "span must include room_id field when handling ice"
        );
    }

    /// LeaveRoom処理のspanにparticipant_idとroom_idが含まれることを検証する。
    #[tokio::test]
    async fn leave_span_includes_participant_and_room() {
        let room_id = RoomId::new();
        let self_id = ParticipantId::new();
        let remaining = vec![ParticipantId::new()];

        let core_result = CreateRoomResult {
            room_id: room_id.clone(),
            self_id: self_id.clone(),
            participants: vec![self_id.clone()]
                .into_iter()
                .chain(remaining.clone())
                .collect(),
        };

        let core = MockCore::new(core_result).with_leave_result(Some(remaining.clone()));
        let sink = RecordingSink::default();
        let broadcast = RecordingBroadcastSink::default();
        let mut handler = WsHandler::new(core, self_id.clone(), sink, broadcast);
        handler.room_id = Some(room_id.clone());

        let layer = RecordingLayer::default();
        let subscriber = Registry::default().with(layer.clone());
        let _guard = tracing::subscriber::set_default(subscriber);

        handler
            .handle_text_message(r#"{\"type\":\"LeaveRoom\"}"#)
            .await;

        drop(_guard);

        let spans = layer
            .spans
            .lock()
            .expect("span records should be collected");
        assert!(!spans.is_empty(), "at least one span should be emitted");

        assert!(
            spans_have_field(&spans, "participant_id", &self_id.to_string()),
            "span must include participant_id"
        );
        assert!(
            spans_have_field(&spans, "room_id", &room_id.to_string()),
            "span must include room_id field when handling leave"
        );
    }

    /// InvalidPayloadエラーパスでもspanにparticipant_idが含まれることを確認する（Red）。
    #[tokio::test]
    async fn invalid_payload_span_keeps_participant_id() {
        let (_room_id, self_id) = new_room();

        let core_result = CreateRoomResult {
            room_id: RoomId::new(),
            self_id: self_id.clone(),
            participants: vec![self_id.clone()],
        };

        let core = MockCore::new(core_result);
        let sink = RecordingSink::default();
        let broadcast = RecordingBroadcastSink::default();
        let mut handler = WsHandler::new(core, self_id.clone(), sink, broadcast);

        let layer = RecordingLayer::default();
        let subscriber = Registry::default().with(layer.clone());
        let _guard = tracing::subscriber::set_default(subscriber);

        // room_id欠落のJoinRoomでInvalidPayloadを発生させる
        handler
            .handle_text_message(r#"{\"type\":\"JoinRoom\"}"#)
            .await;

        drop(_guard);

        let spans = layer
            .spans
            .lock()
            .expect("span records should be collected");
        assert!(
            !spans.is_empty(),
            "invalid payload handling should emit span"
        );

        let has_participant = spans.iter().any(|s| {
            s.fields
                .get("participant_id")
                .map(|v| v.contains(&self_id.to_string()))
                .unwrap_or(false)
        });

        assert!(
            has_participant,
            "span must include participant_id even on invalid payload"
        );
    }

    /// coreイベントのPeerConnectedブロードキャストspanにroom_id/participant_idが含まれることを検証する。
    #[tokio::test]
    async fn core_peer_connected_span_includes_room_and_participant() {
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

        let core = MockCore::new(core_result);
        let sink = RecordingSink::default();
        let mut handler = WsHandler::new(core, p1.clone(), sink, shared_broadcast.clone());
        handler.room_id = Some(room_id.clone());

        let layer = RecordingLayer::default();
        let subscriber = Registry::default().with(layer.clone());
        let _guard = tracing::subscriber::set_default(subscriber);

        handler
            .broadcast_peer_connected(&[p1.clone(), p2.clone()], &newcomer)
            .await;

        drop(_guard);

        let spans = layer
            .spans
            .lock()
            .expect("span records should be collected");
        assert!(!spans.is_empty(), "peer connected should emit span");

        assert!(
            spans_have_field(&spans, "participant_id", &p1.to_string()),
            "span must include participant_id"
        );
        assert!(
            spans_have_field(&spans, "room_id", &room_id.to_string()),
            "peer connected span must include room_id"
        );
    }

    /// coreイベントのPeerDisconnectedブロードキャストspanにroom_id/participant_idが含まれることを検証する。
    #[tokio::test]
    async fn core_peer_disconnected_span_includes_room_and_participant() {
        let room_id = RoomId::new();
        let p1 = ParticipantId::new();
        let p2 = ParticipantId::new();
        let leaving = ParticipantId::new();

        let core_result = CreateRoomResult {
            room_id: room_id.clone(),
            self_id: p1.clone(),
            participants: vec![p1.clone(), p2.clone(), leaving.clone()],
        };

        let shared_broadcast = SharedBroadcastSink::default();

        let core = MockCore::new(core_result);
        let sink = RecordingSink::default();
        let mut handler = WsHandler::new(core, p1.clone(), sink, shared_broadcast.clone());
        handler.room_id = Some(room_id.clone());

        let layer = RecordingLayer::default();
        let subscriber = Registry::default().with(layer.clone());
        let _guard = tracing::subscriber::set_default(subscriber);

        handler
            .broadcast_peer_disconnected(&[p1.clone(), p2.clone()], &leaving)
            .await;

        drop(_guard);

        let spans = layer
            .spans
            .lock()
            .expect("span records should be collected");
        assert!(!spans.is_empty(), "peer disconnected should emit span");

        assert!(
            spans_have_field(&spans, "participant_id", &p1.to_string()),
            "span must include participant_id"
        );
        assert!(
            spans_have_field(&spans, "room_id", &room_id.to_string()),
            "peer disconnected span must include room_id"
        );
    }

    /// RateLimited時のwarnログにparticipant_idフィールドが文字列IDで入っていることを検証する（Red）。
    #[tokio::test(flavor = "current_thread")]
    async fn rate_limit_warn_contains_participant_id_field() {
        let room_id = RoomId::new();
        let sender = ParticipantId::new();
        let receiver = ParticipantId::new();

        let core_result = CreateRoomResult {
            room_id: room_id.clone(),
            self_id: sender.clone(),
            participants: vec![sender.clone(), receiver.clone()],
        };

        let core = MockCore::new(core_result);
        let sink = RecordingSink::default();
        let broadcast = RecordingBroadcastSink::default();

        let clock = Arc::new(TestClock::start_at(Instant::now()));
        let dyn_clock: DynClock = clock.clone();
        let limiter = RateLimiter::from_config(dyn_clock, RateLimitConfig::default());

        let mut handler =
            WsHandler::with_rate_limiter(core, sender.clone(), sink, broadcast, limiter);
        handler.room_id = Some(room_id.clone());

        let layer = ensure_global_tracing();
        // 共有レイヤーを使うので事前にクリア
        if let Ok(mut spans) = layer.spans.lock() {
            spans.clear();
        }
        if let Ok(mut events) = layer.events.lock() {
            events.clear();
        }

        let subscriber = Registry::default()
            .with(tracing_subscriber::filter::LevelFilter::TRACE)
            .with(layer.clone());

        tracing::subscriber::with_default(subscriber, || async {
            tracing::callsite::rebuild_interest_cache();

            let msg = format!(
                r#"{{\"type\":\"IceCandidate\",\"to\":\"{}\",\"candidate\":\"cand\"}}"#,
                receiver
            );

            // 21件目でレートリミットに到達させる
            for _ in 0..21 {
                handler.handle_text_message(&msg).await;
            }
        })
        .await;

        // RateLimitedエラーが返っていることも確認しておく。
        let last_err = handler.sink.sent.last();
        assert!(
            matches!(
                last_err,
                Some(ServerToClient::Error {
                    code: ErrorCode::RateLimited,
                    ..
                })
            ),
            "21st message should be rate limited"
        );

        let events = layer.events.lock().expect("events should be recorded");
        let warn_with_participant = events.iter().find(|e| {
            e.level == tracing::Level::WARN
                && e.fields
                    .get("participant_id")
                    .map(|v| v == &sender.to_string())
                    .unwrap_or(false)
        });

        if warn_with_participant.is_none() {
            panic!(
                "warn log must include participant_id field as string. events={:?}",
                *events
            );
        }
    }

    /// Spanを記録するテスト用Layer。span生成時のフィールドを保持する。
    #[derive(Clone, Default)]
    struct RecordingLayer {
        spans: Arc<Mutex<Vec<SpanRecord>>>,
        events: Arc<Mutex<Vec<EventRecord>>>,
    }

    #[derive(Debug, Clone, Default)]
    struct SpanRecord {
        fields: HashMap<String, String>,
    }

    #[derive(Debug, Clone)]
    struct EventRecord {
        level: tracing::Level,
        fields: HashMap<String, String>,
    }

    impl<S> Layer<S> for RecordingLayer
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        fn on_new_span(
            &self,
            attrs: &tracing::span::Attributes<'_>,
            id: &tracing::span::Id,
            ctx: Context<'_, S>,
        ) {
            let mut visitor = FieldVisitor::default();
            attrs.record(&mut visitor);

            // span拡張に記録されたフィールド（親由来など）も収集する
            if let Some(span) = ctx.span(id) {
                span.extensions().get::<SpanFields>().map(|ext| {
                    for (k, v) in &ext.fields {
                        visitor.fields.entry(k.clone()).or_insert(v.clone());
                    }
                });
                span.extensions_mut().insert(SpanFields {
                    fields: visitor.fields.clone(),
                });
            }

            let record = SpanRecord {
                fields: visitor.fields,
            };

            if let Ok(mut guard) = self.spans.lock() {
                guard.push(record);
            }
        }

        fn on_event(&self, event: &tracing::Event<'_>, _: Context<'_, S>) {
            let mut visitor = FieldVisitor::default();
            event.record(&mut visitor);
            let record = EventRecord {
                level: *event.metadata().level(),
                fields: visitor.fields,
            };

            if let Ok(mut guard) = self.events.lock() {
                guard.push(record);
            }
        }
    }

    #[derive(Default, Debug)]
    struct SpanFields {
        fields: HashMap<String, String>,
    }

    #[derive(Default, Debug)]
    struct FieldVisitor {
        fields: HashMap<String, String>,
    }

    impl tracing::field::Visit for FieldVisitor {
        fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
            self.fields
                .insert(field.name().to_string(), format!("{value:?}"));
        }
    }

    /// 共通ヘルパ: spanが特定フィールド=値を持つかを確認。
    fn spans_have_field(spans: &[SpanRecord], key: &str, value: &str) -> bool {
        spans.iter().any(|s| {
            s.fields
                .get(key)
                .map(|v| v.contains(value))
                .unwrap_or(false)
        })
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

        let json = serde_json::to_string(sent).expect("serialize server message");
        let roundtrip: ServerToClient =
            serde_json::from_str(&json).expect("deserialize server message");
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
            ServerToClient::Error {
                code: ErrorCode::RoomFull,
                ..
            }
        ));
        assert!(handler.broadcast.sent.is_empty());
    }

    /// JoinRoomで存在しないroom_idを指定した場合にRoomNotFoundエラーが返ることを確認。
    #[tokio::test]
    async fn join_room_not_found_returns_room_not_found_error() {
        let room_id = RoomId::new();
        let self_id = ParticipantId::new();

        let core_result = CreateRoomResult {
            room_id: room_id.clone(),
            self_id: self_id.clone(),
            participants: vec![self_id.clone()],
        };

        // join_room_result = None を返すモックで「存在しない」扱いにする
        let core = MockCore::new(core_result).with_join_result(None);
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
            ServerToClient::Error {
                code: ErrorCode::RoomNotFound,
                ..
            }
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

        let core =
            MockCore::new(core_result).with_relay_offer_result(Err(ErrorCode::ParticipantNotFound));
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
            ServerToClient::Error {
                code: ErrorCode::ParticipantNotFound,
                ..
            }
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
            ServerToClient::Error {
                code: ErrorCode::ParticipantNotFound,
                ..
            }
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

        let core =
            MockCore::new(core_result).with_relay_ice_result(Err(ErrorCode::ParticipantNotFound));
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
            ServerToClient::Error {
                code: ErrorCode::ParticipantNotFound,
                ..
            }
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
        assert!(matches!(
            handler.sink.sent[0],
            ServerToClient::Error {
                code: ErrorCode::InvalidPayload,
                ..
            }
        ));
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
        assert!(matches!(
            handler.sink.sent[0],
            ServerToClient::Error {
                code: ErrorCode::InvalidPayload,
                ..
            }
        ));
        assert!(handler.core.join_room_calls.is_empty());
        assert!(handler.broadcast.sent.is_empty());

        handler
            .handle_text_message(&format!(r#"{{"type":"JoinRoom","room_id":"{}"}}"#, room_id))
            .await;

        assert_eq!(handler.core.join_room_calls.len(), 1);
        for p in &[self_id, other] {
            let msgs = handler
                .broadcast
                .messages_for(p)
                .expect("participants should receive broadcast");
            assert!(msgs
                .iter()
                .any(|m| matches!(m, ServerToClient::RoomParticipants { .. })));
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

        handler.handle_abnormal_close().await;

        assert_eq!(handler.core.leave_room_calls.len(), 1);

        for p in &remaining {
            let msgs = broadcast
                .messages_for(p)
                .expect("peer should receive disconnect");
            assert!(msgs
                .iter()
                .any(|m| matches!(m, ServerToClient::PeerDisconnected { participant_id } if participant_id == &self_id.to_string())));
        }

        handler.handle_abnormal_close().await;
        assert_eq!(handler.core.leave_room_calls.len(), 1);
    }

    /// 異常切断時に他ルームの参加者へ通知が漏れないことを検証する（Red）。
    #[tokio::test]
    async fn abnormal_close_broadcasts_only_within_same_room() {
        // room A: p1 -> disconnecting, p2 stays
        // room B: q1 unrelated
        let room_a = RoomId::new();
        let room_b = RoomId::new();
        let p1 = ParticipantId::new();
        let p2 = ParticipantId::new();
        let q1 = ParticipantId::new();

        let core_a_result = CreateRoomResult {
            room_id: room_a.clone(),
            self_id: p1.clone(),
            participants: vec![p1.clone(), p2.clone()],
        };
        let core_b_result = CreateRoomResult {
            room_id: room_b.clone(),
            self_id: q1.clone(),
            participants: vec![q1.clone()],
        };

        // p1 leaves room A; remaining should be [p2]
        let core_a = MockCore::new(core_a_result).with_leave_result(Some(vec![p2.clone()]));
        let core_b = MockCore::new(core_b_result).with_leave_result(Some(vec![]));

        let broadcast = SharedBroadcastSink::default();

        let mut handler_a = WsHandler::new(core_a, p1.clone(), RecordingSink::default(), broadcast.clone());
        handler_a.room_id = Some(room_a.clone());

        let mut handler_b = WsHandler::new(core_b, q1.clone(), RecordingSink::default(), broadcast.clone());
        handler_b.room_id = Some(room_b.clone());

        handler_a.handle_abnormal_close().await;

        // p2 (same room) receives disconnect + participants
        let msgs_p2 = broadcast
            .messages_for(&p2)
            .expect("p2 should receive notifications");
        assert!(msgs_p2
            .iter()
            .any(|m| matches!(m, ServerToClient::PeerDisconnected { participant_id } if participant_id == &p1.to_string())));
        assert!(msgs_p2
            .iter()
            .any(|m| matches!(m, ServerToClient::RoomParticipants { participants, .. } if participants.len() == 1 && participants[0] == p2.to_string())));

        // q1 (other room) must receive nothing
        assert!(broadcast.messages_for(&q1).is_none());

        // leave は1回だけ呼ばれる
        assert_eq!(handler_a.core.leave_room_calls.len(), 1);
        assert!(handler_b.core.leave_room_calls.is_empty());
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
        let mut h1 = WsHandler::new(
            core1,
            p1.clone(),
            RecordingSink::default(),
            shared_broadcast.clone(),
        );
        h1.room_id = Some(room_id.clone());

        let core2 = MockCore::new(core_result);
        let mut h2 = WsHandler::new(
            core2,
            p2.clone(),
            RecordingSink::default(),
            shared_broadcast.clone(),
        );
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

    /// 21個目のシグナリングはRateLimitedで弾かれ、coreにはforwardされないこと。
    #[tokio::test]
    async fn rate_limiting_blocks_21st_signaling_message() {
        let room_id = RoomId::new();
        let sender = ParticipantId::new();
        let receiver = ParticipantId::new();

        let core_result = CreateRoomResult {
            room_id: room_id.clone(),
            self_id: sender.clone(),
            participants: vec![sender.clone(), receiver.clone()],
        };

        let core = MockCore::new(core_result);
        let sink = RecordingSink::default();
        let broadcast = RecordingBroadcastSink::default();

        let clock = Arc::new(TestClock::start_at(Instant::now()));
        let dyn_clock: DynClock = clock.clone();
        let limiter = RateLimiter::from_config(dyn_clock, RateLimitConfig::default());

        let mut handler =
            WsHandler::with_rate_limiter(core, sender.clone(), sink, broadcast, limiter);
        handler.perform_handshake().await;
        handler.room_id = Some(room_id.clone());

        let msg = format!(
            r#"{{"type":"IceCandidate","to":"{}","candidate":"cand"}}"#,
            receiver
        );

        for _ in 0..20 {
            handler.handle_text_message(&msg).await;
        }

        assert_eq!(handler.core.relay_ice_calls.len(), 20, "first 20 forwarded");

        handler.handle_text_message(&msg).await;

        assert_eq!(
            handler.core.relay_ice_calls.len(),
            20,
            "21st should not reach core"
        );
        let last = handler.sink.sent.last().expect("an error should be sent");
        assert!(matches!(
            last,
            ServerToClient::Error {
                code: ErrorCode::RateLimited,
                ..
            }
        ));
    }

    /// レート超過後1秒経過すると再び許可され、coreへのforwardが再開されること。
    #[tokio::test]
    async fn rate_limiting_lifts_after_cooldown() {
        let room_id = RoomId::new();
        let sender = ParticipantId::new();
        let receiver = ParticipantId::new();

        let core_result = CreateRoomResult {
            room_id: room_id.clone(),
            self_id: sender.clone(),
            participants: vec![sender.clone(), receiver.clone()],
        };

        let core = MockCore::new(core_result);
        let sink = RecordingSink::default();
        let broadcast = RecordingBroadcastSink::default();

        let clock = Arc::new(TestClock::start_at(Instant::now()));
        let dyn_clock: DynClock = clock.clone();
        let limiter = RateLimiter::from_config(dyn_clock.clone(), RateLimitConfig::default());

        let mut handler =
            WsHandler::with_rate_limiter(core, sender.clone(), sink, broadcast, limiter);
        handler.perform_handshake().await;
        handler.room_id = Some(room_id.clone());

        let msg = format!(
            r#"{{"type":"IceCandidate","to":"{}","candidate":"cand"}}"#,
            receiver
        );

        for _ in 0..21 {
            handler.handle_text_message(&msg).await;
        }

        assert_eq!(
            handler.core.relay_ice_calls.len(),
            20,
            "21st should be blocked"
        );

        clock.advance(Duration::from_secs(1));

        handler.handle_text_message(&msg).await;

        assert_eq!(
            handler.core.relay_ice_calls.len(),
            21,
            "after cooldown, forwarding resumes"
        );
    }

    /// テスト用クロック（手動で進められる）。
    #[derive(Clone)]
    struct TestClock {
        now: Arc<Mutex<Instant>>,
    }

    impl TestClock {
        fn start_at(now: Instant) -> Self {
            Self {
                now: Arc::new(Mutex::new(now)),
            }
        }

        fn advance(&self, duration: Duration) {
            if let Ok(mut guard) = self.now.lock() {
                *guard = *guard + duration;
            }
        }
    }

    impl Clock for TestClock {
        fn now(&self) -> Instant {
            self.now
                .lock()
                .map(|t| *t)
                .unwrap_or_else(|_| Instant::now())
        }
    }
}
