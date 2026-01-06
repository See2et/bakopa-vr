mod support;

use std::time::Duration;

use tokio::time::timeout;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::handshake::client::{generate_key, Request};
use tokio_tungstenite::tungstenite::http::{
    header::{AUTHORIZATION, CONNECTION, HOST, SEC_WEBSOCKET_KEY, SEC_WEBSOCKET_VERSION, UPGRADE},
    HeaderValue,
};
use tokio_tungstenite::tungstenite::Message;
use url::Url;

use futures_util::{SinkExt, StreamExt};

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

async fn recv_room_created(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> (String, String) {
    let msg = timeout(Duration::from_millis(500), ws.next())
        .await
        .expect("timeout waiting for RoomCreated")
        .expect("stream closed");
    let text = match msg {
        Ok(Message::Text(t)) => t,
        Ok(other) => panic!("unexpected message: {:?}", other),
        Err(err) => panic!("ws error: {err:?}"),
    };
    let value: serde_json::Value = serde_json::from_str(&text).expect("parse json");
    let msg_type = value.get("type").and_then(|v| v.as_str());
    if msg_type != Some("RoomCreated") {
        panic!("expected RoomCreated, got: {text}");
    }
    let room_id = value
        .get("room_id")
        .and_then(|v| v.as_str())
        .expect("room_id")
        .to_string();
    let self_id = value
        .get("self_id")
        .and_then(|v| v.as_str())
        .expect("self_id")
        .to_string();
    (room_id, self_id)
}

#[tokio::test]
async fn send_pose_before_join_returns_not_joined() {
    let _guard = support::EnvGuard::set("SIDECAR_TOKEN", "CORRECT_TOKEN_ABC");
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

    // SendPose before Join.
    let payload = r#"{"type":"SendPose","head":{},"hand_l":null,"hand_r":null}"#;
    ws.send(Message::Text(payload.into())).await.expect("send");

    // Expect Error { kind="NotJoined", ... }
    let msg = timeout(Duration::from_millis(200), ws.next())
        .await
        .expect("timeout waiting for response")
        .expect("stream closed");
    let text = match msg {
        Ok(Message::Text(t)) => t,
        Ok(other) => panic!("unexpected message: {:?}", other),
        Err(err) => panic!("ws error: {err:?}"),
    };
    assert!(
        text.contains("NotJoined"),
        "expected NotJoined error, got: {text}"
    );
}

#[tokio::test]
async fn join_without_room_creates_room_and_selfjoined() {
    let _guard = support::EnvGuard::set("SIDECAR_TOKEN", "CORRECT_TOKEN_ABC");
    let app = sidecar::app::App::new().await.expect("app new");
    let server = support::spawn_axum(app.router())
        .await
        .expect("spawn server");
    let url = Url::parse(&format!("{}/sidecar", server.ws_url(""))).expect("url");

    let bloom = support::bloom::spawn_bloom_ws()
        .await
        .expect("spawn bloom ws");
    let bloom_ws_url = bloom.ws_url();

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

    let msg = timeout(Duration::from_millis(500), ws.next())
        .await
        .expect("timeout waiting for selfjoined")
        .expect("stream closed");
    let text = match msg {
        Ok(Message::Text(t)) => t,
        Ok(other) => panic!("unexpected message: {:?}", other),
        Err(err) => panic!("ws error: {err:?}"),
    };
    assert!(
        text.contains("SelfJoined"),
        "expected SelfJoined response, got: {text}"
    );
}

// Current Phase: RED (TC-002)
// Spec: 既存ルームJoinでparticipantsに既存参加者が含まれる
#[tokio::test]
async fn join_existing_room_returns_participants() {
    let _guard = support::EnvGuard::set("SIDECAR_TOKEN", "CORRECT_TOKEN_ABC");

    let bloom = support::bloom::spawn_bloom_ws()
        .await
        .expect("spawn bloom ws");
    let bloom_ws_url = bloom.ws_url();

    // Participant X: CreateRoom
    let (mut ws_x, _resp_x) = connect_async(&bloom_ws_url)
        .await
        .expect("connect bloom ws");
    ws_x.send(Message::Text(r#"{"type":"CreateRoom"}"#.into()))
        .await
        .expect("send create room");
    let (room_id, participant_x) = recv_room_created(&mut ws_x).await;

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
        "{{\"type\":\"Join\",\"room_id\":\"{}\",\"bloom_ws_url\":\"{}\",\"ice_servers\":[]}}",
        room_id, bloom_ws_url
    );
    ws.send(Message::Text(join_payload))
        .await
        .expect("send join");

    let msg = timeout(Duration::from_millis(500), ws.next())
        .await
        .expect("timeout waiting for selfjoined")
        .expect("stream closed");
    let text = match msg {
        Ok(Message::Text(t)) => t,
        Ok(other) => panic!("unexpected message: {:?}", other),
        Err(err) => panic!("ws error: {err:?}"),
    };
    assert!(
        text.contains(&participant_x),
        "expected participants to include {participant_x}, got: {text}"
    );
}
