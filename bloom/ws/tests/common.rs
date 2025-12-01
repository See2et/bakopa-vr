use bloom_api::ServerToClient;
use bloom_ws::{
    start_ws_server, start_ws_server_with_overrides, MockCore, ServerOverrides, SharedCore,
    WsServerHandle,
};
use futures_util::{SinkExt, StreamExt};
use std::sync::{Arc, Mutex};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

#[allow(dead_code)]
pub async fn spawn_bloom_ws_server_with_core<C: bloom_ws::CoreApi + Send + 'static>(
    core: SharedCore<C>,
) -> (String, WsServerHandle) {
    let handle = start_ws_server("127.0.0.1:0".parse().unwrap(), core)
        .await
        .expect("start ws server");
    let url = format!("ws://{}/ws", handle.addr);
    (url, handle)
}

#[allow(dead_code)]
pub async fn spawn_bloom_ws_server_with_core_and_overrides<
    C: bloom_ws::CoreApi + Send + 'static,
>(
    core: SharedCore<C>,
    overrides: ServerOverrides,
) -> (String, WsServerHandle) {
    let handle = start_ws_server_with_overrides("127.0.0.1:0".parse().unwrap(), core, overrides)
        .await
        .expect("start ws server");
    let url = format!("ws://{}/ws", handle.addr);
    (url, handle)
}

pub async fn recv_server_msg(
    ws: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
) -> ServerToClient {
    loop {
        if let Some(msg) = ws.next().await {
            let msg = msg.expect("ws message ok");
            if let Message::Text(t) = msg {
                if let Ok(parsed) = serde_json::from_str::<ServerToClient>(&t) {
                    return parsed;
                }
            }
        } else {
            panic!("ws closed before receiving message");
        }
    }
}

/// CreateRoomするクライアントAとJoinするクライアントBを起動し、ID類を返すヘルパー。
#[allow(dead_code)]
pub async fn setup_room_with_two_clients(
    server_url: &str,
    core_arc: &Arc<Mutex<MockCore>>,
) -> (
    WebSocketStream<MaybeTlsStream<TcpStream>>, // ws_a
    WebSocketStream<MaybeTlsStream<TcpStream>>, // ws_b
    String,                                     // room_id
    String,                                     // a_id
    String,                                     // b_id
) {
    let (mut ws_a, _) = connect_async(server_url).await.expect("connect A");
    ws_a.send(Message::Text(r#"{"type":"CreateRoom"}"#.into()))
        .await
        .expect("send create room");
    let room_created = recv_server_msg(&mut ws_a).await;
    let (room_id, a_id) = match room_created {
        ServerToClient::RoomCreated { room_id, self_id } => (room_id, self_id),
        other => panic!("expected RoomCreated, got {:?}", other),
    };

    let (mut ws_b, _) = connect_async(server_url).await.expect("connect B");
    ws_b.send(Message::Text(format!(
        r#"{{"type":"JoinRoom","room_id":"{room_id}"}}"#,
        room_id = room_id
    )))
    .await
    .expect("send join room");
    // join完了を待つ
    let _ = recv_server_msg(&mut ws_b).await;

    // Join時に付与されたparticipant_idをCore側の記録から取得
    let b_id = {
        let core = core_arc.lock().expect("lock core");
        core.join_room_calls
            .last()
            .map(|(_, p)| p.to_string())
            .expect("b id recorded")
    };

    (ws_a, ws_b, room_id, a_id, b_id)
}

// minimal helpers shared across test files
