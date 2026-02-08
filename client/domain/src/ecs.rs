use bevy_ecs::prelude::*;
#[cfg(test)]
use bevy_ecs::schedule::ExecutorKind;

use crate::errors::CoreError;

pub const DEFAULT_INPUT_DT_SECONDS: f32 = 1.0 / 60.0;
const MOVE_SPEED_MPS: f32 = 1.5;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameId(pub u64);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameClock {
    current: FrameId,
}

impl FrameClock {
    pub fn next_frame(&mut self) -> FrameId {
        self.current.0 += 1;
        self.current
    }

    pub fn current_frame(&self) -> FrameId {
        self.current
    }

    pub fn reset(&mut self, frame: FrameId) {
        self.current = frame;
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub fn zero() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnitQuat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl UnitQuat {
    pub fn identity() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    pub position: Vec3,
    pub orientation: UnitQuat,
}

impl Pose {
    pub fn identity() -> Self {
        Self {
            position: Vec3::zero(),
            orientation: UnitQuat::identity(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputEvent {
    Move { axis_x: f32, axis_y: f32 },
    Look { yaw_delta: f32, pitch_delta: f32 },
    Action { name: String, pressed: bool },
}

#[derive(Resource, Debug, Clone, PartialEq)]
pub struct InputSnapshot {
    pub frame: FrameId,
    pub dt_seconds: f32,
    pub inputs: Vec<InputEvent>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderFrame {
    pub frame: FrameId,
    primary_pose: Pose,
    remote_poses: Vec<RemoteRenderPose>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemoteRenderPose {
    pub participant_id: String,
    pub pose: Pose,
}

impl RenderFrame {
    pub fn from_primary_pose(frame: FrameId, pose: Pose) -> Self {
        Self {
            frame,
            primary_pose: pose,
            remote_poses: Vec::new(),
        }
    }

    pub fn primary_pose(&self) -> &Pose {
        &self.primary_pose
    }

    pub fn with_remote_poses(mut self, remote_poses: Vec<RemoteRenderPose>) -> Self {
        self.remote_poses = remote_poses;
        self
    }

    pub fn remote_poses(&self) -> &[RemoteRenderPose] {
        &self.remote_poses
    }
}

pub trait EcsCore {
    fn init_world(&mut self) -> Result<(), CoreError>;
    fn tick(&mut self, input: InputSnapshot) -> Result<RenderFrame, CoreError>;
}

pub struct CoreEcs {
    world: World,
    schedule: Schedule,
}

impl CoreEcs {
    pub fn new() -> Self {
        let mut schedule = Schedule::default();
        schedule.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
        schedule.add_systems((apply_input_snapshot, advance_frame).chain());

        let mut world = World::new();
        reset_world(&mut world);

        Self { world, schedule }
    }

    #[cfg(test)]
    pub(crate) fn current_state(&self) -> (FrameId, Pose) {
        let state = self.world.resource::<GameState>();
        (state.frame, state.primary_pose)
    }

    #[cfg(test)]
    pub(crate) fn set_primary_pose_for_test(&mut self, pose: Pose) {
        self.world.resource_mut::<GameState>().primary_pose = pose;
    }

    #[cfg(test)]
    pub(crate) fn executor_kind(&self) -> ExecutorKind {
        self.schedule.get_executor_kind()
    }
}

impl Default for CoreEcs {
    fn default() -> Self {
        Self::new()
    }
}

impl EcsCore for CoreEcs {
    fn init_world(&mut self) -> Result<(), CoreError> {
        reset_world(&mut self.world);
        Ok(())
    }

    fn tick(&mut self, input: InputSnapshot) -> Result<RenderFrame, CoreError> {
        self.world.insert_resource(input);
        self.schedule.run(&mut self.world);

        let state = self.world.resource::<GameState>();
        let frame = state.frame;
        let pose = state.primary_pose;

        self.world.remove_resource::<InputSnapshot>();

        Ok(RenderFrame::from_primary_pose(frame, pose))
    }
}

#[derive(Resource, Debug, Clone, Copy, PartialEq)]
struct GameState {
    frame: FrameId,
    primary_pose: Pose,
    yaw_radians: f32,
    pitch_radians: f32,
}

fn reset_world(world: &mut World) {
    world.insert_resource(GameState {
        frame: FrameId(0),
        primary_pose: Pose::identity(),
        yaw_radians: 0.0,
        pitch_radians: 0.0,
    });
}

fn apply_input_snapshot(input: Res<InputSnapshot>, mut state: ResMut<GameState>) {
    let dt_seconds = sanitize_dt(input.dt_seconds);
    let (axis_x, axis_y, yaw_rate, pitch_rate) = aggregate_input_intents(&input.inputs);
    let (axis_x, axis_y) = normalize_move_axis(axis_x, axis_y);

    state.yaw_radians += yaw_rate * dt_seconds;
    state.pitch_radians += pitch_rate * dt_seconds;

    let (sin_yaw, cos_yaw) = state.yaw_radians.sin_cos();
    let movement_scale = MOVE_SPEED_MPS * dt_seconds;
    let delta_x = (cos_yaw * axis_x + sin_yaw * axis_y) * movement_scale;
    let delta_z = (sin_yaw * axis_x - cos_yaw * axis_y) * movement_scale;
    state.primary_pose.position.x += delta_x;
    state.primary_pose.position.z += delta_z;
    state.primary_pose.orientation =
        quaternion_from_yaw_pitch(state.yaw_radians, state.pitch_radians);
}

fn advance_frame(mut state: ResMut<GameState>) {
    state.frame.0 += 1;
}

fn sanitize_dt(dt_seconds: f32) -> f32 {
    if dt_seconds.is_finite() && dt_seconds > 0.0 {
        dt_seconds
    } else {
        DEFAULT_INPUT_DT_SECONDS
    }
}

fn aggregate_input_intents(inputs: &[InputEvent]) -> (f32, f32, f32, f32) {
    inputs.iter().fold(
        (0.0_f32, 0.0_f32, 0.0_f32, 0.0_f32),
        |(axis_x, axis_y, yaw_rate, pitch_rate), input| match input {
            InputEvent::Move {
                axis_x: input_x,
                axis_y: input_y,
            } => (axis_x + input_x, axis_y + input_y, yaw_rate, pitch_rate),
            InputEvent::Look {
                yaw_delta,
                pitch_delta,
            } => (
                axis_x,
                axis_y,
                yaw_rate + yaw_delta,
                pitch_rate + pitch_delta,
            ),
            InputEvent::Action { .. } => (axis_x, axis_y, yaw_rate, pitch_rate),
        },
    )
}

fn normalize_move_axis(axis_x: f32, axis_y: f32) -> (f32, f32) {
    let mut axis_x = axis_x.clamp(-1.0, 1.0);
    let mut axis_y = axis_y.clamp(-1.0, 1.0);
    let magnitude = (axis_x * axis_x + axis_y * axis_y).sqrt();
    if magnitude > 1.0 {
        axis_x /= magnitude;
        axis_y /= magnitude;
    }
    (axis_x, axis_y)
}

fn quaternion_from_yaw_pitch(yaw_radians: f32, pitch_radians: f32) -> UnitQuat {
    let (sin_yaw, cos_yaw) = (yaw_radians * 0.5).sin_cos();
    let (sin_pitch, cos_pitch) = (pitch_radians * 0.5).sin_cos();
    UnitQuat {
        x: cos_yaw * sin_pitch,
        y: sin_yaw * cos_pitch,
        z: -sin_yaw * sin_pitch,
        w: cos_yaw * cos_pitch,
    }
}
