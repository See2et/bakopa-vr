use crate::bloom_client::{join_via_bloom_session, BloomWs};
use anyhow::Result;
use axum::extract::ws::rejection::WebSocketUpgradeRejection;
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use bloom_core::{ParticipantId, RoomId};
use futures_util::{SinkExt, StreamExt};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use tokio::time::{interval, Duration};
use tracing::info_span;

use crate::auth::{check_bearer_token, check_origin, AuthError};
use crate::config::AppConfig;
use crate::test_support;
use syncer::messages::SyncMessageEnvelope;
use syncer::{
    BasicSyncer, Pose, PoseTransform, StreamKind, Syncer, SyncerEvent, SyncerRequest,
    TracingContext, Transport, TransportPayload,
};
use tokio_tungstenite::tungstenite::Message as WsMessage;

#[derive(Default)]
struct SyncerBus {
    messages: Vec<(ParticipantId, ParticipantId, TransportPayload)>, // (to, from, payload)
    participants: HashSet<ParticipantId>,
}

struct BusTransport {
    me: ParticipantId,
    bus: Arc<Mutex<SyncerBus>>,
    registered: bool,
}

impl BusTransport {
    fn new(me: ParticipantId, bus: Arc<Mutex<SyncerBus>>) -> Self {
        Self {
            me,
            bus,
            registered: false,
        }
    }
}

impl Transport for BusTransport {
    fn register_participant(&mut self, participant: ParticipantId) {
        if participant == self.me {
            self.registered = true;
            if let Ok(mut bus) = self.bus.lock() {
                bus.participants.insert(participant);
            }
        }
    }

    fn send(
        &mut self,
        to: ParticipantId,
        payload: TransportPayload,
        _params: syncer::TransportSendParams,
    ) {
        if !self.registered {
            return;
        }
        let is_control = match &payload {
            TransportPayload::Bytes(bytes) => SyncMessageEnvelope::from_slice(bytes)
                .map(|env| matches!(env.kind, StreamKind::ControlJoin | StreamKind::ControlLeave))
                .unwrap_or(false),
            _ => false,
        };
        if let Ok(mut bus) = self.bus.lock() {
            if is_control {
                let recipients: Vec<ParticipantId> = bus
                    .participants
                    .iter()
                    .filter(|&p| p != &self.me)
                    .cloned()
                    .collect();
                for recipient in recipients {
                    bus.messages
                        .push((recipient, self.me.clone(), payload.clone()));
                }
            } else {
                bus.messages.push((to, self.me.clone(), payload));
            }
        }
    }

    fn poll(&mut self) -> Vec<syncer::TransportEvent> {
        if !self.registered {
            return Vec::new();
        }
        let mut out = Vec::new();
        if let Ok(mut bus) = self.bus.lock() {
            let mut i = 0;
            while i < bus.messages.len() {
                if bus.messages[i].0 == self.me {
                    let (_to, from, payload) = bus.messages.remove(i);
                    out.push(syncer::TransportEvent::Received { from, payload });
                } else {
                    i += 1;
                }
            }
        }
        out
    }
}

#[derive(Default)]
struct SyncerHub {
    rooms: HashMap<RoomId, Arc<Mutex<SyncerBus>>>,
}

impl SyncerHub {
    fn bus_for_room(&mut self, room_id: &RoomId) -> Arc<Mutex<SyncerBus>> {
        self.rooms
            .entry(room_id.clone())
            .or_insert_with(|| Arc::new(Mutex::new(SyncerBus::default())))
            .clone()
    }
}

#[derive(Clone)]
struct AppState {
    token: Arc<String>,
    syncer_hub: Arc<Mutex<SyncerHub>>,
}

/// Core application handle for the Sidecar service.
pub struct App {
    state: AppState,
    config: AppConfig,
}

