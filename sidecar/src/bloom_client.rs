use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::time::{Duration, Instant};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

pub type BloomWs = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub async fn join_via_bloom_session(
    bloom_ws_url: &str,
    room_id: Option<String>,
) -> Result<(String, String, Vec<String>, BloomWs), String> {
    let (mut ws, _resp) = connect_async(bloom_ws_url)
        .await
        .map_err(|e| format!("connect bloom ws failed: {e:?}"))?;

    if let Some(room_id) = room_id {
        let join_payload = format!(r#"{{"type":"JoinRoom","room_id":"{room_id}"}}"#);
        ws.send(WsMessage::Text(join_payload))
            .await
            .map_err(|e| format!("send JoinRoom failed: {e:?}"))?;

        let deadline = Instant::now() + Duration::from_millis(500);
        let mut self_id: Option<String> = None;
        let mut participants: Option<Vec<String>> = None;

        while Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let msg = tokio::time::timeout(remaining, ws.next())
                .await
                .map_err(|_| "timeout waiting for bloom response".to_string())?;
            let Some(Ok(WsMessage::Text(t))) = msg else {
                continue;
            };
            let value: serde_json::Value =
                serde_json::from_str(&t).map_err(|e| format!("parse bloom msg: {e:?}"))?;
            match value.get("type").and_then(|v| v.as_str()) {
                Some("PeerConnected") => {
                    if let Some(pid) = value.get("participant_id").and_then(|v| v.as_str()) {
                        self_id = Some(pid.to_string());
                    }
                }
                Some("RoomParticipants") => {
                    if let Some(ps) = value.get("participants").and_then(|v| v.as_array()) {
                        let list = ps
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect::<Vec<_>>();
                        participants = Some(list);
                    }
                }
                _ => {}
            }

            if let (Some(pid), Some(ps)) = (self_id.clone(), participants.clone()) {
                return Ok((room_id.clone(), pid, ps, ws));
            }
        }

        let ps = participants.unwrap_or_default();
        let pid = self_id.or_else(|| ps.last().cloned()).unwrap_or_default();
        Ok((room_id, pid, ps, ws))
    } else {
        ws.send(WsMessage::Text(r#"{"type":"CreateRoom"}"#.into()))
            .await
            .map_err(|e| format!("send CreateRoom failed: {e:?}"))?;

        let deadline = Instant::now() + Duration::from_millis(500);
        while Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let msg = tokio::time::timeout(remaining, ws.next())
                .await
                .map_err(|_| "timeout waiting for RoomCreated".to_string())?;
            let Some(Ok(WsMessage::Text(t))) = msg else {
                continue;
            };
            let value: serde_json::Value =
                serde_json::from_str(&t).map_err(|e| format!("parse bloom msg: {e:?}"))?;
            if value.get("type").and_then(|v| v.as_str()) == Some("RoomCreated") {
                let rid = value
                    .get("room_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing room_id".to_string())?
                    .to_string();
                let pid = value
                    .get("self_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing self_id".to_string())?
                    .to_string();
                return Ok((rid, pid.clone(), vec![pid], ws));
            }
        }
        Err("timeout waiting for RoomCreated".to_string())
    }
}
