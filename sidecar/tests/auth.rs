mod support;

use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::handshake::client::{generate_key, Request};
use tokio_tungstenite::tungstenite::http::{
    header::{AUTHORIZATION, CONNECTION, HOST, SEC_WEBSOCKET_KEY, SEC_WEBSOCKET_VERSION, UPGRADE},
    HeaderValue, StatusCode,
};
use url::Url;

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
    let host_header = {
        let host = url.host_str().unwrap_or("localhost");
        let port = url.port_or_known_default().unwrap_or(80);
        HeaderValue::from_str(&format!("{host}:{port}")).unwrap()
    };
    let request = Request::builder()
        .method("GET")
        .uri(url.as_str())
        .header(HOST, host_header)
        .header(UPGRADE, "websocket")
        .header(CONNECTION, "Upgrade")
        .header(SEC_WEBSOCKET_VERSION, "13")
        .header(SEC_WEBSOCKET_KEY, generate_key())
        .header(AUTHORIZATION, "Bearer WRONG_TOKEN_XYZ")
        .body(())
        .expect("request");

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