impl App {
    /// Construct a new application instance.
    /// Fails if the required SIDECAR_TOKEN env var is missing.
    pub async fn new() -> Result<Self> {
        let config = AppConfig::from_env()?;
        Ok(Self {
            state: AppState {
                token: Arc::new(config.token.clone()),
                syncer_hub: Arc::new(Mutex::new(SyncerHub::default())),
            },
            config,
        })
    }

    pub fn bind_addr(&self) -> std::net::SocketAddr {
        self.config.bind_addr
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
    ws: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> Result<impl IntoResponse, StatusCode> {
    let ws: WebSocketUpgrade = match ws {
        Ok(ws) => ws,
        Err(_) => return Err(StatusCode::UPGRADE_REQUIRED),
    };

    check_origin(&headers)
        .and_then(|_| check_bearer_token(&headers, &state.token))
        .map_err(AuthError::status_code)?;

    Ok(ws.on_upgrade(move |socket| handle_ws(socket, state.clone())))
}

async fn handle_ws(mut socket: WebSocket, state: AppState) {
    let mut joined = false;
    let mut syncer: Option<BasicSyncer<BusTransport>> = None;
    let mut room_id: Option<RoomId> = None;
    let mut participant_id: Option<ParticipantId> = None;
    let mut bloom_ws: Option<BloomWs> = None;
    let mut poll_tick = interval(Duration::from_millis(10));

    loop {
        tokio::select! {
            incoming = socket.next() => {
                let Some(Ok(msg)) = incoming else { break; };
                match msg {
                    Message::Text(text) => {
                        let value = match serde_json::from_str::<serde_json::Value>(&text) {
                            Ok(v) => v,
                            Err(_) => {
                                let _ = socket
                                    .send(Message::Text(invalid_payload_message(
                                        "invalid JSON payload",
                                    )))
                                    .await;
                                continue;
                            }
                        };
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
                            join_via_bloom_session(&url, room_id_opt)
                                .await
                                .map(|(rid, pid, ps, ws)| (rid, pid, ps, Some(ws)))
                        } else {
                            Err("missing bloom_ws_url".to_string())
                        };

                        match result {
                            Ok((rid, pid, participants, ws)) => {
                                joined = true;
                                bloom_ws = ws;
                                if let (Ok(rid_parsed), Ok(pid_parsed)) = (
                                            RoomId::from_str(&rid),
                                            ParticipantId::from_str(&pid),
                                        ) {
                                            if let Ok(mut hub) = state.syncer_hub.lock() {
                                                let bus = hub.bus_for_room(&rid_parsed);
                                                let transport = BusTransport::new(pid_parsed.clone(), bus);
                                                let mut s = BasicSyncer::new(pid_parsed.clone(), transport);
                                                let _ = s.handle(SyncerRequest::Join {
                                                    room_id: rid_parsed.clone(),
                                                    participant_id: pid_parsed.clone(),
                                                });
                                                syncer = Some(s);
                                                room_id = Some(rid_parsed);
                                                participant_id = Some(pid_parsed);
                                            }
                                        }
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
                                            "kind": "SignalingError",
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

                                if let (Some(syncer), Some(room_id), Some(participant_id)) =
                                    (syncer.as_mut(), room_id.as_ref(), participant_id.as_ref())
                                {
                                    let span = info_span!(
                                        "sidecar.send_pose",
                                        room_id = %room_id,
                                        participant_id = %participant_id,
                                        stream_kind = "pose"
                                    );
                                    let _enter = span.enter();
                                    let Some(pose) = parse_pose_message(&value) else {
                                        let _ = socket
                                            .send(Message::Text(invalid_payload_message(
                                                "invalid SendPose payload",
                                            )))
                                            .await;
                                        continue;
                                    };
                                    let ctx = TracingContext {
                                        room_id: room_id.clone(),
                                        participant_id: participant_id.clone(),
                                        stream_kind: StreamKind::Pose,
                                    };
                                    let events = syncer.handle(SyncerRequest::SendPose {
                                        from: participant_id.clone(),
                                        pose,
                                        ctx,
                                    });
                                    for event in events {
                                        match event {
                                            SyncerEvent::PoseReceived { from, pose, .. } => {
                                                if let Some(payload) = pose_received_payload(&from, &pose) {
                                                    let _ = socket.send(Message::Text(payload)).await;
                                                }
                                            }
                                            SyncerEvent::RateLimited { stream_kind } => {
                                                let _ = socket
                                                    .send(Message::Text(rate_limited_payload(
                                                        stream_kind,
                                                    )))
                                                    .await;
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                        }
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
            _ = poll_tick.tick() => {
                if let Some(syncer) = syncer.as_mut() {
                    for event in syncer.poll_only() {
                        match event {
                            SyncerEvent::PoseReceived { from, pose, .. } => {
                                if let Some(payload) = pose_received_payload(&from, &pose) {
                                    let _ = socket.send(Message::Text(payload)).await;
                                }
                            }
                            SyncerEvent::RateLimited { stream_kind } => {
                                let _ = socket
                                    .send(Message::Text(rate_limited_payload(stream_kind)))
                                    .await;
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    if joined {
        if let Some(mut ws) = bloom_ws {
            let _ = ws
                .send(WsMessage::Text(r#"{"type":"LeaveRoom"}"#.into()))
                .await;
        }
    }
}

fn parse_pose_message(value: &serde_json::Value) -> Option<Pose> {
    let head = value.get("head").and_then(parse_transform)?;
    let hand_l = value.get("hand_l").and_then(parse_transform_opt);
    let hand_r = value.get("hand_r").and_then(parse_transform_opt);
    Some(Pose {
        version: 1,
        timestamp_micros: 0,
        head,
        hand_l,
        hand_r,
    })
}

fn parse_transform_opt(value: &serde_json::Value) -> Option<PoseTransform> {
    if value.is_null() {
        return None;
    }
    parse_transform(value)
}

fn parse_transform(value: &serde_json::Value) -> Option<PoseTransform> {
    let pos = value.get("position")?;
    let rot = value.get("rotation")?;
    let position = [
        pos.get("x")?.as_f64()? as f32,
        pos.get("y")?.as_f64()? as f32,
        pos.get("z")?.as_f64()? as f32,
    ];
    let rotation = [
        rot.get("x")?.as_f64()? as f32,
        rot.get("y")?.as_f64()? as f32,
        rot.get("z")?.as_f64()? as f32,
        rot.get("w")?.as_f64()? as f32,
    ];
    if !position.iter().all(|v| v.is_finite()) || !rotation.iter().all(|v| v.is_finite()) {
        return None;
    }
    Some(PoseTransform { position, rotation })
}

fn pose_received_payload(from: &ParticipantId, pose: &Pose) -> Option<String> {
    let head = pose_transform_to_json(&pose.head);
    let hand_l = pose.hand_l.as_ref().map(pose_transform_to_json);
    let hand_r = pose.hand_r.as_ref().map(pose_transform_to_json);
    let payload = serde_json::json!({
        "type": "PoseReceived",
        "from": from.to_string(),
        "pose": {
            "head": head,
            "hand_l": hand_l,
            "hand_r": hand_r,
        }
    });
    Some(payload.to_string())
}

fn rate_limited_payload(stream_kind: StreamKind) -> String {
    serde_json::json!({
        "type": "RateLimited",
        "stream_kind": stream_kind.as_str(),
    })
    .to_string()
}

fn invalid_payload_message(message: &str) -> String {
    serde_json::json!({
        "type": "Error",
        "kind": "InvalidPayload",
        "message": message,
    })
    .to_string()
}

fn pose_transform_to_json(pose: &PoseTransform) -> serde_json::Value {
    serde_json::json!({
        "position": {
            "x": pose.position[0],
            "y": pose.position[1],
            "z": pose.position[2],
        },
        "rotation": {
            "x": pose.rotation[0],
            "y": pose.rotation[1],
            "z": pose.rotation[2],
            "w": pose.rotation[3],
        }
    })
}
