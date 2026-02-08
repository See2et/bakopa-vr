use std::collections::HashMap;

use crate::bridge::RuntimeMode;
use crate::ecs::{EcsCore, FrameId, InputSnapshot, Pose, RemoteRenderPose, RenderFrame};
use crate::errors::CoreError;
use tracing::{info, warn};

const UNKNOWN_ROOM_ID: &str = "unknown";
const MODE_UNKNOWN: &str = "unknown";
const STREAM_POSE: &str = "pose";
const STREAM_CONTROL: &str = "control";
const LOCAL_PARTICIPANT_ID: &str = "local";

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum SyncSessionError {
    #[error("sync transport unavailable: {reason}")]
    TransportUnavailable { reason: String },
    #[error("sync session not ready: {reason}")]
    NotReady { reason: String },
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum ScopeBoundaryError {
    #[error("unsupported stream kind for this slice: {stream_kind}")]
    UnsupportedStreamKind { stream_kind: String },
    #[error("production bloom signaling is out of slice scope")]
    ProductionBloomSignalingOutOfScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalingRoute {
    SliceTestPath,
    BloomProduction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScopeBoundaryPolicy {
    signaling_route: SignalingRoute,
}

impl Default for ScopeBoundaryPolicy {
    fn default() -> Self {
        Self {
            signaling_route: SignalingRoute::SliceTestPath,
        }
    }
}

impl ScopeBoundaryPolicy {
    pub fn with_signaling_route(signaling_route: SignalingRoute) -> Self {
        Self { signaling_route }
    }

    pub fn ensure_stream_kind(self, stream_kind: &str) -> Result<(), ScopeBoundaryError> {
        match stream_kind {
            STREAM_POSE | STREAM_CONTROL => Ok(()),
            _ => Err(ScopeBoundaryError::UnsupportedStreamKind {
                stream_kind: stream_kind.to_string(),
            }),
        }
    }

    pub fn ensure_signaling_route(self) -> Result<(), ScopeBoundaryError> {
        match self.signaling_route {
            SignalingRoute::SliceTestPath => Ok(()),
            SignalingRoute::BloomProduction => {
                Err(ScopeBoundaryError::ProductionBloomSignalingOutOfScope)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShutdownDrainReport {
    pub applied_control_events: usize,
    pub dropped_pose_events: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SyncDelta {
    PeerJoined {
        participant_id: ParticipantId,
        session_epoch: u64,
    },
    PeerLeft {
        participant_id: ParticipantId,
    },
    PeerInactive {
        participant_id: ParticipantId,
    },
    PoseReceived {
        participant_id: ParticipantId,
        pose: Pose,
        version: PoseVersion,
    },
}

pub trait SyncSessionPort {
    fn send_local_pose(
        &mut self,
        frame: FrameId,
        mode: RuntimeMode,
        pose: Pose,
    ) -> Result<(), SyncSessionError>;

    fn poll_events(&mut self) -> Vec<SyncDelta> {
        Vec::new()
    }

    fn begin_shutdown(&mut self) {}

    fn drain_pending_events(&mut self) -> Vec<SyncDelta> {
        Vec::new()
    }
}

pub fn runtime_mode_label(mode: RuntimeMode) -> &'static str {
    match mode {
        RuntimeMode::Desktop => "desktop",
        RuntimeMode::Vr => "vr",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ParticipantId(String);

impl ParticipantId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PoseVersion {
    pub session_epoch: u64,
    pub pose_seq: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RemotePoseState {
    pub pose: Pose,
    pub version: PoseVersion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteLiveness {
    Active,
    SuspectedDisconnected,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RemoteParticipantState {
    pub session_epoch: u64,
    pub pose_state: Option<RemotePoseState>,
    pub liveness: RemoteLiveness,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemotePoseUpdate {
    Applied,
    StaleDropped,
}

#[derive(Debug, Default, Clone)]
pub struct RemotePoseRepository {
    entries: HashMap<ParticipantId, RemoteParticipantState>,
}

impl RemotePoseRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn on_peer_joined(&mut self, participant_id: ParticipantId, session_epoch: u64) {
        self.entries.insert(
            participant_id,
            RemoteParticipantState {
                session_epoch,
                pose_state: None,
                liveness: RemoteLiveness::Active,
            },
        );
    }

    pub fn on_peer_left(&mut self, participant_id: &ParticipantId) -> bool {
        self.entries.remove(participant_id).is_some()
    }

    pub fn mark_inactive(&mut self, participant_id: &ParticipantId) -> bool {
        if let Some(state) = self.entries.get_mut(participant_id) {
            state.liveness = RemoteLiveness::SuspectedDisconnected;
            return true;
        }
        false
    }

    pub fn apply_if_newer(
        &mut self,
        participant_id: ParticipantId,
        pose: Pose,
        version: PoseVersion,
    ) -> RemotePoseUpdate {
        let state = self
            .entries
            .entry(participant_id)
            .or_insert(RemoteParticipantState {
                session_epoch: version.session_epoch,
                pose_state: None,
                liveness: RemoteLiveness::Active,
            });

        if version.session_epoch != state.session_epoch {
            return RemotePoseUpdate::StaleDropped;
        }

        match state.pose_state {
            Some(current) if version <= current.version => RemotePoseUpdate::StaleDropped,
            _ => {
                state.pose_state = Some(RemotePoseState { pose, version });
                state.liveness = RemoteLiveness::Active;
                RemotePoseUpdate::Applied
            }
        }
    }

    pub fn get(&self, participant_id: &ParticipantId) -> Option<&RemoteParticipantState> {
        self.entries.get(participant_id)
    }

    pub fn render_snapshot(&self) -> Vec<RemoteRenderPose> {
        self.entries
            .iter()
            .filter_map(|(participant_id, state)| {
                state.pose_state.map(|pose_state| RemoteRenderPose {
                    participant_id: participant_id.as_str().to_string(),
                    pose: pose_state.pose,
                })
            })
            .collect()
    }
}

pub struct PoseSyncCoordinator<C: EcsCore, S: SyncSessionPort> {
    core: C,
    sync_port: S,
    remotes: RemotePoseRepository,
    last_sync_error: Option<SyncSessionError>,
    scope_policy: ScopeBoundaryPolicy,
}

impl<C: EcsCore, S: SyncSessionPort> PoseSyncCoordinator<C, S> {
    pub fn new(core: C, sync_port: S) -> Self {
        Self::with_scope_policy(core, sync_port, ScopeBoundaryPolicy::default())
    }

    pub fn with_scope_policy(core: C, sync_port: S, scope_policy: ScopeBoundaryPolicy) -> Self {
        Self {
            core,
            sync_port,
            remotes: RemotePoseRepository::new(),
            last_sync_error: None,
            scope_policy,
        }
    }

    pub fn apply_frame(
        &mut self,
        mode: RuntimeMode,
        input: InputSnapshot,
    ) -> Result<RenderFrame, CoreError> {
        let deltas = self.sync_port.poll_events();
        self.apply_sync_deltas(deltas, true);
        let frame = self
            .core
            .tick(input)?
            .with_remote_poses(self.remotes.render_snapshot());
        self.last_sync_error = None;
        if let Err(error) = self.scope_policy.ensure_signaling_route() {
            warn!(
                stage = "scope_guard",
                room_id = UNKNOWN_ROOM_ID,
                participant_id = LOCAL_PARTICIPANT_ID,
                stream_kind = STREAM_POSE,
                mode = runtime_mode_label(mode),
                error = %error,
                "sync send skipped due to slice scope policy"
            );
            return Ok(frame);
        }
        if let Err(error) = self.scope_policy.ensure_stream_kind(STREAM_POSE) {
            warn!(
                stage = "scope_guard",
                room_id = UNKNOWN_ROOM_ID,
                participant_id = LOCAL_PARTICIPANT_ID,
                stream_kind = STREAM_POSE,
                mode = runtime_mode_label(mode),
                error = %error,
                "sync send skipped due to unsupported stream kind"
            );
            return Ok(frame);
        }
        info!(
            stage = "send",
            room_id = UNKNOWN_ROOM_ID,
            participant_id = LOCAL_PARTICIPANT_ID,
            stream_kind = STREAM_POSE,
            mode = runtime_mode_label(mode),
            frame_id = frame.frame.0,
            "sending local pose snapshot"
        );
        match self
            .sync_port
            .send_local_pose(frame.frame, mode, *frame.primary_pose())
        {
            Ok(()) => {}
            Err(SyncSessionError::NotReady { reason }) => {
                warn!(
                    stage = "send_skip_not_joined",
                    room_id = UNKNOWN_ROOM_ID,
                    participant_id = LOCAL_PARTICIPANT_ID,
                    stream_kind = STREAM_POSE,
                    mode = runtime_mode_label(mode),
                    reason = %reason,
                    "skipping local pose send while session is not ready"
                );
            }
            Err(error) => {
                warn!(
                    stage = "send",
                    room_id = UNKNOWN_ROOM_ID,
                    participant_id = LOCAL_PARTICIPANT_ID,
                    stream_kind = STREAM_POSE,
                    mode = runtime_mode_label(mode),
                    error = %error,
                    "failed to send local pose; continuing frame update"
                );
                self.last_sync_error = Some(error);
            }
        }
        Ok(frame)
    }

    pub fn shutdown_sync_session(&mut self) -> ShutdownDrainReport {
        self.sync_port.begin_shutdown();
        let deltas = self.sync_port.drain_pending_events();
        let dropped_pose_events = deltas
            .iter()
            .filter(|delta| matches!(delta, SyncDelta::PoseReceived { .. }))
            .count();
        let applied_control_events = deltas.len().saturating_sub(dropped_pose_events);
        self.apply_sync_deltas(deltas, false);
        ShutdownDrainReport {
            applied_control_events,
            dropped_pose_events,
        }
    }

    pub fn on_peer_joined(&mut self, participant_id: ParticipantId, session_epoch: u64) {
        if let Err(error) = self.scope_policy.ensure_stream_kind(STREAM_CONTROL) {
            warn!(
                stage = "scope_guard",
                room_id = UNKNOWN_ROOM_ID,
                participant_id = participant_id.as_str(),
                stream_kind = STREAM_CONTROL,
                mode = MODE_UNKNOWN,
                error = %error,
                "peer join ignored due to unsupported stream kind"
            );
            return;
        }
        info!(
            stage = "join",
            room_id = UNKNOWN_ROOM_ID,
            participant_id = participant_id.as_str(),
            stream_kind = STREAM_CONTROL,
            mode = MODE_UNKNOWN,
            session_epoch,
            "peer joined sync session"
        );
        self.remotes.on_peer_joined(participant_id, session_epoch);
    }

    pub fn on_peer_left(&mut self, participant_id: &ParticipantId) -> bool {
        if let Err(error) = self.scope_policy.ensure_stream_kind(STREAM_CONTROL) {
            warn!(
                stage = "scope_guard",
                room_id = UNKNOWN_ROOM_ID,
                participant_id = participant_id.as_str(),
                stream_kind = STREAM_CONTROL,
                mode = MODE_UNKNOWN,
                error = %error,
                "peer left ignored due to unsupported stream kind"
            );
            return false;
        }
        info!(
            stage = "leave",
            room_id = UNKNOWN_ROOM_ID,
            participant_id = participant_id.as_str(),
            stream_kind = STREAM_CONTROL,
            mode = MODE_UNKNOWN,
            "peer left sync session"
        );
        self.remotes.on_peer_left(participant_id)
    }

    pub fn mark_peer_inactive(&mut self, participant_id: &ParticipantId) -> bool {
        self.remotes.mark_inactive(participant_id)
    }

    pub fn apply_remote_pose(
        &mut self,
        participant_id: ParticipantId,
        pose: Pose,
        version: PoseVersion,
    ) -> RemotePoseUpdate {
        if let Err(error) = self.scope_policy.ensure_stream_kind(STREAM_POSE) {
            warn!(
                stage = "scope_guard",
                room_id = UNKNOWN_ROOM_ID,
                participant_id = participant_id.as_str(),
                stream_kind = STREAM_POSE,
                mode = MODE_UNKNOWN,
                error = %error,
                "remote pose ignored due to unsupported stream kind"
            );
            return RemotePoseUpdate::StaleDropped;
        }
        info!(
            stage = "receive",
            room_id = UNKNOWN_ROOM_ID,
            participant_id = participant_id.as_str(),
            stream_kind = STREAM_POSE,
            mode = MODE_UNKNOWN,
            session_epoch = version.session_epoch,
            pose_seq = version.pose_seq,
            "received remote pose snapshot"
        );
        self.remotes.apply_if_newer(participant_id, pose, version)
    }

    pub fn sync_port(&self) -> &S {
        &self.sync_port
    }

    pub fn remotes(&self) -> &RemotePoseRepository {
        &self.remotes
    }

    pub fn last_sync_error_ref(&self) -> Option<&SyncSessionError> {
        self.last_sync_error.as_ref()
    }

    fn apply_sync_deltas(&mut self, deltas: Vec<SyncDelta>, include_pose: bool) {
        let mut pose_deltas = Vec::new();
        for delta in deltas {
            match delta {
                SyncDelta::PeerJoined {
                    participant_id,
                    session_epoch,
                } => self.on_peer_joined(participant_id, session_epoch),
                SyncDelta::PeerLeft { participant_id } => {
                    self.on_peer_left(&participant_id);
                }
                SyncDelta::PeerInactive { participant_id } => {
                    self.mark_peer_inactive(&participant_id);
                }
                pose_delta @ SyncDelta::PoseReceived { .. } => pose_deltas.push(pose_delta),
            }
        }

        if !include_pose {
            return;
        }

        for delta in pose_deltas {
            if let SyncDelta::PoseReceived {
                participant_id,
                pose,
                version,
            } = delta
            {
                self.apply_remote_pose(participant_id, pose, version);
            }
        }
    }
}
