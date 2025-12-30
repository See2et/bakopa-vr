use tokio::time::{timeout, Duration};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use futures_util::StreamExt;
use futures_util::SinkExt;

// Spec-ID: TC-013 (FR-001/FR-002/FR-004)
// Join 前の SendPose は NotJoined として拒否されることを確認する

#[tokio::test]
async fn send_pose_before_join_is_rejected() {
    let token = "CORRECT_TOKEN_ABC";
    unsafe { std::env::set_var("SIDECAR_TOKEN", token) };

    let server = sidecar::run_for_tests("127.0.0.1:0")
        .await
        .expect("server start");

    let ws_url = format!("ws://{}/sidecar", server.local_addr());
    let mut req = ws_url.into_client_request().expect("req");
    req.headers_mut()
        .insert(http::header::AUTHORIZATION, format!("Bearer {}", token).parse().unwrap());

    let (mut stream, _resp) = tokio_tungstenite::connect_async(req)
        .await
        .expect("ws connect");

    // SendPose JSON (headだけ簡略)
    let payload = serde_json::json!({
        "type": "SendPose",
        "head": {"position": {"x":0.0,"y":0.0,"z":0.0}, "rotation": {"x":0.0,"y":0.0,"z":0.0,"w":1.0}},
        "hand_l": null,
        "hand_r": null
    })
    .to_string();

    stream
        .send(tokio_tungstenite::tungstenite::Message::Text(payload))
        .await
        .expect("send pose");

    let msg = timeout(Duration::from_secs(2), stream.next())
        .await
        .expect("recv timeout")
        .expect("ws msg")
        .expect("ok msg");

    match msg {
        tokio_tungstenite::tungstenite::Message::Text(body) => {
            let v: serde_json::Value = serde_json::from_str(&body).expect("json");
            assert_eq!(v["type"], "Error");
            assert_eq!(v["kind"], "NotJoined");
        }
        other => panic!("unexpected message: {other:?}"),
    }
}

// Spec-ID: TC-001 (FR-001)
// 新規ルームJoinで SelfJoined を受信し、room_id/participant_id が払い出され participants に自分が含まれる
#[tokio::test]
async fn join_creates_room_and_returns_self_joined() {
    let token = "CORRECT_TOKEN_ABC";
    unsafe { std::env::set_var("SIDECAR_TOKEN", token) };

    let server = sidecar::run_for_tests("127.0.0.1:0")
        .await
        .expect("server start");

    let ws_url = format!("ws://{}/sidecar", server.local_addr());
    let mut req = ws_url.into_client_request().expect("req");
    req.headers_mut()
        .insert(http::header::AUTHORIZATION, format!("Bearer {}", token).parse().unwrap());

    let (mut stream, _resp) = tokio_tungstenite::connect_async(req)
        .await
        .expect("ws connect");

    // Join request (room_id null => create new)
    let payload = serde_json::json!({
        "type": "Join",
        "room_id": null,
        "bloom_ws_url": "ws://dummy",
        "ice_servers": []
    })
    .to_string();

    stream
        .send(tokio_tungstenite::tungstenite::Message::Text(payload))
        .await
        .expect("send join");

    let msg = tokio::time::timeout(Duration::from_secs(2), stream.next())
        .await
        .expect("recv timeout")
        .expect("ws msg")
        .expect("ok msg");

    match msg {
        tokio_tungstenite::tungstenite::Message::Text(body) => {
            let v: serde_json::Value = serde_json::from_str(&body).expect("json");
            assert_eq!(v["type"], "SelfJoined");
            let room_id = v["room_id"].as_str().unwrap_or_default();
            let participant_id = v["participant_id"].as_str().unwrap_or_default();
            assert!(!room_id.is_empty(), "room_id must be non-empty");
            assert!(!participant_id.is_empty(), "participant_id must be non-empty");
            assert!(v["participants"].as_array().is_some(), "participants array");
            let list = v["participants"].as_array().unwrap();
            assert_eq!(list.len(), 1);
            assert_eq!(list[0], participant_id);
        }
        other => panic!("unexpected message: {other:?}"),
    }
}

// Spec-ID: TC-002 (FR-001)
// 既存ルーム Join で participants に先行参加者が含まれる
#[tokio::test]
async fn join_existing_room_returns_participants() {
    let token = "CORRECT_TOKEN_ABC";
    unsafe { std::env::set_var("SIDECAR_TOKEN", token) };

    let server = sidecar::run_for_tests("127.0.0.1:0")
        .await
        .expect("server start");
    let ws_url = format!("ws://{}/sidecar", server.local_addr());

    // Client A joins
    let mut req_a = ws_url.clone().into_client_request().expect("reqA");
    req_a.headers_mut()
        .insert(http::header::AUTHORIZATION, format!("Bearer {}", token).parse().unwrap());
    let (mut stream_a, _resp_a) = tokio_tungstenite::connect_async(req_a)
        .await
        .expect("ws connect A");
    let join_payload = serde_json::json!({
        "type": "Join",
        "room_id": null,
        "bloom_ws_url": "ws://dummy",
        "ice_servers": []
    })
    .to_string();
    stream_a
        .send(tokio_tungstenite::tungstenite::Message::Text(join_payload))
        .await
        .expect("send join A");
    let msg_a = tokio::time::timeout(Duration::from_secs(2), stream_a.next())
        .await
        .expect("recv A timeout")
        .expect("ws msg A")
        .expect("ok msg A");
    let (room_id, participant_a) = match msg_a {
        tokio_tungstenite::tungstenite::Message::Text(body) => {
            let v: serde_json::Value = serde_json::from_str(&body).expect("json");
            (v["room_id"].as_str().unwrap().to_string(), v["participant_id"].as_str().unwrap().to_string())
        }
        other => panic!("unexpected message: {other:?}"),
    };

    // Client B joins same room_id
    let mut req_b = ws_url.into_client_request().expect("reqB");
    req_b.headers_mut()
        .insert(http::header::AUTHORIZATION, format!("Bearer {}", token).parse().unwrap());
    let (mut stream_b, _resp_b) = tokio_tungstenite::connect_async(req_b)
        .await
        .expect("ws connect B");
    let join_b = serde_json::json!({
        "type": "Join",
        "room_id": room_id,
        "bloom_ws_url": "ws://dummy",
        "ice_servers": []
    })
    .to_string();
    stream_b
        .send(tokio_tungstenite::tungstenite::Message::Text(join_b))
        .await
        .expect("send join B");
    let msg_b = tokio::time::timeout(Duration::from_secs(2), stream_b.next())
        .await
        .expect("recv B timeout")
        .expect("ws msg B")
        .expect("ok msg B");

    match msg_b {
        tokio_tungstenite::tungstenite::Message::Text(body) => {
            let v: serde_json::Value = serde_json::from_str(&body).expect("json");
            assert_eq!(v["type"], "SelfJoined");
            let participants = v["participants"].as_array().expect("participants array");
            assert_eq!(participants.len(), 2);
            assert!(participants.contains(&serde_json::Value::String(participant_a.clone())));
        }
        other => panic!("unexpected message: {other:?}"),
    }
}
