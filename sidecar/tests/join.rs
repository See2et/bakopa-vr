use tokio::time::{timeout, Duration};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use futures_util::StreamExt;
use futures_util::SinkExt;
use tokio::time::sleep;

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

// Spec-ID: TC-003 (FR-002)
// Join 後の SendPose がサーバ側に送信記録される
#[tokio::test]
async fn send_pose_after_join_records_payload() {
    let token = "CORRECT_TOKEN_ABC";
    unsafe { std::env::set_var("SIDECAR_TOKEN", token) };

    let server = sidecar::run_for_tests("127.0.0.1:0")
        .await
        .expect("server start");
    let ws_url = format!("ws://{}/sidecar", server.local_addr());

    let mut req = ws_url.clone().into_client_request().expect("req");
    req.headers_mut()
        .insert(http::header::AUTHORIZATION, format!("Bearer {}", token).parse().unwrap());
    let (mut stream, _resp) = tokio_tungstenite::connect_async(req)
        .await
        .expect("ws connect");

    // Join
    let join_payload = serde_json::json!({
        "type": "Join",
        "room_id": null,
        "bloom_ws_url": "ws://dummy",
        "ice_servers": []
    })
    .to_string();
    stream
        .send(tokio_tungstenite::tungstenite::Message::Text(join_payload))
        .await
        .expect("send join");
    let _ = tokio::time::timeout(Duration::from_secs(2), stream.next())
        .await
        .expect("recv join")
        .expect("ws msg")
        .expect("ok msg");

    // SendPose
    let pose_payload = serde_json::json!({
        "type": "SendPose",
        "head": {"position": {"x":1.0,"y":2.0,"z":3.0}, "rotation": {"x":0.0,"y":0.0,"z":0.0,"w":1.0}},
        "hand_l": null,
        "hand_r": null
    })
    .to_string();
    stream
        .send(tokio_tungstenite::tungstenite::Message::Text(pose_payload))
        .await
        .expect("send pose");

    // Give server a moment to record
    tokio::time::sleep(Duration::from_millis(50)).await;
    let poses = server.sent_poses().await;
    assert_eq!(poses.len(), 1);
    let (_from, payload) = &poses[0];
    assert_eq!(payload["type"], "SendPose");
    assert_eq!(payload["head"]["position"]["x"], 1.0);
}

// Spec-ID: TC-014 (FR-004/FR-002 入力検証)
// Pose に NaN が含まれると InvalidPayload となり記録されない
#[tokio::test]
async fn send_pose_with_nan_rejected_as_invalid_payload() {
    let token = "CORRECT_TOKEN_ABC";
    unsafe { std::env::set_var("SIDECAR_TOKEN", token) };

    let server = sidecar::run_for_tests("127.0.0.1:0")
        .await
        .expect("server start");
    let ws_url = format!("ws://{}/sidecar", server.local_addr());

    let mut req = ws_url.clone().into_client_request().expect("req");
    req.headers_mut()
        .insert(http::header::AUTHORIZATION, format!("Bearer {}", token).parse().unwrap());
    let (mut stream, _resp) = tokio_tungstenite::connect_async(req)
        .await
        .expect("ws connect");

    // Join
    let join_payload = serde_json::json!({
        "type": "Join",
        "room_id": null,
        "bloom_ws_url": "ws://dummy",
        "ice_servers": []
    })
    .to_string();
    stream
        .send(tokio_tungstenite::tungstenite::Message::Text(join_payload))
        .await
        .expect("send join");
    let _ = tokio::time::timeout(Duration::from_secs(2), stream.next())
        .await
        .expect("recv join")
        .expect("ws msg")
        .expect("ok msg");

    // Invalid Pose: NaN in position.x (JSON literal NaN is invalid, so we send as string and expect InvalidPayload)
    let invalid_payload = r#"{
        "type": "SendPose",
        "head": {"position": {"x": NaN, "y":0.0, "z":0.0}, "rotation": {"x":0.0,"y":0.0,"z":0.0,"w":1.0}},
        "hand_l": null,
        "hand_r": null
    }"#;

    stream
        .send(tokio_tungstenite::tungstenite::Message::Text(invalid_payload.to_string()))
        .await
        .expect("send invalid pose");

    let msg = tokio::time::timeout(Duration::from_secs(2), stream.next())
        .await
        .expect("recv timeout")
        .expect("ws msg")
        .expect("ok msg");

    match msg {
        tokio_tungstenite::tungstenite::Message::Text(body) => {
            let v: serde_json::Value = serde_json::from_str(&body).expect("json");
            assert_eq!(v["type"], "Error");
            assert_eq!(v["kind"], "InvalidPayload");
        }
        other => panic!("unexpected message: {other:?}"),
    }

    // 確認: 記録されない
    let poses = server.sent_poses().await;
    assert_eq!(poses.len(), 0);
}

