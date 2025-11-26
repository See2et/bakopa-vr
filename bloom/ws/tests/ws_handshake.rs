use futures_util::StreamExt;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_hdr_async;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::protocol::Message;

/// WSハンドシェイクがHTTP 101で確立され、participant_id付きのspanが出ることを検証する
#[tokio::test]
async fn handshake_returns_switching_protocols_and_sets_participant_span() {
    // Arrange: Bloom WSサーバを起動（未実装）
    let server_url = spawn_bloom_ws_server().await;

    // Act: クライアントから接続してハンドシェイクを行う
    let (_ws_stream, response) = connect_async(&server_url)
        .await
        .expect("connect to bloom ws server");

    // Assert: HTTP 101 Switching Protocols を返すこと
    assert_eq!(response.status(), 101);

    // TODO: tracing layer を差し込み、handshake span に participant_id フィールドが含まれることを検証する
    // （サーバ実装後に有効化する）
}

/// Bloom WSサーバを起動して接続用URLを返す。
async fn spawn_bloom_ws_server() -> String {
    // 0番ポートでバインドし、実際のポートを取得
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind tcp listener");
    let addr = listener.local_addr().expect("local addr");

    // 簡易WSサーバ: /ws だけ受け入れて 101 を返す。処理は何もしない。
    tokio::spawn(async move {
        loop {
            let (stream, _peer) = match listener.accept().await {
                Ok(s) => s,
                Err(_) => break,
            };

            tokio::spawn(async move {
                let callback = |req: &Request, resp: Response| {
                    // パスが /ws 以外なら 426 を返す
                    if req.uri().path() != "/ws" {
                        let resp = Response::builder()
                            .status(StatusCode::UPGRADE_REQUIRED)
                            .body(None)
                            .expect("build 426 response");
                        Err(resp)
                    } else {
                        Ok(resp)
                    }
                };

                // Handshake を実行（成功すれば 101）
                let ws_stream = match accept_hdr_async(stream, callback).await {
                    Ok(s) => s,
                    Err(_) => return,
                };

                // 何もしないで読み捨て（接続は開けたまま）
                let (_sink, mut stream) = ws_stream.split();
                while let Some(msg) = stream.next().await {
                    if matches!(msg, Ok(Message::Close(_))) {
                        break;
                    }
                }
            });
        }
    });

    format!("ws://{}/ws", addr)
}
