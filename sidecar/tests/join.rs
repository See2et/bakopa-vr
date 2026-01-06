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