// Spec-ID: TC-007 (FR-004)
// 未知kindや必須フィールド欠損は InvalidPayload となり記録されない
#[tokio::test]
async fn unknown_message_kind_is_invalid_payload() {
    let token = "CORRECT_TOKEN_ABC";
    unsafe { std::env::set_var("SIDECAR_TOKEN", token) };

    let server = sidecar::run_for_tests("127.0.0.1:0")
        .await
        .expect("server start");
    let ws_url = format!("ws://{}/sidecar", server.local_addr());

    let mut req = ws_url.clone().into_client_request().expect("req");
    req.headers_mut()
        .insert(http::header::AUTHORIZATION, format!("Bearer {}", token).parse().unwrap());
    let (mut stream, _resp) = tokio_tungstenite::connect_async(req)
        .await
        .expect("ws connect");

    // Join
    let join_payload = serde_json::json!({
        "type": "Join",
        "room_id": null,
        "bloom_ws_url": "ws://dummy",
        "ice_servers": []
    })
    .to_string();
    stream
        .send(tokio_tungstenite::tungstenite::Message::Text(join_payload))
        .await
        .expect("send join");
    let _ = tokio::time::timeout(Duration::from_secs(2), stream.next())
        .await
        .expect("recv join")
        .expect("ws msg")
        .expect("ok msg");

    // Unknown message kind
    let unknown = serde_json::json!({ "type": "FooBar" }).to_string();
    stream
        .send(tokio_tungstenite::tungstenite::Message::Text(unknown))
        .await
        .expect("send unknown");

    let msg = tokio::time::timeout(Duration::from_secs(2), stream.next())
        .await
        .expect("recv timeout")
        .expect("ws msg")
        .expect("ok msg");

    match msg {
        tokio_tungstenite::tungstenite::Message::Text(body) => {
            let v: serde_json::Value = serde_json::from_str(&body).expect("json");
            assert_eq!(v["type"], "Error");
            assert_eq!(v["kind"], "InvalidPayload");
        }
        other => panic!("unexpected message: {other:?}"),
    }

    // 記録されない
    let poses = server.sent_poses().await;
    assert!(poses.is_empty());
}

// Spec-ID: TC-004 (FR-003)
// 2クライアントで、AのSendPoseがBにPoseReceivedとして届く（自分自身には届かない）
#[tokio::test]
async fn pose_is_broadcast_to_other_participants() {
    let token = "CORRECT_TOKEN_ABC";
    unsafe { std::env::set_var("SIDECAR_TOKEN", token) };

    let server = sidecar::run_for_tests("127.0.0.1:0")
        .await
        .expect("server start");
    let ws_url = format!("ws://{}/sidecar", server.local_addr());

    // Client A join
    let (mut stream_a, participant_a) = join_and_get_participant(&ws_url, token).await;
    // Client B join
    let (mut stream_b, participant_b) = join_and_get_participant(&ws_url, token).await;
    assert_ne!(participant_a, participant_b);

    // A sends pose
    let pose_payload = serde_json::json!({
        "type": "SendPose",
        "head": {"position": {"x":1.0,"y":2.0,"z":3.0}, "rotation": {"x":0.0,"y":0.0,"z":0.0,"w":1.0}},
        "hand_l": null,
        "hand_r": null
    })
    .to_string();
    stream_a
        .send(tokio_tungstenite::tungstenite::Message::Text(pose_payload))
        .await
        .expect("send pose");

    // B should receive PoseReceived
    let msg_b = tokio::time::timeout(Duration::from_secs(2), stream_b.next())
        .await
        .expect("recv B timeout")
        .expect("ws msg B")
        .expect("ok msg B");
    match msg_b {
        tokio_tungstenite::tungstenite::Message::Text(body) => {
            let v: serde_json::Value = serde_json::from_str(&body).expect("json");
            assert_eq!(v["type"], "PoseReceived");
            assert_eq!(v["from"], participant_a);
            assert_eq!(v["pose"]["head"]["position"]["x"], 1.0);
        }
        other => panic!("unexpected message to B: {other:?}"),
    }

    // A should NOT receive its own PoseReceived within short timeout
    let res_a = tokio::time::timeout(Duration::from_millis(200), stream_a.next()).await;
    assert!(res_a.is_err(), "A should not receive its own pose");
}

