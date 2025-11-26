use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bloom_core::{CreateRoomResult, ParticipantId, RoomId};
use bloom_ws::{start_ws_server, MockCore, WsServerHandle};
use tokio_tungstenite::connect_async;
use tracing::Subscriber;
use tracing_subscriber::{
    layer::{Context, Layer},
    registry::{LookupSpan, Registry},
};
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;

/// WSハンドシェイクがHTTP 101で確立され、participant_id付きのspanが出ることを検証する。
#[tokio::test]
async fn handshake_returns_switching_protocols_and_sets_participant_span() {
    let layer = RecordingLayer::default();
    let subscriber = Registry::default().with(layer.clone());
    let _guard = tracing::subscriber::set_default(subscriber);

    let (server_url, handle) = spawn_bloom_ws_server().await;

    let (_ws_stream, response) = connect_async(&server_url)
        .await
        .expect("connect to bloom ws server");

    assert_eq!(response.status(), 101);

    tokio::time::sleep(Duration::from_millis(50)).await;
    let spans = layer.spans.lock().expect("collect spans");
    assert!(
        spans_have_field(&spans, "participant_id"),
        "handshake span must include participant_id"
    );

    handle.shutdown().await;
    drop(_guard);
}

/// Bloom WSサーバを起動して接続用URLとspan記録レイヤを返す。
async fn spawn_bloom_ws_server() -> (String, WsServerHandle) {
    let (room_id, self_id) = (RoomId::new(), ParticipantId::new());
    let core = MockCore::new(CreateRoomResult {
        room_id,
        self_id,
        participants: vec![],
    });

    let handle = start_ws_server("127.0.0.1:0".parse().unwrap(), core)
        .await
        .expect("start ws server");
    let url = format!("ws://{}/ws", handle.addr);
    (url, handle)
}

#[derive(Default, Clone)]
struct RecordingLayer {
    spans: Arc<Mutex<Vec<SpanRecord>>>,
}

#[derive(Default, Debug)]
struct SpanRecord {
    fields: HashMap<String, String>,
}

#[derive(Default)]
struct FieldVisitor {
    fields: HashMap<String, String>,
}

impl tracing::field::Visit for FieldVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.fields
            .insert(field.name().to_string(), format!("{value:?}"));
    }
}

impl<S> Layer<S> for RecordingLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &tracing::span::Attributes<'_>, _id: &tracing::Id, _ctx: Context<'_, S>) {
        let mut visitor = FieldVisitor::default();
        attrs.record(&mut visitor);

        if let Ok(mut spans) = self.spans.lock() {
            spans.push(SpanRecord {
                fields: visitor.fields,
            });
        }
    }
}

/// 共通ヘルパ: spanが特定フィールドを持つか確認。
fn spans_have_field(spans: &[SpanRecord], key: &str) -> bool {
    spans.iter().any(|s| s.fields.contains_key(key))
}
