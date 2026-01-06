use anyhow::{anyhow, Result};
use axum::{
    extract::{State, WebSocketUpgrade},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
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

    Ok(ws.on_upgrade(|socket| async move {
        // Placeholder: drop immediately until real session handling is added.
        drop(socket);
    }))
}