// Spec-ID: TC-005 (FR-004 boundary)
// 1秒に21件送ると RateLimited が返り、超過分は記録されない
#[tokio::test]
async fn rate_limit_triggers_at_21_per_second() {
    let token = "CORRECT_TOKEN_ABC";
    unsafe { std::env::set_var("SIDECAR_TOKEN", token) };

    let server = sidecar::run_for_tests("127.0.0.1:0")
        .await
        .expect("server start");
    let ws_url = format!("ws://{}/sidecar", server.local_addr());

    let (mut stream, _pid) = join_and_get_participant(&ws_url, token).await;

    let pose_payload = serde_json::json!({
        "type": "SendPose",
        "head": {"position": {"x":1.0,"y":2.0,"z":3.0}, "rotation": {"x":0.0,"y":0.0,"z":0.0,"w":1.0}},
        "hand_l": null,
        "hand_r": null
    })
    .to_string();

    // send 21 quickly
    for _ in 0..21 {
        stream
            .send(tokio_tungstenite::tungstenite::Message::Text(pose_payload.clone()))
            .await
            .expect("send pose");
    }

    // expect RateLimited
    let msg = tokio::time::timeout(Duration::from_secs(2), stream.next())
        .await
        .expect("recv timeout")
        .expect("ws msg")
        .expect("ok msg");
    match msg {
        tokio_tungstenite::tungstenite::Message::Text(body) => {
            let v: serde_json::Value = serde_json::from_str(&body).expect("json");
            assert_eq!(v["type"], "RateLimited");
            assert_eq!(v["kind"], "RateLimited");
            assert_eq!(v["stream_kind"], "pose");
        }
        other => panic!("unexpected message: {other:?}"),
    }

    let poses = server.sent_poses().await;
    assert_eq!(poses.len(), 20); // only 20 accepted
}

// Spec-ID: TC-006 (FR-002/FR-004 boundary)
// レートリミット後、1秒待てば再び送れる
#[tokio::test]
async fn rate_limit_recovers_after_wait() {
    let token = "CORRECT_TOKEN_ABC";
    unsafe { std::env::set_var("SIDECAR_TOKEN", token) };

    let server = sidecar::run_for_tests("127.0.0.1:0")
        .await
        .expect("server start");
    let ws_url = format!("ws://{}/sidecar", server.local_addr());

    let (mut stream, _pid) = join_and_get_participant(&ws_url, token).await;

    let pose_payload = serde_json::json!({
        "type": "SendPose",
        "head": {"position": {"x":1.0,"y":2.0,"z":3.0}, "rotation": {"x":0.0,"y":0.0,"z":0.0,"w":1.0}},
        "hand_l": null,
        "hand_r": null
    })
    .to_string();

    for _ in 0..21 {
        stream
            .send(tokio_tungstenite::tungstenite::Message::Text(pose_payload.clone()))
            .await
            .expect("send pose");
    }
    // consume RateLimited
    let _ = tokio::time::timeout(Duration::from_secs(2), stream.next())
        .await
        .expect("recv timeout");

    // wait for 1s window to slide
    sleep(Duration::from_millis(1100)).await;

    // send one more
    stream
        .send(tokio_tungstenite::tungstenite::Message::Text(pose_payload.clone()))
        .await
        .expect("send pose again");

    // Expect no RateLimited; if any message arrives, ensure it's not RateLimited
    let res = tokio::time::timeout(Duration::from_millis(300), stream.next()).await;
    if let Ok(Some(Ok(tokio_tungstenite::tungstenite::Message::Text(body)))) = res {
        let v: serde_json::Value = serde_json::from_str(&body).expect("json");
        assert_ne!(v["type"], "RateLimited");
    }

    let poses = server.sent_poses().await;
    assert_eq!(poses.len(), 21); // 20 accepted + 1 after recovery
}
async fn join_and_get_participant(
    ws_url: &str,
    token: &str,
) -> (
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    String,
) {
    let mut req = ws_url.into_client_request().expect("req");
    req.headers_mut()
        .insert(http::header::AUTHORIZATION, format!("Bearer {}", token).parse().unwrap());
    let (mut stream, _resp) = tokio_tungstenite::connect_async(req)
        .await
        .expect("ws connect");

    let join_payload = serde_json::json!({
        "type": "Join",
        "room_id": null,
        "bloom_ws_url": "ws://dummy",
        "ice_servers": []
    })
    .to_string();
    stream
        .send(tokio_tungstenite::tungstenite::Message::Text(join_payload))
        .await
        .expect("send join");
    let msg = tokio::time::timeout(Duration::from_secs(2), stream.next())
        .await
        .expect("recv join")
        .expect("ws msg")
        .expect("ok msg");
    match msg {
        tokio_tungstenite::tungstenite::Message::Text(body) => {
            let v: serde_json::Value = serde_json::from_str(&body).expect("json");
            let pid = v["participant_id"].as_str().unwrap().to_string();
            (stream, pid)
        }
        other => panic!("unexpected join response: {other:?}"),
    }
}
