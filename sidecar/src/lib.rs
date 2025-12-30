use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Router, serve};
use subtle::ConstantTimeEq;
use tokio::net::TcpListener;
use tokio::sync::{Mutex, oneshot};
use tokio::task::JoinHandle;
use std::sync::Arc;
use tracing::instrument;
use uuid::Uuid;

use anyhow::{Context, Result};

#[derive(Clone)]
struct AppState {
    expected_token: String,
    room_state: Arc<Mutex<RoomState>>,
}

#[derive(Default)]
struct RoomState {
    room_id: Option<String>,
    participants: Vec<String>,
}

pub struct TestServerHandle {
    addr: std::net::SocketAddr,
    shutdown_tx: Option<oneshot::Sender<()>>,
    join_handle: Option<JoinHandle<()>>,
}

impl TestServerHandle {
    pub fn local_addr(&self) -> std::net::SocketAddr {
        self.addr
    }

    pub async fn shutdown(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.await;
        }
    }
}

impl Drop for TestServerHandle {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.join_handle.take() {
            handle.abort();
        }
    }
}

pub async fn run_for_tests(listen_addr: &str) -> Result<TestServerHandle> {
    let expected_token = std::env::var("SIDECAR_TOKEN").context("SIDECAR_TOKEN not set")?;

    let state = AppState {
        expected_token,
        room_state: Arc::new(Mutex::new(RoomState::default())),
    };

    let listener = TcpListener::bind(listen_addr)
        .await
        .context("failed to bind listen addr")?;
    let addr = listener.local_addr().context("local_addr failed")?;

    let router = Router::new()
        .route("/sidecar", get(ws_handler))
        .with_state(state);

    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let join_handle = tokio::spawn(async move {
        let server = serve(listener, router);
        let _ = server
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await;
    });

    Ok(TestServerHandle {
        addr,
        shutdown_tx: Some(shutdown_tx),
        join_handle: Some(join_handle),
    })
}

#[instrument(skip(state, upgrade, headers))]
async fn ws_handler(
    State(state): State<AppState>,
    upgrade: WebSocketUpgrade,
    headers: HeaderMap,
) -> Response {
    // Origin チェック: 非null/非空は拒否
    if let Some(origin) = headers
        .get(header::ORIGIN)
        .filter(|o| !o.as_bytes().is_empty())
    {
        tracing::debug!(?origin, "rejecting request due to Origin");
        return (StatusCode::FORBIDDEN, "forbidden origin").into_response();
    }

    // Authorization チェック
    let auth_header = match headers.get(header::AUTHORIZATION) {
        Some(v) => v,
        None => return (StatusCode::UNAUTHORIZED, "missing authorization").into_response(),
    };

    let auth_str = match auth_header.to_str() {
        Ok(s) => s,
        Err(_) => return (StatusCode::UNAUTHORIZED, "invalid authorization").into_response(),
    };

    const BEARER_PREFIX: &str = "Bearer ";
    if !auth_str.starts_with(BEARER_PREFIX) {
        return (StatusCode::UNAUTHORIZED, "invalid scheme").into_response();
    }

    let token_provided = &auth_str[BEARER_PREFIX.len()..];
    if !constant_time_eq(token_provided.as_bytes(), state.expected_token.as_bytes()) {
        return (StatusCode::UNAUTHORIZED, "invalid token").into_response();
    }

    let room_state = state.room_state.clone();
    upgrade
        .on_upgrade(move |socket| handle_ws(socket, room_state))
        .into_response()
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.ct_eq(b).into()
}

async fn handle_ws(mut socket: WebSocket, room_state: Arc<Mutex<RoomState>>) {
    let mut joined = false;
    let mut _my_room_id: Option<String> = None;
    let mut _my_participant_id: Option<String> = None;
    while let Some(Ok(msg)) = socket.recv().await {
        match msg {
            Message::Ping(data) => {
                if socket.send(Message::Pong(data)).await.is_err() {
                    break;
                }
            }
            Message::Close(frame) => {
                let _ = socket.send(Message::Close(frame)).await;
                break;
            }
            Message::Text(body) => {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                    let msg_type = v
                        .get("type")
                        .and_then(|t| t.as_str())
                        .unwrap_or("?");
                    match msg_type {
                        "Join" => {
                            let requested_room = v
                                .get("room_id")
                                .and_then(|r| r.as_str())
                                .map(|s| s.to_string());

                            let mut room_state = room_state.lock().await;
                            let room_id = if let Some(existing) = &room_state.room_id {
                                existing.clone()
                            } else {
                                let new_id = requested_room
                                    .clone()
                                    .unwrap_or_else(|| Uuid::new_v4().to_string());
                                room_state.room_id = Some(new_id.clone());
                                new_id
                            };

                            if let Some(req) = requested_room.as_ref().filter(|r| *r != &room_id) {
                                tracing::debug!(requested = %req, current = %room_id, "room_id mismatch ignored in minimal impl");
                            }

                            let participant_id = Uuid::new_v4().to_string();
                            if !room_state.participants.contains(&participant_id) {
                                room_state.participants.push(participant_id.clone());
                            }
                            joined = true;
                            _my_room_id = Some(room_id.clone());
                            _my_participant_id = Some(participant_id.clone());
                            let list = room_state.participants.clone();
                            let _ = socket
                                .send(Message::Text(
                                    serde_json::json!({
                                        "type": "SelfJoined",
                                        "room_id": room_id,
                                        "participant_id": participant_id,
                                        "participants": list
                                    })
                                    .to_string(),
                                ))
                                .await;
                        }
                        "SendPose" if !joined => {
                            let _ = socket
                                .send(Message::Text(
                                    serde_json::json!({
                                        "type": "Error",
                                        "kind": "NotJoined",
                                        "message": "join required"
                                    })
                                    .to_string(),
                                ))
                                .await;
                        }
                        _ => {}
                    }
                }
            }
            _ => {
                // 受信したメッセージは破棄
            }
        }
    }
}
