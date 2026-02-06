use bevy_ecs::prelude::*;

use crate::errors::CoreError;

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
    pub inputs: Vec<InputEvent>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderFrame {
    pub frame: FrameId,
    primary_pose: Pose,
}

impl RenderFrame {
    pub fn from_primary_pose(frame: FrameId, pose: Pose) -> Self {
        Self {
            frame,
            primary_pose: pose,
        }
    }

    pub fn primary_pose(&self) -> &Pose {
        &self.primary_pose
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
        schedule.add_systems(advance_frame);
        #[cfg(feature = "demo-motion")]
        schedule.add_systems(demo_motion);

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
}

fn reset_world(world: &mut World) {
    world.insert_resource(GameState {
        frame: FrameId(0),
        primary_pose: Pose::identity(),
    });
}

fn advance_frame(mut state: ResMut<GameState>) {
    state.frame.0 += 1;
}

#[cfg(feature = "demo-motion")]
fn demo_motion(mut state: ResMut<GameState>) {
    let step = (state.frame.0 % 180) as f32 / 180.0;
    state.primary_pose.position.x = step * 0.5;
}
