mod support;

use axum::http::StatusCode;
use reqwest::Client;
use std::net::TcpListener;

#[tokio::test]
async fn default_bind_is_loopback_and_non_sidecar_returns_404() {
    let _guard = support::EnvGuard::set("SIDECAR_TOKEN", "TEST_TOKEN_CONFIG");
    let port = TcpListener::bind("127.0.0.1:0")
        .expect("bind for free port")
        .local_addr()
        .expect("local addr")
        .port();
    let _port_guard = support::EnvGuard::set("SIDECAR_PORT", port.to_string());

    let app = sidecar::app::App::new().await.expect("app new");
    let server = support::spawn_axum_on(app.bind_addr(), app.router())
        .await
        .expect("spawn server");

    assert!(
        server.addr.ip().is_loopback(),
        "expected loopback default bind, got {}",
        server.addr.ip()
    );
    assert_eq!(
        server.addr.port(),
        port,
        "expected bind port to match SIDECAR_PORT"
    );

    let url = format!("http://{}{}", server.addr, "/not-sidecar");
    let response = Client::new()
        .get(url)
        .send()
        .await
        .expect("request should succeed");
    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "expected 404 for non-sidecar path"
    );

    let response = Client::new()
        .get(format!("http://{}{}", server.addr, "/sidecar"))
        .send()
        .await
        .expect("request should succeed");
    assert_eq!(
        response.status(),
        StatusCode::UPGRADE_REQUIRED,
        "expected 426 for /sidecar without upgrade"
    );
}
