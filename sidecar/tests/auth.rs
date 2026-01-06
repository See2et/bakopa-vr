mod support;

use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::handshake::client::{generate_key, Request};
use tokio_tungstenite::tungstenite::http::{
    header::{
        AUTHORIZATION, CONNECTION, HOST, ORIGIN, SEC_WEBSOCKET_KEY, SEC_WEBSOCKET_VERSION, UPGRADE,
    },
    HeaderValue, StatusCode,
};
use tokio_tungstenite::tungstenite::Message;
use url::Url;

use futures_util::SinkExt;

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
async fn ws_rejects_wrong_token_with_401() {
    let _guard = support::EnvGuard::set("SIDECAR_TOKEN", "CORRECT_TOKEN_ABC");

    // Arrange
    let app = sidecar::app::App::new().await.expect("app new");
    let server = support::spawn_axum(app.router())
        .await
        .expect("spawn server");
    let url = Url::parse(&format!("{}/sidecar", server.ws_url(""))).expect("url");

    // Build a WebSocket handshake request with wrong token, providing required WS headers manually.
    let mut request = build_ws_request(&url);
    request
        .headers_mut()
        .insert(AUTHORIZATION, "Bearer WRONG_TOKEN_XYZ".parse().unwrap());

    // Act
    let result: Result<_, _> = connect_async(request).await;

    // Assert
    // Expect handshake to fail with HTTP 401 once auth is implemented.
    let err = result.expect_err("handshake should be rejected");
    let status = match err {
        tokio_tungstenite::tungstenite::Error::Http(resp) => resp.status(),
        other => panic!("expected HTTP error status, got {:?}", other),
    };
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn ws_missing_authorization_header_returns_401() {
    let _guard = support::EnvGuard::set("SIDECAR_TOKEN", "CORRECT_TOKEN_ABC");
    let app = sidecar::app::App::new().await.expect("app new");
    let server = support::spawn_axum(app.router())
        .await
        .expect("spawn server");
    let url = Url::parse(&format!("{}/sidecar", server.ws_url(""))).expect("url");

    let request = build_ws_request(&url);
    let result: Result<_, _> = connect_async(request).await;
    let err = result.expect_err("handshake should be rejected");
    let status = match err {
        tokio_tungstenite::tungstenite::Error::Http(resp) => resp.status(),
        other => panic!("expected HTTP error status, got {:?}", other),
    };
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn ws_malformed_authorization_header_returns_401() {
    let _guard = support::EnvGuard::set("SIDECAR_TOKEN", "CORRECT_TOKEN_ABC");
    let app = sidecar::app::App::new().await.expect("app new");
    let server = support::spawn_axum(app.router())
        .await
        .expect("spawn server");
    let url = Url::parse(&format!("{}/sidecar", server.ws_url(""))).expect("url");

    let mut request = build_ws_request(&url);
    request
        .headers_mut()
        .insert(AUTHORIZATION, "Token WRONG_FORMAT".parse().unwrap());

    let result: Result<_, _> = connect_async(request).await;
    let err = result.expect_err("handshake should be rejected");
    let status = match err {
        tokio_tungstenite::tungstenite::Error::Http(resp) => resp.status(),
        other => panic!("expected HTTP error status, got {:?}", other),
    };
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn ws_with_disallowed_origin_is_rejected_with_403() {
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
    request
        .headers_mut()
        .insert(ORIGIN, "https://evil.example".parse().unwrap());

    let result: Result<_, _> = connect_async(request).await;
    let err = result.expect_err("handshake should be rejected");
    let status = match err {
        tokio_tungstenite::tungstenite::Error::Http(resp) => resp.status(),
        other => panic!("expected HTTP error status, got {:?}", other),
    };
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn ws_without_origin_is_accepted() {
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
    // Deliberately no ORIGIN header.

    let result: Result<_, _> = connect_async(request).await;
    let (_stream, _resp) = result.expect("handshake should succeed");
    // Stream dropped to close connection.
}

// Current Phase: RED (TC-000d)
// Spec: 正しいトークンで101 Switching Protocolsになり、WSが確立する
#[tokio::test]
async fn ws_with_correct_token_and_no_origin_succeeds() {
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
    // No Origin header to allow handshake.

    let result: Result<_, _> = connect_async(request).await;
    let (_ws, response) = result.expect("handshake should succeed");
    assert_eq!(response.status(), StatusCode::SWITCHING_PROTOCOLS);
}

#[tokio::test]
async fn ws_wrong_token_does_not_disconnect_existing_session() {
    let _guard = support::EnvGuard::set("SIDECAR_TOKEN", "CORRECT_TOKEN_ABC");
    let app = sidecar::app::App::new().await.expect("app new");
    let server = support::spawn_axum(app.router())
        .await
        .expect("spawn server");
    let url = Url::parse(&format!("{}/sidecar", server.ws_url(""))).expect("url");

    // Client A: correct token
    let mut request_a = build_ws_request(&url);
    request_a
        .headers_mut()
        .insert(AUTHORIZATION, "Bearer CORRECT_TOKEN_ABC".parse().unwrap());
    let (mut ws_a, _resp_a) = connect_async(request_a)
        .await
        .expect("client A handshake should succeed");

    // Client B: wrong token should be rejected
    let mut request_b = build_ws_request(&url);
    request_b
        .headers_mut()
        .insert(AUTHORIZATION, "Bearer WRONG_TOKEN_XYZ".parse().unwrap());
    let result_b: Result<_, _> = connect_async(request_b).await;
    let err = result_b.expect_err("client B handshake should be rejected");
    let status = match err {
        tokio_tungstenite::tungstenite::Error::Http(resp) => resp.status(),
        other => panic!("expected HTTP error status, got {:?}", other),
    };
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Client A should still be connected.
    ws_a.send(Message::Ping(vec![1, 2, 3]))
        .await
        .expect("client A should remain connected");
}
