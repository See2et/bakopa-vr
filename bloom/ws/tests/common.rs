use bloom_api::ServerToClient;
use bloom_ws::{start_ws_server, MockCore, SharedCore, WsServerHandle};
use futures_util::StreamExt;
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tokio_tungstenite::tungstenite::protocol::Message;

pub async fn spawn_bloom_ws_server_with_core(
    core: SharedCore<MockCore>,
) -> (String, WsServerHandle) {
    let handle = start_ws_server("127.0.0.1:0".parse().unwrap(), core)
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

// minimal helpers shared across test files
