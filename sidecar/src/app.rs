use anyhow::{anyhow, Result};
use axum::{
    extract::ws::{Message, WebSocket},
    extract::{State, WebSocketUpgrade},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::time::{Duration, Instant};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message as WsMessage;

use crate::auth::{check_bearer_token, check_origin, AuthError};

#[derive(Clone)]
struct AppState {
    token: Arc<String>,
}

/// Core application handle for the Sidecar service.
pub struct App {
    state: AppState,
}

impl App {
    /// Construct a new application instance.
    /// Fails if the required SIDECAR_TOKEN env var is missing.
    pub async fn new() -> Result<Self> {
        let token = std::env::var("SIDECAR_TOKEN")
            .map_err(|_| anyhow!("SIDECAR_TOKEN is required to start sidecar"))?;
        Ok(Self {
            state: AppState {
                token: Arc::new(token),
            },
        })
    }

    /// Build the HTTP/WebSocket router.
    /// Currently only /sidecar for WebSocket upgrade is exposed.
    pub fn router(&self) -> Router {
        Router::new()
            .route("/sidecar", get(ws_upgrade))
            .with_state(self.state.clone())
    }
}

async fn ws_upgrade(
    State(state): State<AppState>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, StatusCode> {
    check_origin(&headers)
        .and_then(|_| check_bearer_token(&headers, &state.token))
        .map_err(AuthError::status_code)?;

    Ok(ws.on_upgrade(handle_ws))
}

async fn handle_ws(mut socket: WebSocket) {
    let mut joined = false;

    while let Some(Ok(msg)) = socket.next().await {
        match msg {
            Message::Text(text) => {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                    let msg_type = value.get("type").and_then(|v| v.as_str());
                    if msg_type == Some("SendPose") && !joined {
                        let _ = socket
                            .send(Message::Text(
                                r#"{"type":"Error","kind":"NotJoined","message":"Join is required before SendPose"}"#.into(),
                            ))
                            .await;
                    } else if msg_type == Some("Join") && !joined {
                        let room_id_opt = value
                            .get("room_id")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        let bloom_ws_url = value
                            .get("bloom_ws_url")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());

                        let result = if let Some(url) = bloom_ws_url {
                            join_via_bloom(&url, room_id_opt).await
                        } else {
                            // Fallback: local generation (should be replaced by Bloom WS in tests)
                            let rid =
                                room_id_opt.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                            let pid = uuid::Uuid::new_v4().to_string();
                            Ok((rid, pid.clone(), vec![pid]))
                        };

                        match result {
                            Ok((rid, pid, participants)) => {
                                joined = true;
                                let self_joined = serde_json::json!({
                                    "type": "SelfJoined",
                                    "room_id": rid,
                                    "participant_id": pid,
                                    "participants": participants,
                                });
                                let _ = socket.send(Message::Text(self_joined.to_string())).await;
                            }
                            Err(message) => {
                                let err = serde_json::json!({
                                    "type": "Error",
                                    "kind": "BloomError",
                                    "message": message,
                                });
                                let _ = socket.send(Message::Text(err.to_string())).await;
                            }
                        }
                    } else if msg_type == Some("Join") {
                        // Ignore duplicate Join for now.
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
}

async fn join_via_bloom(
    bloom_ws_url: &str,
    room_id: Option<String>,
) -> Result<(String, String, Vec<String>), String> {
    let (mut ws, _resp) = connect_async(bloom_ws_url)
        .await
        .map_err(|e| format!("connect bloom ws failed: {e:?}"))?;

    if let Some(room_id) = room_id {
        let join_payload = format!(r#"{{"type":"JoinRoom","room_id":"{room_id}"}}"#);
        ws.send(WsMessage::Text(join_payload.into()))
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
                return Ok((room_id.clone(), pid, ps));
            }
        }

        let ps = participants.unwrap_or_default();
        let pid = self_id.or_else(|| ps.last().cloned()).unwrap_or_default();
        Ok((room_id, pid, ps))
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
                return Ok((rid, pid.clone(), vec![pid]));
            }
        }
        Err("timeout waiting for RoomCreated".to_string())
    }
}
