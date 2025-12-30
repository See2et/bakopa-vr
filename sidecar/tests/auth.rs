use futures_util::{SinkExt, StreamExt};
use http::header::{AUTHORIZATION, ORIGIN};
use tokio::time::{Duration, timeout};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::protocol::Message;

// Spec-ID: TC-000-a/b/c/d/e/f/012 (FR-000, FR-005)
// 認証・Originポリシー・既存セッション保持の振る舞い

async fn start_server(token: &str) -> (sidecar::TestServerHandle, String) {
    unsafe { std::env::set_var("SIDECAR_TOKEN", token) };
    let server = sidecar::run_for_tests("127.0.0.1:0")
        .await
        .expect("server should start");
    let ws_url = format!("ws://{}/sidecar", server.local_addr());
    (server, ws_url)
}

fn build_request(ws_url: &str, auth: Option<&str>, origin: Option<&str>) -> http::Request<()> {
    let mut req = ws_url.into_client_request().expect("valid request");
    if let Some(auth) = auth {
        req.headers_mut()
            .insert(AUTHORIZATION, auth.parse().expect("auth header"));
    }
    if let Some(origin) = origin {
        req.headers_mut()
            .insert(ORIGIN, origin.parse().expect("origin header"));
    }
    req
}

async fn handshake_status(req: http::Request<()>) -> Option<http::StatusCode> {
    match timeout(
        Duration::from_secs(2),
        tokio_tungstenite::connect_async(req),
    )
    .await
    {
        Ok(Ok((_stream, resp))) => Some(resp.status()),
        Ok(Err(tokio_tungstenite::tungstenite::Error::Http(resp))) => Some(resp.status()),
        _ => None,
    }
}

#[tokio::test]
async fn websocket_upgrade_succeeds_with_correct_token() {
    let token = "CORRECT_TOKEN_ABC";
    let (_server, ws_url) = start_server(token).await;
    let req = build_request(&ws_url, Some(&format!("Bearer {}", token)), None);
    let status = handshake_status(req).await;
    assert_eq!(status, Some(http::StatusCode::SWITCHING_PROTOCOLS));
}

#[tokio::test]
async fn websocket_upgrade_rejected_without_authorization() {
    let token = "CORRECT_TOKEN_ABC";
    let (_server, ws_url) = start_server(token).await;
    let req = build_request(&ws_url, None, None);
    let status = handshake_status(req).await;
    assert_eq!(status, Some(http::StatusCode::UNAUTHORIZED));
}

#[tokio::test]
async fn websocket_upgrade_rejected_with_wrong_token() {
    let token = "CORRECT_TOKEN_ABC";
    let (_server, ws_url) = start_server(token).await;
    let req = build_request(&ws_url, Some("Bearer WRONG_TOKEN_XYZ"), None);
    let status = handshake_status(req).await;
    assert_eq!(status, Some(http::StatusCode::UNAUTHORIZED));
}

#[tokio::test]
async fn websocket_upgrade_rejected_with_invalid_scheme() {
    let token = "CORRECT_TOKEN_ABC";
    let (_server, ws_url) = start_server(token).await;
    let req = build_request(&ws_url, Some("Token WRONG_FORMAT"), None);
    let status = handshake_status(req).await;
    assert_eq!(status, Some(http::StatusCode::UNAUTHORIZED));
}

#[tokio::test]
async fn websocket_upgrade_rejected_with_origin() {
    let token = "CORRECT_TOKEN_ABC";
    let (_server, ws_url) = start_server(token).await;
    let req = build_request(
        &ws_url,
        Some(&format!("Bearer {}", token)),
        Some("https://evil.example"),
    );
    let status = handshake_status(req).await;
    assert_eq!(status, Some(http::StatusCode::FORBIDDEN));
}

#[tokio::test]
async fn websocket_upgrade_allows_empty_origin() {
    let token = "CORRECT_TOKEN_ABC";
    let (_server, ws_url) = start_server(token).await;
    let req = build_request(&ws_url, Some(&format!("Bearer {}", token)), Some(""));
    let status = handshake_status(req).await;
    assert_eq!(status, Some(http::StatusCode::SWITCHING_PROTOCOLS));
}

#[tokio::test]
async fn websocket_upgrade_does_not_drop_existing_on_failed_auth() {
    let token = "CORRECT_TOKEN_ABC";
    let (server, ws_url) = start_server(token).await;

    // First, establish a valid connection
    let req1 = build_request(&ws_url, Some(&format!("Bearer {}", token)), None);
    let (mut stream, _resp) = tokio_tungstenite::connect_async(req1)
        .await
        .expect("first connection succeeds");

    // Second, try with wrong token (should fail handshake)
    let req2 = build_request(&ws_url, Some("Bearer WRONG_TOKEN_XYZ"), None);
    let failed = tokio_tungstenite::connect_async(req2).await;
    assert!(matches!(
        failed,
        Err(tokio_tungstenite::tungstenite::Error::Http(response))
            if response.status() == http::StatusCode::UNAUTHORIZED
    ));

    // The first connection should still be alive: send Ping/Pong
    stream
        .send(Message::Ping(vec![1, 2, 3]))
        .await
        .expect("send ping on original session");
    let msg = stream.next().await.expect("receive pong");
    assert!(matches!(msg, Ok(Message::Pong(data)) if data == vec![1, 2, 3]));

    drop(server); // ensure clean shutdown
}
