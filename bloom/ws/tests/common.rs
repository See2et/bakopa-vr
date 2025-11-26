use bloom_api::ServerToClient;
use bloom_core::{CreateRoomResult, ParticipantId, RoomId};
use bloom_ws::{start_ws_server, MockCore, SharedCore, WsServerHandle};
use futures_util::StreamExt;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tracing::Subscriber;
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::{
    layer::{Context, Layer},
    registry::{LookupSpan, Registry},
};

pub fn setup_tracing() -> (RecordingLayer, tracing::subscriber::DefaultGuard) {
    let layer = RecordingLayer::default();
    let subscriber = Registry::default().with(layer.clone());
    let guard = tracing::subscriber::set_default(subscriber);
    (layer, guard)
}

#[allow(dead_code)]
pub fn new_shared_core_with_arc() -> (SharedCore<MockCore>, Arc<std::sync::Mutex<MockCore>>) {
    let mock_core = MockCore::new(CreateRoomResult {
        room_id: RoomId::new(),
        self_id: ParticipantId::new(),
        participants: vec![],
    });
    let core_arc = Arc::new(std::sync::Mutex::new(mock_core));
    let shared = SharedCore::from_arc(core_arc.clone());
    (shared, core_arc)
}

pub async fn spawn_bloom_ws_server_with_core(
    core: SharedCore<MockCore>,
) -> (String, WsServerHandle) {
    let handle = start_ws_server("127.0.0.1:0".parse().unwrap(), core)
        .await
        .expect("start ws server");
    let url = format!("ws://{}/ws", handle.addr);
    (url, handle)
}

#[allow(dead_code)]
pub async fn spawn_bloom_ws_server() -> (String, WsServerHandle) {
    let core = SharedCore::new(MockCore::new(CreateRoomResult {
        room_id: RoomId::new(),
        self_id: ParticipantId::new(),
        participants: vec![],
    }));
    spawn_bloom_ws_server_with_core(core).await
}

pub async fn recv_server_msg(
    ws: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
) -> ServerToClient {
    loop {
        if let Some(msg) = ws.next().await {
            let msg = msg.expect("ws message ok");
            if let Message::Text(t) = msg {
                if let Ok(parsed) = serde_json::from_str::<ServerToClient>(&t) {
                    return parsed;
                }
            }
        } else {
            panic!("ws closed before receiving message");
        }
    }
}

#[derive(Default, Clone)]
pub struct RecordingLayer {
    pub spans: Arc<Mutex<Vec<SpanRecord>>>,
}

#[derive(Default, Debug)]
#[allow(dead_code)]
pub struct SpanRecord {
    pub fields: HashMap<String, String>,
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
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        _id: &tracing::Id,
        _ctx: Context<'_, S>,
    ) {
        let mut visitor = FieldVisitor::default();
        attrs.record(&mut visitor);

        if let Ok(mut spans) = self.spans.lock() {
            spans.push(SpanRecord {
                fields: visitor.fields,
            });
        }
    }
}

#[allow(dead_code)]
pub fn spans_have_field(spans: &[SpanRecord], key: &str) -> bool {
    spans.iter().any(|s| s.fields.contains_key(key))
}

#[allow(dead_code)]
pub fn spans_have_field_value(spans: &[SpanRecord], key: &str, expected: &str) -> bool {
    spans.iter().any(|s| {
        s.fields
            .get(key)
            .map(|v| v.contains(expected))
            .unwrap_or(false)
    })
}
