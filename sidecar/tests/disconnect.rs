mod support;

use bloom_core::{ParticipantId, RoomId};
use futures_util::SinkExt;
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

async fn connect_sidecar(
    url: &Url,
) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>> {
    let mut request = build_ws_request(url);
    request
        .headers_mut()
        .insert(AUTHORIZATION, "Bearer CORRECT_TOKEN_ABC".parse().unwrap());
    let (ws, _resp) = connect_async(request)
        .await
        .expect("handshake should succeed");
    ws
}

async fn join_sidecar(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    bloom_ws_url: &str,
    room_id: Option<&str>,
) -> serde_json::Value {
    let join_payload = match room_id {
        Some(room_id) => format!(
            "{{\"type\":\"Join\",\"room_id\":\"{}\",\"bloom_ws_url\":\"{}\",\"ice_servers\":[]}}",
            room_id, bloom_ws_url
        ),
        None => format!(
            "{{\"type\":\"Join\",\"room_id\":null,\"bloom_ws_url\":\"{}\",\"ice_servers\":[]}}",
            bloom_ws_url
        ),
    };
    ws.send(Message::Text(join_payload))
        .await
        .expect("send join");
    let json = support::wait_for_self_joined(ws).await;
    json
}

async fn wait_leave_with_args(
    core: &Arc<std::sync::Mutex<bloom_ws::MockCore>>,
    leave_notify: &Notify,
    expected_room: &RoomId,
    expected_participant: &ParticipantId,
) {
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
    assert_eq!(&called_room, expected_room);
    assert_eq!(&called_participant, expected_participant);
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

    let mut ws = connect_sidecar(&url).await;
    let json = join_sidecar(&mut ws, &bloom_ws_url, None).await;
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

    let expected_room = RoomId::from_str(room_id_str).expect("room_id parse");
    let expected_participant =
        ParticipantId::from_str(participant_id_str).expect("participant_id parse");
    wait_leave_with_args(&core, &leave_notify, &expected_room, &expected_participant).await;
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

    let mut ws = connect_sidecar(&url).await;
    let json = join_sidecar(&mut ws, &bloom_ws_url, None).await;
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

    let expected_room = RoomId::from_str(room_id_str).expect("room_id parse");
    let expected_participant =
        ParticipantId::from_str(participant_id_str).expect("participant_id parse");
    wait_leave_with_args(&core, &leave_notify, &expected_room, &expected_participant).await;
}

#[tokio::test]
async fn reconnect_creates_new_session_after_leave() {
    let _guard = support::EnvGuard::set("SIDECAR_TOKEN", "CORRECT_TOKEN_ABC");

    let leave_notify = Arc::new(Notify::new());
    let (bloom, core) = support::bloom::spawn_bloom_ws_with_mock_core(Some(leave_notify.clone()))
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

    let mut ws_a = connect_sidecar(&url).await;
    let json_a = join_sidecar(&mut ws_a, &bloom_ws_url, None).await;
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

    let expected_room = RoomId::from_str(&room_id).expect("room_id parse");
    let expected_participant =
        ParticipantId::from_str(&participant_a).expect("participant_id parse");
    wait_leave_with_args(&core, &leave_notify, &expected_room, &expected_participant).await;

    let mut ws_b = connect_sidecar(&url).await;
    let json_b = join_sidecar(&mut ws_b, &bloom_ws_url, Some(&room_id)).await;
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

#[tokio::test]
async fn disconnect_clears_syncer_state() {
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

    let mut ws = connect_sidecar(&url).await;
    let json = join_sidecar(&mut ws, &bloom_ws_url, None).await;
    let room_id = json
        .get("room_id")
        .and_then(|v| v.as_str())
        .expect("room_id");

    ws.send(Message::Close(None)).await.expect("send close");
    drop(ws);

    let cleared = sidecar::test_support::wait_for_cleared_room_id(std::time::Duration::from_secs(3))
        .await
        .expect("expected syncer state to be cleared");
    assert_eq!(cleared, room_id);
}
