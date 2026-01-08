mod support;

use futures_util::{SinkExt, StreamExt};
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

async fn wait_for_message_type(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    message_type: &str,
) -> Option<serde_json::Value> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while std::time::Instant::now() < deadline {
        match tokio::time::timeout(std::time::Duration::from_millis(300), ws.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
                    continue;
                };
                if json.get("type").and_then(|v| v.as_str()) == Some(message_type) {
                    return Some(json);
                }
            }
            Ok(Some(Ok(_))) => {}
            Ok(Some(Err(err))) => panic!("ws error: {err:?}"),
            Ok(None) => break,
            Err(_) => {}
        }
    }
    None
}

#[tokio::test]
async fn webrtc_e2e_minimal_roundtrip() {
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
    let (mut ws_a, _resp) = connect_async(request_a).await.expect("handshake A");

    let join_payload_a = format!(
        "{{\"type\":\"Join\",\"room_id\":null,\"bloom_ws_url\":\"{}\",\"ice_servers\":[]}}",
        bloom_ws_url
    );
    ws_a.send(Message::Text(join_payload_a))
        .await
        .expect("send join A");
    let json_a = support::wait_for_self_joined(&mut ws_a).await;
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
    let (mut ws_b, _resp) = connect_async(request_b).await.expect("handshake B");

    let join_payload_b = format!(
        "{{\"type\":\"Join\",\"room_id\":\"{}\",\"bloom_ws_url\":\"{}\",\"ice_servers\":[]}}",
        room_id, bloom_ws_url
    );
    ws_b.send(Message::Text(join_payload_b))
        .await
        .expect("send join B");
    let json_b = support::wait_for_self_joined(&mut ws_b).await;
    let participant_b = json_b
        .get("participant_id")
        .and_then(|v| v.as_str())
        .expect("participant_id B")
        .to_string();

    let offer_payload = format!(
        "{{\"type\":\"Offer\",\"to\":\"{}\",\"sdp\":\"dummy-offer\"}}",
        participant_b
    );
    ws_a.send(Message::Text(offer_payload))
        .await
        .expect("send offer");
    let offer = wait_for_message_type(&mut ws_b, "Offer")
        .await
        .expect("expected Offer via sidecar");
    assert_eq!(
        offer.get("from").and_then(|v| v.as_str()),
        Some(participant_a.as_str())
    );

    let answer_payload = format!(
        "{{\"type\":\"Answer\",\"to\":\"{}\",\"sdp\":\"dummy-answer\"}}",
        participant_a
    );
    ws_b.send(Message::Text(answer_payload))
        .await
        .expect("send answer");
    let answer = wait_for_message_type(&mut ws_a, "Answer")
        .await
        .expect("expected Answer via sidecar");
    assert_eq!(
        answer.get("from").and_then(|v| v.as_str()),
        Some(participant_b.as_str())
    );

    let ice_payload = format!(
        "{{\"type\":\"IceCandidate\",\"to\":\"{}\",\"candidate\":\"dummy-candidate\"}}",
        participant_b
    );
    ws_a.send(Message::Text(ice_payload))
        .await
        .expect("send ice");
    let ice = wait_for_message_type(&mut ws_b, "IceCandidate")
        .await
        .expect("expected IceCandidate via sidecar");
    assert_eq!(
        ice.get("from").and_then(|v| v.as_str()),
        Some(participant_a.as_str())
    );

    let pose_payload = r#"{"type":"SendPose","head":{"position":{"x":0.0,"y":1.0,"z":2.0},"rotation":{"x":0.0,"y":0.0,"z":0.0,"w":1.0}},"hand_l":null,"hand_r":null}"#;
    ws_a.send(Message::Text(pose_payload.into()))
        .await
        .expect("send pose A");
    let pose = wait_for_message_type(&mut ws_b, "PoseReceived")
        .await
        .expect("expected PoseReceived");
    assert_eq!(
        pose.get("from").and_then(|v| v.as_str()),
        Some(participant_a.as_str())
    );
}

#[tokio::test]
async fn webrtc_e2e_requires_real_transport() {
    let _guard = support::EnvGuard::set("SIDECAR_TOKEN", "CORRECT_TOKEN_ABC");
    let _transport = support::EnvGuard::set("SIDECAR_TRANSPORT", "webrtc");

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
    let (mut ws, _resp) = connect_async(request).await.expect("handshake");

    let join_payload = format!(
        "{{\"type\":\"Join\",\"room_id\":null,\"bloom_ws_url\":\"{}\",\"ice_servers\":[]}}",
        bloom_ws_url
    );
    ws.send(Message::Text(join_payload))
        .await
        .expect("send join");
    let _ = support::wait_for_self_joined(&mut ws).await;

    let transport = sidecar::test_support::last_transport_kind()
        .expect("expected transport kind to be recorded");
    assert_eq!(transport, "webrtc");
}
