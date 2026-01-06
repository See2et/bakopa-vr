use crate::bloom_client::join_via_bloom;
use anyhow::{anyhow, Result};
use axum::{
    extract::ws::{Message, WebSocket},
    extract::{State, WebSocketUpgrade},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use futures_util::StreamExt;
use std::sync::Arc;

use crate::auth::{check_bearer_token, check_origin, AuthError};
use crate::test_support;

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
                    } else if msg_type == Some("SendPose") && joined {
                        let params = syncer::TransportSendParams::for_stream(syncer::StreamKind::Pose);
                        test_support::record_send_params(params);
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
}
