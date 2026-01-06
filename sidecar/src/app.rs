use anyhow::{anyhow, Result};
use axum::{
    extract::{State, WebSocketUpgrade},
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use std::sync::Arc;

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
    let auth_header = headers.get(AUTHORIZATION).and_then(|v| v.to_str().ok());
    let Some(bearer) = auth_header else {
        return Err(StatusCode::UNAUTHORIZED);
    };

    let Some(provided) = bearer.strip_prefix("Bearer ") else {
        return Err(StatusCode::UNAUTHORIZED);
    };

    if subtle_equals(provided.as_bytes(), state.token.as_bytes()) {
        Ok(ws.on_upgrade(|socket| async move {
            // Placeholder: drop immediately until real session handling is added.
            drop(socket);
        }))
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

/// Constant-time comparison to avoid timing leaks.
fn subtle_equals(a: &[u8], b: &[u8]) -> bool {
    use subtle::ConstantTimeEq;
    a.ct_eq(b).into()
}
