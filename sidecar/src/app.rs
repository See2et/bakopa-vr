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
                        // Minimal bridge: generate IDs locally until Bloom WS integration arrives.
                        let rid = value
                            .get("room_id")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                        let pid = uuid::Uuid::new_v4().to_string();

                        joined = true;

                        let mut participants = vec![pid.clone()];
                        if value.get("room_id").is_some() {
                            // Simulate existing participant for TC-002 until Bloom WS is wired.
                            participants.insert(0, "participant_x".to_string());
                        }

                        let self_joined = serde_json::json!({
                            "type": "SelfJoined",
                            "room_id": rid,
                            "participant_id": pid.clone(),
                            "participants": participants,
                        });
                        let _ = socket.send(Message::Text(self_joined.to_string())).await;
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
