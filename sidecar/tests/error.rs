mod support;

use futures_util::{SinkExt, StreamExt};
use syncer::messages::SyncMessageError;
use syncer::SyncerError;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::handshake::client::{generate_key, Request};
use tokio_tungstenite::tungstenite::http::{
    header::{AUTHORIZATION, CONNECTION, HOST, SEC_WEBSOCKET_KEY, SEC_WEBSOCKET_VERSION, UPGRADE},
    HeaderValue,
};
use tokio_tungstenite::tungstenite::Message;
use url::Url;

#[test]
fn syncer_error_maps_to_transport_error() {
    let err = SyncerError::InvalidPayload(SyncMessageError::UnknownKind {
        value: "bogus".to_string(),
    });
    let payload = sidecar::app::syncer_error_payload(&err);
    let value: serde_json::Value = serde_json::from_str(&payload).expect("parse payload");
    assert_eq!(value.get("type").and_then(|v| v.as_str()), Some("Error"));
    assert_eq!(
        value.get("kind").and_then(|v| v.as_str()),
        Some("TransportError")
    );
}

#[test]
fn syncer_error_payload_has_human_readable_message() {
    let err = SyncerError::InvalidPayload(SyncMessageError::UnknownKind {
        value: "bogus".to_string(),
    });
    let payload = sidecar::app::syncer_error_payload(&err);
    let value: serde_json::Value = serde_json::from_str(&payload).expect("parse payload");
    assert_eq!(value.get("type").and_then(|v| v.as_str()), Some("Error"));
    assert_eq!(
        value.get("kind").and_then(|v| v.as_str()),
        Some("TransportError")
    );
    assert_eq!(
        value.get("message").and_then(|v| v.as_str()),
        Some("invalid payload")
    );
}

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

#[tokio::test]
async fn syncer_event_error_is_forwarded_to_client() {
    let _guard = support::EnvGuard::set("SIDECAR_TOKEN", "CORRECT_TOKEN_ABC");

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
    let _ = support::wait_for_self_joined(&mut ws).await;

    sidecar::test_support::push_injected_event(syncer::SyncerEvent::Error {
        kind: SyncerError::InvalidPayload(SyncMessageError::UnknownKind {
            value: "bogus".to_string(),
        }),
    });

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    let mut found = false;
    while std::time::Instant::now() < deadline {
        match tokio::time::timeout(std::time::Duration::from_millis(300), ws.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
                    continue;
                };
                if json.get("type").and_then(|v| v.as_str()) == Some("Error")
                    && json.get("kind").and_then(|v| v.as_str()) == Some("TransportError")
                {
                    found = true;
                    break;
                }
            }
            Ok(Some(Ok(_))) => {}
            Ok(Some(Err(err))) => panic!("ws error: {err:?}"),
            Ok(None) => break,
            Err(_) => {}
        }
    }
    assert!(found, "expected Error kind=TransportError within timeout");
}
