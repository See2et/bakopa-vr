mod support;

use bloom_core::{ParticipantId, RoomId};
use futures_util::{SinkExt, StreamExt};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Notify;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::handshake::client::{generate_key, Request};
use tokio_tungstenite::tungstenite::http::{
    header::{AUTHORIZATION, CONNECTION, HOST, SEC_WEBSOCKET_KEY, SEC_WEBSOCKET_VERSION, UPGRADE},
    HeaderValue,
};
use tokio_tungstenite::tungstenite::Message;
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

#[tokio::test]
async fn disconnect_triggers_leave() {
    let _guard = support::EnvGuard::set("SIDECAR_TOKEN", "CORRECT_TOKEN_ABC");

    let leave_notify = Arc::new(Notify::new());
    let (bloom, core) = support::bloom::spawn_bloom_ws_with_mock_core(Some(leave_notify.clone()))
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
    let room_id_str = json
        .get("room_id")
        .and_then(|v| v.as_str())
        .expect("room_id");
    let participant_id_str = json
        .get("participant_id")
        .and_then(|v| v.as_str())
        .expect("participant_id");

    ws.send(Message::Close(None)).await.expect("send close");
    drop(ws);

    tokio::time::timeout(std::time::Duration::from_secs(2), leave_notify.notified())
        .await
        .expect("timeout waiting leave_room");

    let (called_room, called_participant) = {
        let core = core.lock().expect("lock core");
        core.leave_room_calls
            .first()
            .cloned()
            .expect("leave_room calls")
    };
    let expected_room = RoomId::from_str(room_id_str).expect("room_id parse");
    let expected_participant =
        ParticipantId::from_str(participant_id_str).expect("participant_id parse");
    assert_eq!(called_room, expected_room);
    assert_eq!(called_participant, expected_participant);
}

#[tokio::test]
async fn abrupt_disconnect_triggers_leave() {
    let _guard = support::EnvGuard::set("SIDECAR_TOKEN", "CORRECT_TOKEN_ABC");

    let leave_notify = Arc::new(Notify::new());
    let (bloom, core) = support::bloom::spawn_bloom_ws_with_mock_core(Some(leave_notify.clone()))
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
    let room_id_str = json
        .get("room_id")
        .and_then(|v| v.as_str())
        .expect("room_id");
    let participant_id_str = json
        .get("participant_id")
        .and_then(|v| v.as_str())
        .expect("participant_id");

    // drop without sending Close
    drop(ws);

    tokio::time::timeout(std::time::Duration::from_secs(2), leave_notify.notified())
        .await
        .expect("timeout waiting leave_room");

    let (called_room, called_participant) = {
        let core = core.lock().expect("lock core");
        core.leave_room_calls
            .first()
            .cloned()
            .expect("leave_room calls")
    };
    let expected_room = RoomId::from_str(room_id_str).expect("room_id parse");
    let expected_participant =
        ParticipantId::from_str(participant_id_str).expect("participant_id parse");
    assert_eq!(called_room, expected_room);
    assert_eq!(called_participant, expected_participant);
}

#[tokio::test]
async fn reconnect_creates_new_session_after_leave() {
    let _guard = support::EnvGuard::set("SIDECAR_TOKEN", "CORRECT_TOKEN_ABC");

    let leave_notify = Arc::new(Notify::new());
    let (bloom, core) =
        support::bloom::spawn_bloom_ws_with_mock_core(Some(leave_notify.clone()))
            .await
            .expect("spawn bloom ws");
    let bloom_ws_url = bloom.ws_url();
    {
        let mut core = core.lock().expect("lock core");
        core.join_room_result = Some(Ok(vec![]));
    }

    let app = sidecar::app::App::new().await.expect("app new");
    let server = support::spawn_axum(app.router())
        .await
        .expect("spawn server");
    let url = Url::parse(&format!("{}/sidecar", server.ws_url(""))).expect("url");

    let mut request_a = build_ws_request(&url);
    request_a
        .headers_mut()
        .insert(AUTHORIZATION, "Bearer CORRECT_TOKEN_ABC".parse().unwrap());
    let (mut ws_a, _resp) = connect_async(request_a)
        .await
        .expect("handshake should succeed (A)");

    let join_payload_a = format!(
        "{{\"type\":\"Join\",\"room_id\":null,\"bloom_ws_url\":\"{}\",\"ice_servers\":[]}}",
        bloom_ws_url
    );
    ws_a.send(Message::Text(join_payload_a))
        .await
        .expect("send join A");
    let msg_a = tokio::time::timeout(std::time::Duration::from_millis(500), ws_a.next())
        .await
        .expect("timeout waiting selfjoined A")
        .expect("stream closed A")
        .expect("ws error A");
    let text_a = match msg_a {
        Message::Text(t) => t,
        other => panic!("unexpected join response A: {:?}", other),
    };
    let json_a: serde_json::Value = serde_json::from_str(&text_a).expect("parse SelfJoined A");
    assert_eq!(
        json_a.get("type").and_then(|v| v.as_str()),
        Some("SelfJoined")
    );
    let room_id = json_a
        .get("room_id")
        .and_then(|v| v.as_str())
        .expect("room_id A")
        .to_string();
    let participant_a = json_a
        .get("participant_id")
        .and_then(|v| v.as_str())
        .expect("participant_id A")
        .to_string();

    ws_a.send(Message::Close(None)).await.expect("send close A");
    drop(ws_a);

    tokio::time::timeout(std::time::Duration::from_secs(2), leave_notify.notified())
        .await
        .expect("timeout waiting leave_room A");

    let leave_calls = {
        let core = core.lock().expect("lock core");
        core.leave_room_calls.len()
    };
    assert!(leave_calls >= 1, "expected leave_room called before rejoin");

    let mut request_b = build_ws_request(&url);
    request_b
        .headers_mut()
        .insert(AUTHORIZATION, "Bearer CORRECT_TOKEN_ABC".parse().unwrap());
    let (mut ws_b, _resp) = connect_async(request_b)
        .await
        .expect("handshake should succeed (B)");

    let join_payload_b = format!(
        "{{\"type\":\"Join\",\"room_id\":\"{}\",\"bloom_ws_url\":\"{}\",\"ice_servers\":[]}}",
        room_id, bloom_ws_url
    );
    ws_b.send(Message::Text(join_payload_b))
        .await
        .expect("send join B");
    let msg_b = tokio::time::timeout(std::time::Duration::from_millis(500), ws_b.next())
        .await
        .expect("timeout waiting selfjoined B")
        .expect("stream closed B")
        .expect("ws error B");
    let text_b = match msg_b {
        Message::Text(t) => t,
        other => panic!("unexpected join response B: {:?}", other),
    };
    let json_b: serde_json::Value = serde_json::from_str(&text_b).expect("parse SelfJoined B");
    assert_eq!(
        json_b.get("type").and_then(|v| v.as_str()),
        Some("SelfJoined")
    );
    let participant_b = json_b
        .get("participant_id")
        .and_then(|v| v.as_str())
        .expect("participant_id B")
        .to_string();

    let join_calls = {
        let core = core.lock().expect("lock core");
        core.join_room_calls.len()
    };
    assert!(join_calls >= 1, "expected join_room called after leave");
    assert_ne!(participant_a, participant_b);
}
