#[path = "common.rs"]
mod common;

use bloom_api::{ErrorCode, ServerToClient};
use bloom_ws::{RealCore, SharedCore};
use futures_util::SinkExt;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;

use common::*;

/// RealCore 環境でも rate limit が適用されることを確認する。
#[tokio::test]
async fn rate_limit_blocks_21st_message_with_real_core() {
    let shared = SharedCore::new(RealCore::new());
    let (server_url, handle) = spawn_bloom_ws_server_with_core(shared).await;

    // 単一接続で 21 個の IceCandidate を連続送信
    let (mut ws, _) = connect_async(&server_url).await.expect("connect");
    ws.send(Message::Text(r#"{"type":"CreateRoom"}"#.into()))
        .await
        .expect("create room");
    let _ = recv_server_msg(&mut ws).await;

    for _ in 0..20 {
        ws.send(Message::Text(
            r#"{"type":"IceCandidate","to":"00000000-0000-0000-0000-000000000000","candidate":"c"}"#.into(),
        ))
        .await
        .expect("send ice");
        let _ = recv_server_msg(&mut ws).await; // エラーや何か返るかもしれないが無視
    }

    // 21発目で RateLimited を期待
    ws.send(Message::Text(
        r#"{"type":"IceCandidate","to":"00000000-0000-0000-0000-000000000000","candidate":"c"}"#.into(),
    ))
    .await
    .expect("send ice 21st");

    let resp = recv_server_msg(&mut ws).await;
    match resp {
        ServerToClient::Error { code, .. } => assert_eq!(code, ErrorCode::RateLimited),
        other => panic!("expected RateLimited error, got {:?}", other),
    }

    handle.shutdown().await;
}
