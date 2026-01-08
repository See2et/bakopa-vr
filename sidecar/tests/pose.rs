mod support;

use futures_util::{SinkExt, StreamExt};
use syncer::{StreamKind, TransportSendParams};
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
async fn unknown_message_type_is_invalid_payload() {
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
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    let mut joined = false;
    while std::time::Instant::now() < deadline {
        match tokio::time::timeout(std::time::Duration::from_millis(200), ws.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
                    continue;
                };
                if json.get("type").and_then(|v| v.as_str()) == Some("SelfJoined") {
                    joined = true;
                    break;
                }
            }
            Ok(Some(Ok(_))) => {}
            Ok(Some(Err(err))) => panic!("ws error: {err:?}"),
            Ok(None) => break,
            Err(_) => {}
        }
    }
    assert!(joined, "expected SelfJoined within deadline");

    // unknown type
    let unknown_payload = r#"{"type":"MysteryMessage","foo":"bar"}"#;
    ws.send(Message::Text(unknown_payload.into()))
        .await
        .expect("send unknown message");

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    let mut found = false;
    while std::time::Instant::now() < deadline {
        match tokio::time::timeout(std::time::Duration::from_millis(300), ws.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
                    continue;
                };
                if json.get("type").and_then(|v| v.as_str()) == Some("Error")
                    && json.get("kind").and_then(|v| v.as_str()) == Some("InvalidPayload")
                {
                    found = true;
                    break;
                }
            }
            Ok(Some(Ok(_))) => {}
            Ok(Some(Err(err))) => panic!("ws error: {err:?}"),
            Ok(None) => break,
            Err(_) => {}
        }
    }
    assert!(found, "expected Error kind=InvalidPayload within timeout");
}

