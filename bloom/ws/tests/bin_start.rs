use bloom_api::ServerToClient;
use bloom_ws::{start_ws_server, RealCore, SharedCore};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;

/// start_ws_server(RealCore) が起動し、Create→Join が通ることを結合レベルで確認。
#[tokio::test]
async fn start_ws_server_with_real_core_allows_create_and_join() {
    let shared = SharedCore::new(RealCore::new());
    let handle = start_ws_server("0.0.0.0:8080".parse().unwrap(), shared)
        .await
        .expect("start server");
    let url = format!("ws://{}/ws", handle.addr);

    // A: CreateRoom
    let (mut ws_a, _) = connect_async(&url).await.expect("connect A");
    ws_a.send(Message::Text(r#"{"type":"CreateRoom"}"#.into()))
        .await
        .expect("send create");
    let room_created = recv(&mut ws_a).await;
    let (room_id, _a_id) = match room_created {
        ServerToClient::RoomCreated { room_id, self_id } => (room_id, self_id),
        other => panic!("expected RoomCreated, got {:?}", other),
    };

    // B: JoinRoom
    let (mut ws_b, _) = connect_async(&url).await.expect("connect B");
    ws_b.send(Message::Text(format!(
        r#"{{"type":"JoinRoom","room_id":"{room_id}"}}"#
    )))
    .await
    .expect("send join");

    // どちらかでRoomParticipantsを受信できればOK（順序揺れ許容）
    let mut got_participants = false;
    for _ in 0..4 {
        if let Some(evt) = recv_optional(&mut ws_a).await {
            if matches!(evt, ServerToClient::RoomParticipants { .. }) {
                got_participants = true;
                break;
            }
        }
    }
    if !got_participants {
        for _ in 0..4 {
            if let Some(evt) = recv_optional(&mut ws_b).await {
                if matches!(evt, ServerToClient::RoomParticipants { .. }) {
                    got_participants = true;
                    break;
                }
            }
        }
    }
    assert!(
        got_participants,
        "RoomParticipants should be broadcast after join"
    );

    handle.shutdown().await;
}

async fn recv(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> ServerToClient {
    loop {
        if let Some(msg) = ws.next().await {
            let msg = msg.expect("ws message");
            if let Message::Text(t) = msg {
                if let Ok(parsed) = serde_json::from_str::<ServerToClient>(&t) {
                    return parsed;
                }
            }
        } else {
            panic!("ws closed");
        }
    }
}

async fn recv_optional(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> Option<ServerToClient> {
    if let Ok(Some(Ok(Message::Text(t)))) =
        tokio::time::timeout(std::time::Duration::from_millis(300), ws.next()).await
    {
        serde_json::from_str::<ServerToClient>(&t).ok()
    } else {
        None
    }
}
