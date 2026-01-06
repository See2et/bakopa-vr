mod support;

#[tokio::test]
async fn sidecar_can_be_constructed() {
    let _guard = support::EnvGuard::set("SIDECAR_TOKEN", "TEST_TOKEN_SMOKE");
    // RED: App skeleton should exist; expecting Ok but module is not yet implemented.
    let app = sidecar::app::App::new().await;
    assert!(app.is_ok(), "App::new should initialize without error");
}

#[tokio::test]
async fn test_server_can_start_and_stop() {
    let _guard = support::EnvGuard::set("SIDECAR_TOKEN", "TEST_TOKEN_SMOKE");
    let app = sidecar::app::App::new().await.expect("app new");
    let router = app.router();
    let server = support::spawn_axum(router).await.expect("spawn server");
    assert!(server.addr.ip().is_loopback());
    // Drop server to abort background task; ensures no panic on drop.
    drop(server);
}