#[tokio::test]
async fn send_pose_is_forwarded_with_params() {
    let _guard = support::EnvGuard::set("SIDECAR_TOKEN", "CORRECT_TOKEN_ABC");

    let bloom = support::bloom::spawn_bloom_ws()
        .await
        .expect("spawn bloom ws");
    let bloom_ws_url = bloom.ws_url();

    // TODO: inject recording syncer once App supports it
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

    // Join first
    let join_payload = format!(
        "{{\"type\":\"Join\",\"room_id\":null,\"bloom_ws_url\":\"{}\",\"ice_servers\":[]}}",
        bloom_ws_url
    );
    ws.send(Message::Text(join_payload))
        .await
        .expect("send join");

    // Wait SelfJoined
    let msg = tokio::time::timeout(std::time::Duration::from_millis(500), ws.next())
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
        "expected SelfJoined, got: {text}"
    );

    // SendPose
    let pose_payload = r#"{"type":"SendPose","head":{"position":{"x":0.0,"y":1.0,"z":2.0},"rotation":{"x":0.0,"y":0.0,"z":0.0,"w":1.0}},"hand_l":null,"hand_r":null}"#;
    ws.send(Message::Text(pose_payload.into()))
        .await
        .expect("send pose");

    // Expect Syncer send params to be Pose (unordered/unreliable)
    let expected = TransportSendParams::for_stream(StreamKind::Pose);
    let recorded = tokio::time::timeout(std::time::Duration::from_millis(200), async {
        loop {
            if let Some(p) = sidecar::test_support::last_send_params() {
                break p;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("expected syncer to record send params");
    assert_eq!(recorded, expected);
}

#[tokio::test]
async fn pose_received_is_pushed_to_client() {
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
    let text_a = match tokio::time::timeout(std::time::Duration::from_millis(500), ws_a.next())
        .await
        .expect("timeout waiting selfjoined A")
        .expect("stream closed A")
    {
        Ok(Message::Text(t)) => t,
        Ok(other) => panic!("unexpected message A: {:?}", other),
        Err(err) => panic!("ws error A: {err:?}"),
    };
    let json_a: serde_json::Value = serde_json::from_str(&text_a).expect("parse SelfJoined A");
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
    let _ = tokio::time::timeout(std::time::Duration::from_millis(500), ws_b.next())
        .await
        .expect("timeout waiting selfjoined B")
        .expect("stream closed B")
        .expect("ws error B");

    let pose_payload = r#"{"type":"SendPose","head":{"position":{"x":0.0,"y":1.0,"z":2.0},"rotation":{"x":0.0,"y":0.0,"z":0.0,"w":1.0}},"hand_l":null,"hand_r":null}"#;
    ws_a.send(Message::Text(pose_payload.into()))
        .await
        .expect("send pose A");

    let received = tokio::time::timeout(std::time::Duration::from_millis(200), ws_b.next())
        .await
        .expect("timeout waiting PoseReceived")
        .expect("stream closed B")
        .expect("ws error B");
    let text_b = match received {
        Message::Text(t) => t,
        other => panic!("unexpected message B: {:?}", other),
    };
    let json_b: serde_json::Value = serde_json::from_str(&text_b).expect("parse PoseReceived");
    assert_eq!(
        json_b.get("type").and_then(|v| v.as_str()),
        Some("PoseReceived")
    );
    assert_eq!(
        json_b.get("from").and_then(|v| v.as_str()),
        Some(participant_a.as_str())
    );
    assert!(json_b.get("pose").is_some(), "pose is required");
}

#[tokio::test]
async fn rate_limit_emits_rate_limited() {
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

    let pose_payload = r#"{"type":"SendPose","head":{"position":{"x":0.0,"y":1.0,"z":2.0},"rotation":{"x":0.0,"y":0.0,"z":0.0,"w":1.0}},"hand_l":null,"hand_r":null}"#;
    for _ in 0..21 {
        ws.send(Message::Text(pose_payload.into()))
            .await
            .expect("send pose");
    }

    let received = tokio::time::timeout(std::time::Duration::from_millis(500), ws.next())
        .await
        .expect("timeout waiting RateLimited")
        .expect("stream closed")
        .expect("ws error");
    let text = match received {
        Message::Text(t) => t,
        other => panic!("unexpected message: {:?}", other),
    };
    let json: serde_json::Value = serde_json::from_str(&text).expect("parse RateLimited");
    assert_eq!(
        json.get("type").and_then(|v| v.as_str()),
        Some("RateLimited")
    );
    assert_eq!(
        json.get("stream_kind").and_then(|v| v.as_str()),
        Some("pose")
    );
}

#[tokio::test]
async fn rate_limit_recovery_allows_send() {
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

    let pose_payload = r#"{"type":"SendPose","head":{"position":{"x":0.0,"y":1.0,"z":2.0},"rotation":{"x":0.0,"y":0.0,"z":0.0,"w":1.0}},"hand_l":null,"hand_r":null}"#;
    for _ in 0..21 {
        ws_a.send(Message::Text(pose_payload.into()))
            .await
            .expect("send pose");
    }

    let rate_limited = tokio::time::timeout(std::time::Duration::from_millis(500), ws_a.next())
        .await
        .expect("timeout waiting RateLimited")
        .expect("stream closed A")
        .expect("ws error A");
    let rate_text = match rate_limited {
        Message::Text(t) => t,
        other => panic!("unexpected rate limit message: {:?}", other),
    };
    let rate_json: serde_json::Value = serde_json::from_str(&rate_text).expect("parse RateLimited");
    assert_eq!(
        rate_json.get("type").and_then(|v| v.as_str()),
        Some("RateLimited")
    );

    // drain any pending PoseReceived for B (ignore other messages)
    let mut pending_texts: Vec<String> = Vec::new();
    loop {
        match tokio::time::timeout(std::time::Duration::from_millis(50), ws_b.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                let parsed = serde_json::from_str::<serde_json::Value>(&text)
                    .ok()
                    .and_then(|v| {
                        v.get("type")
                            .and_then(|t| t.as_str())
                            .map(|t| t.to_string())
                    });
                if parsed.as_deref() == Some("PoseReceived") {
                    continue;
                }
                pending_texts.push(text);
                break;
            }
            Ok(Some(Ok(_))) => {}
            Ok(Some(Err(err))) => panic!("ws error B: {err:?}"),
            Ok(None) => break,
            Err(_) => break,
        }
    }

    // retry until recovery is observed or timeout (avoid fixed sleep)
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    let mut recovered_text: Option<String> = pending_texts.pop();
    while std::time::Instant::now() < deadline && recovered_text.is_none() {
        ws_a.send(Message::Text(pose_payload.into()))
            .await
            .expect("send pose after recovery");

        match tokio::time::timeout(std::time::Duration::from_millis(300), ws_b.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                let parsed = serde_json::from_str::<serde_json::Value>(&text)
                    .ok()
                    .and_then(|v| {
                        v.get("type")
                            .and_then(|t| t.as_str())
                            .map(|t| t.to_string())
                    });
                if parsed.as_deref() == Some("PoseReceived") {
                    recovered_text = Some(text);
                    break;
                }
            }
            Ok(Some(Ok(_))) => {}
            Ok(Some(Err(err))) => panic!("ws error B: {err:?}"),
            Ok(None) => break,
            Err(_) => {}
        }
    }

    let recovered_text = recovered_text.expect("expected PoseReceived after recovery");
    let recovered_json: serde_json::Value =
        serde_json::from_str(&recovered_text).expect("parse PoseReceived");
    assert_eq!(
        recovered_json.get("type").and_then(|v| v.as_str()),
        Some("PoseReceived")
    );
    assert_eq!(
        recovered_json.get("from").and_then(|v| v.as_str()),
        Some(participant_a.as_str())
    );
}

// Current Phase: RED (TC-007)
// Spec: 不正 SendPose は Error { kind="InvalidPayload" } を返す
#[tokio::test]
async fn invalid_payload_is_reported() {
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

    // invalid payload: missing head
    let invalid_payload = r#"{"type":"SendPose","hand_l":null,"hand_r":null}"#;
    ws.send(Message::Text(invalid_payload.into()))
        .await
        .expect("send invalid pose");

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    let mut found = false;
    while std::time::Instant::now() < deadline {
        match tokio::time::timeout(std::time::Duration::from_millis(300), ws.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
                    continue;
                };
                if json.get("type").and_then(|v| v.as_str()) == Some("Error")
                    && json.get("kind").and_then(|v| v.as_str()) == Some("InvalidPayload")
                {
                    found = true;
                    break;
                }
            }
            Ok(Some(Ok(_))) => {}
            Ok(Some(Err(err))) => panic!("ws error: {err:?}"),
            Ok(None) => break,
            Err(_) => {}
        }
    }
    assert!(found, "expected Error kind=InvalidPayload within timeout");
}

#[tokio::test]
async fn nan_or_inf_is_invalid() {
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

    let invalid_payload = r#"{"type":"SendPose","head":{"position":{"x":1e309,"y":1.0,"z":2.0},"rotation":{"x":0.0,"y":0.0,"z":0.0,"w":1.0}},"hand_l":null,"hand_r":null}"#;
    ws.send(Message::Text(invalid_payload.into()))
        .await
        .expect("send invalid pose");

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    let mut found = false;
    while std::time::Instant::now() < deadline {
        match tokio::time::timeout(std::time::Duration::from_millis(300), ws.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
                    continue;
                };
                if json.get("type").and_then(|v| v.as_str()) == Some("Error")
                    && json.get("kind").and_then(|v| v.as_str()) == Some("InvalidPayload")
                {
                    found = true;
                    break;
                }
            }
            Ok(Some(Ok(_))) => {}
            Ok(Some(Err(err))) => panic!("ws error: {err:?}"),
            Ok(None) => break,
            Err(_) => {}
        }
    }
    assert!(found, "expected Error kind=InvalidPayload within timeout");
}
