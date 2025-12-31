use tokio_tungstenite::tungstenite::client::IntoClientRequest;

// Spec-ID: TC-011 (NFR-002)
// デフォルトバインド 127.0.0.1:0 で /sidecar 以外は 404 になる
#[tokio::test]
async fn other_path_returns_404() {
    let token = "CORRECT_TOKEN_ABC";
    unsafe { std::env::set_var("SIDECAR_TOKEN", token) };

    let server = sidecar::run_for_tests("127.0.0.1:0")
        .await
        .expect("server start");
    let ws_url = format!("ws://{}/other", server.local_addr());
    let mut req = ws_url.into_client_request().expect("req");
    req.headers_mut()
        .insert(http::header::AUTHORIZATION, format!("Bearer {}", token).parse().unwrap());

    let res = tokio_tungstenite::connect_async(req).await;
    match res {
        Err(tokio_tungstenite::tungstenite::Error::Http(resp)) => {
            assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);
        }
        other => panic!("expected 404 Http error, got {other:?}"),
    }
}
