#[path = "common.rs"]
mod common;
#[path = "logging_common.rs"]
mod logging_common;

use bloom_api::ServerToClient;
use bloom_core::{CreateRoomResult, ParticipantId, RoomId};
use bloom_ws::{MockCore, SharedCore};
use futures_util::SinkExt;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;

use common::*;
use logging_common::*;

fn shared_core_with_arc() -> (
    SharedCore<MockCore>,
    std::sync::Arc<std::sync::Mutex<MockCore>>,
) {
    let mock_core = MockCore::new(CreateRoomResult {
        room_id: RoomId::new(),
        self_id: ParticipantId::new(),
        participants: vec![],
    });
    let core_arc = std::sync::Arc::new(std::sync::Mutex::new(mock_core));
    let shared = SharedCore::from_arc(core_arc.clone());
    (shared, core_arc)
}

/// WSハンドシェイクがHTTP 101で確立され、participant_id付きのspanが出ることを検証する。
#[tokio::test]
async fn handshake_returns_switching_protocols_and_sets_participant_span() {
    let (layer, _guard) = setup_tracing();
    let core = SharedCore::new(MockCore::new(CreateRoomResult {
        room_id: RoomId::new(),
        self_id: ParticipantId::new(),
        participants: vec![],
    }));
    let (server_url, handle) = spawn_bloom_ws_server_with_core(core).await;

    let (_ws_stream, response) = connect_async(&server_url)
        .await
        .expect("connect to bloom ws server");

    assert_eq!(response.status(), 101);

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let has_participant = {
        let spans = layer.spans.lock().expect("collect spans");
        spans_have_field_value(&spans, "participant_id", "")
    };
    assert!(has_participant);

    handle.shutdown().await;
}

/// /ws 以外のパスに対するHTTP応答が404となり、Upgrade/Connectionヘッダを付与しないことを検証する。
#[tokio::test]
async fn non_ws_path_returns_404_without_upgrade_headers() {
    let core = SharedCore::new(MockCore::new(CreateRoomResult {
        room_id: RoomId::new(),
        self_id: ParticipantId::new(),
        participants: vec![],
    }));
    let (server_url, handle) = spawn_bloom_ws_server_with_core(core).await;

    // /ws ではなく /foo にHTTPリクエスト（素朴なTCPで十分）
    let url = server_url.replace("/ws", "/foo");
    let host = url.strip_prefix("ws://").expect("ws url").to_string();
    let mut parts = host.split('/');
    let authority = parts.next().unwrap();

    let mut stream = tokio::net::TcpStream::connect(authority)
        .await
        .expect("connect tcp");
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let req = format!(
        "GET /foo HTTP/1.1\r\n\
Host: {authority}\r\n\
Upgrade: websocket\r\n\
Connection: Upgrade\r\n\
Sec-WebSocket-Key: dummydummydummydummy==\r\n\
Sec-WebSocket-Version: 13\r\n\
\r\n"
    );
    stream
        .write_all(req.as_bytes())
        .await
        .expect("write request");
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.expect("read response");
    let resp_str = String::from_utf8_lossy(&buf);

    // ステータス行とヘッダを手動で解析
    let mut lines = resp_str.lines();
    let status_line = lines.next().unwrap_or("");
    assert!(
        status_line.contains("404"),
        "status line should contain 404, got {status_line}"
    );
    let mut upgrade_hdr = "";
    let mut connection_hdr = "";
    for line in lines {
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("upgrade:") {
            upgrade_hdr = line.split_once(':').map(|x| x.1).unwrap_or("").trim();
        } else if lower.starts_with("connection:") {
            connection_hdr = line.split_once(':').map(|x| x.1).unwrap_or("").trim();
        }
        if line.is_empty() {
            break;
        }
    }
    assert!(
        upgrade_hdr.is_empty(),
        "Upgrade header should be absent for non-ws path, got '{upgrade_hdr}'"
    );
    assert!(
        connection_hdr.is_empty(),
        "Connection header should be absent for non-ws path, got '{connection_hdr}'"
    );

    handle.shutdown().await;
}

