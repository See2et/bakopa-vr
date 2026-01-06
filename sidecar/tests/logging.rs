mod support;

use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::handshake::client::{generate_key, Request};
use tokio_tungstenite::tungstenite::http::{
    header::{AUTHORIZATION, CONNECTION, HOST, SEC_WEBSOCKET_KEY, SEC_WEBSOCKET_VERSION, UPGRADE},
    HeaderValue,
};
use tokio_tungstenite::tungstenite::Message;
use tracing::field::{Field, Visit};
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::Layer;
use url::Url;

fn build_ws_request(url: &Url) -> Request {
    let host = url.host_str().unwrap_or("localhost");
    let port = url.port_or_known_default().unwrap_or(80);
    let host_header = HeaderValue::from_str(&format!("{host}:{port}")).unwrap();

    Request::builder()
        .method("GET")
        .uri(url.as_str())
        .header(HOST, host_header)
        .header(UPGRADE, "websocket")
        .header(CONNECTION, "Upgrade")
        .header(SEC_WEBSOCKET_VERSION, "13")
        .header(SEC_WEBSOCKET_KEY, generate_key())
        .body(())
        .expect("request")
}

#[derive(Clone, Debug)]
struct SpanData {
    name: String,
    fields: HashMap<String, String>,
}

#[derive(Clone, Default)]
struct SpanFieldRecorder {
    spans: Arc<Mutex<HashMap<u64, SpanData>>>,
}

impl SpanFieldRecorder {
    fn spans(&self) -> Arc<Mutex<HashMap<u64, SpanData>>> {
        self.spans.clone()
    }
}

struct FieldCollector {
    fields: HashMap<String, String>,
}

impl FieldCollector {
    fn new() -> Self {
        Self {
            fields: HashMap::new(),
        }
    }
}

impl Visit for FieldCollector {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.fields
            .insert(field.name().to_string(), format!("{value:?}"));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields
            .insert(field.name().to_string(), value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields
            .insert(field.name().to_string(), value.to_string());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields
            .insert(field.name().to_string(), value.to_string());
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.fields
            .insert(field.name().to_string(), value.to_string());
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.fields
            .insert(field.name().to_string(), value.to_string());
    }

    fn record_i128(&mut self, field: &Field, value: i128) {
        self.fields
            .insert(field.name().to_string(), value.to_string());
    }

    fn record_u128(&mut self, field: &Field, value: u128) {
        self.fields
            .insert(field.name().to_string(), value.to_string());
    }
}

impl<S> Layer<S> for SpanFieldRecorder
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::Id,
        _ctx: Context<'_, S>,
    ) {
        let mut collector = FieldCollector::new();
        attrs.record(&mut collector);

        let mut map = self.spans.lock().expect("lock span fields");
        map.insert(
            id.into_u64(),
            SpanData {
                name: attrs.metadata().name().to_string(),
                fields: collector.fields,
            },
        );
    }

    fn on_record(
        &self,
        id: &tracing::Id,
        values: &tracing::span::Record<'_>,
        _ctx: Context<'_, S>,
    ) {
        let mut collector = FieldCollector::new();
        values.record(&mut collector);
        if collector.fields.is_empty() {
            return;
        }
        let mut map = self.spans.lock().expect("lock span fields");
        let entry = map.entry(id.into_u64()).or_insert_with(|| SpanData {
            name: "unknown".to_string(),
            fields: HashMap::new(),
        });
        entry.fields.extend(collector.fields);
    }
}

#[tokio::test]
async fn tracing_emits_required_fields() {
    let _guard = support::EnvGuard::set("SIDECAR_TOKEN", "CORRECT_TOKEN_ABC");

    let recorder = SpanFieldRecorder::default();
    let spans = recorder.spans();
    let subscriber = tracing_subscriber::registry().with(recorder);
    let _subscriber_guard = tracing::subscriber::set_default(subscriber);

    let bloom = support::bloom::spawn_bloom_ws()
        .await
        .expect("spawn bloom ws");
    let bloom_ws_url = bloom.ws_url();

    let app = sidecar::app::App::new().await.expect("app new");
    let server = support::spawn_axum(app.router())
        .await
        .expect("spawn server");
    let url = Url::parse(&format!("{}/sidecar", server.ws_url(""))).expect("url");

    let mut request = build_ws_request(&url);
    request
        .headers_mut()
        .insert(AUTHORIZATION, "Bearer CORRECT_TOKEN_ABC".parse().unwrap());
    let (mut ws, _resp) = connect_async(request)
        .await
        .expect("handshake should succeed");

    let join_payload = format!(
        "{{\"type\":\"Join\",\"room_id\":null,\"bloom_ws_url\":\"{}\",\"ice_servers\":[]}}",
        bloom_ws_url
    );
    ws.send(Message::Text(join_payload))
        .await
        .expect("send join");
    let msg = tokio::time::timeout(std::time::Duration::from_millis(500), ws.next())
        .await
        .expect("timeout waiting selfjoined")
        .expect("stream closed")
        .expect("ws error");
    let text = match msg {
        Message::Text(t) => t,
        other => panic!("unexpected join response: {:?}", other),
    };
    let json: serde_json::Value = serde_json::from_str(&text).expect("parse SelfJoined");
    assert_eq!(
        json.get("type").and_then(|v| v.as_str()),
        Some("SelfJoined")
    );

    let pose_payload = r#"{"type":"SendPose","head":{"position":{"x":0.0,"y":1.0,"z":2.0},"rotation":{"x":0.0,"y":0.0,"z":0.0,"w":1.0}},"hand_l":null,"hand_r":null}"#;
    ws.send(Message::Text(pose_payload.into()))
        .await
        .expect("send pose");

    let expected_room_id = json
        .get("room_id")
        .and_then(|v| v.as_str())
        .expect("room_id");
    let expected_participant_id = json
        .get("participant_id")
        .and_then(|v| v.as_str())
        .expect("participant_id");

    let required_span = "sidecar.send_pose";
    let has_required = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            {
                let spans = spans.lock().expect("lock span fields");
                if let Some(span) = spans.values().find(|span| span.name == required_span) {
                    let room_id = span.fields.get("room_id").map(String::as_str);
                    let participant_id = span.fields.get("participant_id").map(String::as_str);
                    let stream_kind = span.fields.get("stream_kind").map(String::as_str);
                    if room_id == Some(expected_room_id)
                        && participant_id == Some(expected_participant_id)
                        && stream_kind == Some("pose")
                    {
                        break true;
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or(false);

    let spans = spans.lock().expect("lock span fields");
    assert!(
        has_required,
        "expected span '{}' with room_id={}, participant_id={}, stream_kind=pose, got {:?}",
        required_span, expected_room_id, expected_participant_id, *spans
    );
}
