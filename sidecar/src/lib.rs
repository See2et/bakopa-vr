use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Router, serve};
use subtle::ConstantTimeEq;
use tokio::net::TcpListener;
use tokio::sync::{oneshot, Mutex, mpsc::{UnboundedSender, unbounded_channel}};
use tokio::task::JoinHandle;
use std::sync::Arc;
use tracing::instrument;
use uuid::Uuid;
use futures_util::{StreamExt, SinkExt};

use anyhow::{Context, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ErrorKind {
    NotJoined,
    InvalidPayload,
}

#[derive(Clone)]
struct AppState {
    expected_token: String,
    room_state: Arc<Mutex<RoomState>>,
}

#[derive(Default)]
struct RoomState {
    room_id: Option<String>,
    participants: Vec<String>,
    sent_poses: Vec<(String, serde_json::Value)>,
    connections: Vec<ConnectionEntry>,
}

#[derive(Clone)]
struct ConnectionEntry {
    conn_id: String,
    participant_id: Option<String>,
    tx: UnboundedSender<Message>,
}

pub struct TestServerHandle {
    addr: std::net::SocketAddr,
    shutdown_tx: Option<oneshot::Sender<()>>,
    join_handle: Option<JoinHandle<()>>,
    room_state: Arc<Mutex<RoomState>>,
}

impl TestServerHandle {
    pub fn local_addr(&self) -> std::net::SocketAddr {
        self.addr
    }

    pub async fn sent_poses(&self) -> Vec<(String, serde_json::Value)> {
        self.room_state.lock().await.sent_poses.clone()
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
        .with_state(state.clone());

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
        room_state: state.room_state.clone(),
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

async fn handle_ws(socket: WebSocket, room_state: Arc<Mutex<RoomState>>) {
    let conn_id = Uuid::new_v4().to_string();
    let (tx, mut rx) = unbounded_channel::<Message>();

    {
        let mut state = room_state.lock().await;
        state.connections.push(ConnectionEntry {
            conn_id: conn_id.clone(),
            participant_id: None,
            tx: tx.clone(),
        });
    }

    let (mut ws_tx, mut ws_rx) = socket.split();
    let writer = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_tx.send(msg).await.is_err() {
                break;
            }
        }
    });

    let mut joined = false;
    let mut my_participant_id: Option<String> = None;

    while let Some(Ok(msg)) = ws_rx.next().await {
        match msg {
            Message::Ping(data) => {
                let _ = tx.send(Message::Pong(data));
            }
            Message::Close(frame) => {
                let _ = tx.send(Message::Close(frame));
                break;
            }
            Message::Text(body) => {
                let v = match serde_json::from_str::<serde_json::Value>(&body) {
                    Ok(v) => v,
                    Err(_) => {
                        send_error(&tx, ErrorKind::InvalidPayload);
                        continue;
                    }
                };

                let msg_type = v.get("type").and_then(|t| t.as_str()).unwrap_or("?");
                match msg_type {
                    "Join" => {
                        let requested_room = v
                            .get("room_id")
                            .and_then(|r| r.as_str())
                            .map(|s| s.to_string());

                        let mut state = room_state.lock().await;
                        let room_id = if let Some(existing) = &state.room_id {
                            existing.clone()
                        } else {
                            let new_id = requested_room
                                .clone()
                                .unwrap_or_else(|| Uuid::new_v4().to_string());
                            state.room_id = Some(new_id.clone());
                            new_id
                        };

                        if let Some(req) = requested_room.as_ref().filter(|r| *r != &room_id) {
                            tracing::debug!(requested = %req, current = %room_id, "room_id mismatch ignored in minimal impl");
                        }

                        let participant_id = Uuid::new_v4().to_string();
                        if !state.participants.contains(&participant_id) {
                            state.participants.push(participant_id.clone());
                        }
                        if let Some(conn) = state.connections.iter_mut().find(|c| c.conn_id == conn_id) {
                            conn.participant_id = Some(participant_id.clone());
                        }
                        joined = true;
                        my_participant_id = Some(participant_id.clone());
                        let list = state.participants.clone();
                        let _ = tx.send(Message::Text(
                            serde_json::json!({
                                "type": "SelfJoined",
                                "room_id": room_id,
                                "participant_id": participant_id,
                                "participants": list
                            })
                            .to_string(),
                        ));
                    }
                    "SendPose" if !joined => {
                        send_error(&tx, ErrorKind::NotJoined);
                    }
                    "SendPose" => {
                        if !is_pose_valid(&v) {
                            send_error(&tx, ErrorKind::InvalidPayload);
                            continue;
                        }
                        if let Some(pid) = my_participant_id.clone() {
                            let mut state = room_state.lock().await;
                            state.sent_poses.push((pid.clone(), v.clone()));
                            // broadcast to other connections
                            let pose_received = Message::Text(
                                serde_json::json!({
                                    "type": "PoseReceived",
                                    "from": pid,
                                    "pose": v
                                })
                                .to_string(),
                            );
                            for conn in state.connections.iter() {
                                if conn.conn_id != conn_id {
                                    let _ = conn.tx.send(pose_received.clone());
                                }
                            }
                        }
                    }
                    _ => {
                        send_error(&tx, ErrorKind::InvalidPayload);
                    }
                }
            }
            _ => {
                // ignore
            }
        }
    }

    {
        let mut state = room_state.lock().await;
        state.connections.retain(|c| c.conn_id != conn_id);
    }
    writer.abort();
}

fn send_error(tx: &UnboundedSender<Message>, kind: ErrorKind) {
    let kind_str = match kind {
        ErrorKind::NotJoined => "NotJoined",
        ErrorKind::InvalidPayload => "InvalidPayload",
    };
    let _ = tx.send(Message::Text(
        serde_json::json!({
            "type": "Error",
            "kind": kind_str,
            "message": kind_str
        })
        .to_string(),
    ));
}

fn is_pose_valid(v: &serde_json::Value) -> bool {
    let check_vec = |vec: &serde_json::Value| {
        vec.as_object().is_some_and(|m| {
            ["x", "y", "z"]
                .iter()
                .all(|k| m.get(*k).and_then(|n| n.as_f64()).map(|f| f.is_finite()).unwrap_or(false))
        })
    };
    let check_rot = |rot: &serde_json::Value| {
        rot.as_object().is_some_and(|m| {
            ["x", "y", "z", "w"]
                .iter()
                .all(|k| m.get(*k).and_then(|n| n.as_f64()).map(|f| f.is_finite()).unwrap_or(false))
        })
    };

    let head = match v.get("head") {
        Some(h) => h,
        None => return false,
    };
    let pos_ok = head.get("position").is_some_and(check_vec);
    let rot_ok = head.get("rotation").is_some_and(check_rot);
    pos_ok && rot_ok
}