/// /ws へのリクエストで Upgrade ヘッダが欠如している場合に 426 を返し、Upgrade/Connection ヘッダを要求することを検証する。
#[tokio::test]
async fn ws_path_without_upgrade_header_returns_426_with_upgrade_headers() {
    let core = SharedCore::new(MockCore::new(CreateRoomResult {
        room_id: RoomId::new(),
        self_id: ParticipantId::new(),
        participants: vec![],
    }));
    let (server_url, handle) = spawn_bloom_ws_server_with_core(core).await;

    // Upgradeヘッダを抜いた /ws へのHTTPリクエスト
    let host = server_url
        .strip_prefix("ws://")
        .expect("ws url")
        .to_string();
    let mut parts = host.split('/');
    let authority = parts.next().unwrap();

    let mut stream = tokio::net::TcpStream::connect(authority)
        .await
        .expect("connect tcp");
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let req = format!(
        "GET /ws HTTP/1.1\r\n\
Host: {authority}\r\n\
\r\n"
    );
    stream
        .write_all(req.as_bytes())
        .await
        .expect("write request");
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.expect("read response");
    let resp_str = String::from_utf8_lossy(&buf);

    // ステータス行とヘッダを手動で解析
    let mut lines = resp_str.lines();
    let status_line = lines.next().unwrap_or("");
    assert!(
        status_line.contains("426"),
        "status line should contain 426, got {status_line}"
    );
    let mut upgrade_hdr = "";
    let mut connection_hdr = "";
    for line in lines {
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("upgrade:") {
            upgrade_hdr = line.split_once(':').map(|x| x.1).unwrap_or("").trim();
        } else if lower.starts_with("connection:") {
            connection_hdr = line.split_once(':').map(|x| x.1).unwrap_or("").trim();
        }
        if line.is_empty() {
            break;
        }
    }
    assert!(
        upgrade_hdr.to_ascii_lowercase().contains("websocket"),
        "Upgrade header should include websocket, got '{upgrade_hdr}'"
    );
    assert!(
        connection_hdr.to_ascii_lowercase().contains("upgrade"),
        "Connection header should include Upgrade, got '{connection_hdr}'"
    );

    handle.shutdown().await;
}

/// CreateRoomを送信するとRoomCreatedが返り、coreが一度呼ばれることを検証。
#[tokio::test]
async fn create_room_returns_room_created_and_calls_core_once() {
    let (_layer, _guard) = setup_tracing();
    let (shared_core, core_arc) = shared_core_with_arc();
    let (server_url, handle) = spawn_bloom_ws_server_with_core(shared_core).await;

    let (mut ws_stream, _response) = connect_async(&server_url)
        .await
        .expect("connect to bloom ws server");

    ws_stream
        .send(Message::Text(r#"{"type":"CreateRoom"}"#.into()))
        .await
        .expect("send create room");

    let msg = recv_server_msg(&mut ws_stream).await;
    match msg {
        ServerToClient::RoomCreated { .. } => {}
        _ => panic!("expected RoomCreated, got {:?}", msg),
    }

    let core_calls = core_arc.lock().expect("lock core").create_room_calls.len();
    assert_eq!(core_calls, 1);

    handle.shutdown().await;
}

/// tracingにparticipant_idとroom_idが載ることを統合経路で確認する。
#[tokio::test]
async fn offer_span_includes_participant_and_room_over_ws() {
    let (layer, _guard) = setup_tracing();
    let core = SharedCore::new(MockCore::new(CreateRoomResult {
        room_id: RoomId::new(),
        self_id: ParticipantId::new(),
        participants: vec![],
    }));
    let (server_url, handle) = spawn_bloom_ws_server_with_core(core).await;

    let (mut ws, _) = connect_async(&server_url).await.expect("connect client");
    ws.send(Message::Text(r#"{"type":"CreateRoom"}"#.into()))
        .await
        .expect("send create room");
    let room_created = recv_server_msg(&mut ws).await;
    let (room_id, self_id) = match room_created {
        ServerToClient::RoomCreated { room_id, self_id } => (room_id, self_id),
        other => panic!("expected RoomCreated, got {:?}", other),
    };

    let offer_json = format!(
        r#"{{"type":"Offer","to":"{to}","sdp":"v=0 offer","room_id":"{room_id}"}}"#,
        to = self_id,
        room_id = room_id
    );
    ws.send(Message::Text(offer_json))
        .await
        .expect("send offer");

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let (has_participant, has_room) = {
        let spans = layer.spans.lock().expect("collect spans");
        (
            spans_have_field_value(&spans, "participant_id", &self_id),
            spans_have_field_value(&spans, "room_id", &room_id),
        )
    };
    assert!(has_participant);
    assert!(has_room);

    handle.shutdown().await;
}
