use bevy_ecs::prelude::*;
use godot::prelude::*;

#[derive(Debug, Clone, thiserror::Error)]
pub enum XrError {
    #[error("xr initialization failed: {reason}")]
    InitializationFailed { reason: String },
    #[error("xr shutdown failed: {reason}")]
    ShutdownFailed { reason: String },
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum BridgeError {
    #[error("bridge initialization failed: {reason}")]
    InitializationFailed { reason: String },
    #[error("bridge shutdown failed: {reason}")]
    ShutdownFailed { reason: String },
}

#[derive(Debug, thiserror::Error)]
pub enum StartError {
    #[error("xr runtime initialization failed")]
    XrInit(#[source] XrError),
    #[error("xr runtime is not ready")]
    XrNotReady,
    #[error("godot bridge initialization failed")]
    BridgeInit(#[source] BridgeError),
}

pub trait XrRuntime {
    fn enable(&mut self) -> Result<(), XrError>;
    fn is_ready(&self) -> bool;
    fn shutdown(&mut self) -> Result<(), XrError>;
}

pub trait GodotBridge {
    fn on_start(&mut self) -> Result<(), BridgeError>;
    fn on_shutdown(&mut self) -> Result<(), BridgeError>;
}

pub trait ClientLifecycle {
    fn start(&mut self) -> Result<(), StartError>;
    fn shutdown(&mut self) -> Result<(), ShutdownError>;
}

#[derive(Debug, thiserror::Error)]
pub enum ShutdownError {
    #[error("shutdown failed: {reason}")]
    Failed { reason: String },
    #[error("xr shutdown failed")]
    XrShutdown(#[source] XrError),
    #[error("bridge shutdown failed")]
    BridgeShutdown(#[source] BridgeError),
}

pub struct ClientBootstrap<X: XrRuntime, B: GodotBridge> {
    xr: X,
    bridge: B,
    running: bool,
    frame_id: FrameId,
}

impl<X: XrRuntime, B: GodotBridge> ClientBootstrap<X, B> {
    pub fn new(xr: X, bridge: B) -> Self {
        Self {
            xr,
            bridge,
            running: false,
            frame_id: FrameId(0),
        }
    }

    pub fn start(&mut self) -> Result<(), StartError> {
        self.xr.enable().map_err(StartError::XrInit)?;
        if !self.xr.is_ready() {
            return Err(StartError::XrNotReady);
        }
        self.bridge.on_start().map_err(StartError::BridgeInit)?;
        self.running = true;
        Ok(())
    }

    pub fn tick_frame(&mut self) -> Result<FrameId, FrameError> {
        if !self.running {
            return Err(FrameError::NotRunning);
        }
        self.frame_id.0 += 1;
        Ok(self.frame_id)
    }
}

impl<X: XrRuntime, B: GodotBridge> ClientLifecycle for ClientBootstrap<X, B> {
    fn start(&mut self) -> Result<(), StartError> {
        ClientBootstrap::start(self)
    }

    fn shutdown(&mut self) -> Result<(), ShutdownError> {
        if !self.running {
            return Ok(());
        }
        self.bridge
            .on_shutdown()
            .map_err(ShutdownError::BridgeShutdown)?;
        self.xr
            .shutdown()
            .map_err(ShutdownError::XrShutdown)?;
        self.running = false;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameId(u64);

#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error("client is not running")]
    NotRunning,
}

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("ecs world is not initialized")]
    NotInitialized,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnitQuat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    pub position: Vec3,
    pub orientation: UnitQuat,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputEvent {
    Noop,
}

#[derive(Resource, Debug, Clone, PartialEq)]
pub struct InputSnapshot {
    pub frame: FrameId,
    pub inputs: Vec<InputEvent>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderFrame {
    pub frame: FrameId,
    pub poses: Vec<Pose>,
}

pub trait EcsCore {
    fn init_world(&mut self) -> Result<(), CoreError>;
    fn tick(&mut self, input: InputSnapshot) -> Result<RenderFrame, CoreError>;
}

pub struct CoreEcs {
    world: Option<World>,
    schedule: Schedule,
}

impl CoreEcs {
    pub fn new() -> Self {
        let mut schedule = Schedule::default();
        schedule.add_systems(advance_frame);
        Self {
            world: None,
            schedule,
        }
    }
}

impl EcsCore for CoreEcs {
    fn init_world(&mut self) -> Result<(), CoreError> {
        let mut world = World::new();
        world.insert_resource(GameState { frame: FrameId(0) });
        self.world = Some(world);
        Ok(())
    }

    fn tick(&mut self, input: InputSnapshot) -> Result<RenderFrame, CoreError> {
        let world = self.world.as_mut().ok_or(CoreError::NotInitialized)?;
        world.insert_resource(input);
        self.schedule.run(world);
        let frame = world.resource::<GameState>().frame;
        world.remove_resource::<InputSnapshot>();
        Ok(RenderFrame {
            frame,
            poses: Vec::new(),
        })
    }
}

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
struct GameState {
    frame: FrameId,
}

fn advance_frame(mut state: ResMut<GameState>) {
    state.frame.0 += 1;
}

struct SuteraClientCore;

#[gdextension]
unsafe impl ExtensionLibrary for SuteraClientCore {}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeXr {
        enable_calls: usize,
        ready: bool,
        enable_result: Result<(), XrError>,
        shutdown_calls: usize,
        shutdown_result: Result<(), XrError>,
    }

    impl XrRuntime for FakeXr {
        fn enable(&mut self) -> Result<(), XrError> {
            self.enable_calls += 1;
            self.enable_result.clone()
        }

        fn is_ready(&self) -> bool {
            self.ready
        }

        fn shutdown(&mut self) -> Result<(), XrError> {
            self.shutdown_calls += 1;
            self.shutdown_result.clone()
        }
    }

    struct FakeBridge {
        start_calls: usize,
        start_result: Result<(), BridgeError>,
        shutdown_calls: usize,
        shutdown_result: Result<(), BridgeError>,
    }

    impl Default for FakeXr {
        fn default() -> Self {
            Self {
                enable_calls: 0,
                ready: false,
                enable_result: Ok(()),
                shutdown_calls: 0,
                shutdown_result: Ok(()),
            }
        }
    }

    impl Default for FakeBridge {
        fn default() -> Self {
            Self {
                start_calls: 0,
                start_result: Ok(()),
                shutdown_calls: 0,
                shutdown_result: Ok(()),
            }
        }
    }

    impl GodotBridge for FakeBridge {
        fn on_start(&mut self) -> Result<(), BridgeError> {
            self.start_calls += 1;
            self.start_result.clone()
        }

        fn on_shutdown(&mut self) -> Result<(), BridgeError> {
            self.shutdown_calls += 1;
            self.shutdown_result.clone()
        }
    }

    #[test]
    fn start_initializes_dependencies_on_request() {
        let xr = FakeXr {
            ready: true,
            enable_result: Ok(()),
            ..FakeXr::default()
        };
        let bridge = FakeBridge {
            start_result: Ok(()),
            ..FakeBridge::default()
        };
        let mut bootstrap = ClientBootstrap::new(xr, bridge);

        let result = bootstrap.start();

        assert!(result.is_ok());
        assert_eq!(bootstrap.xr.enable_calls, 1);
        assert_eq!(bootstrap.bridge.start_calls, 1);
    }

    #[test]
    fn start_reports_xr_initialization_failure() {
        let xr = FakeXr {
            ready: false,
            enable_result: Err(XrError::InitializationFailed {
                reason: "no runtime".to_string(),
            }),
            ..FakeXr::default()
        };
        let bridge = FakeBridge::default();
        let mut bootstrap = ClientBootstrap::new(xr, bridge);

        let result = bootstrap.start();

        assert!(matches!(result, Err(StartError::XrInit(_))));
        assert_eq!(bootstrap.bridge.start_calls, 0);
    }

    #[test]
    fn start_reports_xr_not_ready() {
        let xr = FakeXr {
            ready: false,
            enable_result: Ok(()),
            ..FakeXr::default()
        };
        let bridge = FakeBridge::default();
        let mut bootstrap = ClientBootstrap::new(xr, bridge);

        let result = bootstrap.start();

        assert!(matches!(result, Err(StartError::XrNotReady)));
        assert_eq!(bootstrap.bridge.start_calls, 0);
    }

    #[test]
    fn start_reports_bridge_initialization_failure() {
        let xr = FakeXr {
            ready: true,
            enable_result: Ok(()),
            ..FakeXr::default()
        };
        let bridge = FakeBridge {
            start_result: Err(BridgeError::InitializationFailed {
                reason: "gdext missing".to_string(),
            }),
            ..FakeBridge::default()
        };
        let mut bootstrap = ClientBootstrap::new(xr, bridge);

        let result = bootstrap.start();

        assert!(matches!(result, Err(StartError::BridgeInit(_))));
        assert_eq!(bootstrap.bridge.start_calls, 1);
    }

    #[test]
    fn tick_frame_continues_while_running() {
        let xr = FakeXr {
            ready: true,
            enable_result: Ok(()),
            ..FakeXr::default()
        };
        let bridge = FakeBridge {
            start_result: Ok(()),
            ..FakeBridge::default()
        };
        let mut bootstrap = ClientBootstrap::new(xr, bridge);

        bootstrap.start().expect("start succeeds");
        let first = bootstrap.tick_frame().expect("first frame");
        let second = bootstrap.tick_frame().expect("second frame");

        assert_eq!(first, FrameId(1));
        assert_eq!(second, FrameId(2));
    }

    #[test]
    fn tick_frame_fails_when_not_running() {
        let xr = FakeXr::default();
        let bridge = FakeBridge::default();
        let mut bootstrap = ClientBootstrap::new(xr, bridge);

        let result = bootstrap.tick_frame();

        assert!(matches!(result, Err(FrameError::NotRunning)));
    }

    #[test]
    fn shutdown_releases_resources_and_stops_running() {
        let xr = FakeXr {
            ready: true,
            enable_result: Ok(()),
            ..FakeXr::default()
        };
        let bridge = FakeBridge {
            start_result: Ok(()),
            ..FakeBridge::default()
        };
        let mut bootstrap = ClientBootstrap::new(xr, bridge);

        bootstrap.start().expect("start succeeds");
        bootstrap.shutdown().expect("shutdown succeeds");

        assert_eq!(bootstrap.bridge.shutdown_calls, 1);
        assert_eq!(bootstrap.xr.shutdown_calls, 1);
        assert!(matches!(
            bootstrap.tick_frame(),
            Err(FrameError::NotRunning)
        ));
    }

    #[test]
    fn shutdown_reports_bridge_failure() {
        let xr = FakeXr {
            ready: true,
            enable_result: Ok(()),
            ..FakeXr::default()
        };
        let bridge = FakeBridge {
            start_result: Ok(()),
            shutdown_result: Err(BridgeError::ShutdownFailed {
                reason: "bridge busy".to_string(),
            }),
            ..FakeBridge::default()
        };
        let mut bootstrap = ClientBootstrap::new(xr, bridge);

        bootstrap.start().expect("start succeeds");
        let result = bootstrap.shutdown();

        assert!(matches!(result, Err(ShutdownError::BridgeShutdown(_))));
        assert_eq!(bootstrap.bridge.shutdown_calls, 1);
        assert_eq!(bootstrap.xr.shutdown_calls, 0);
    }

    #[test]
    fn init_world_creates_game_state_resource() {
        let mut ecs = CoreEcs::new();

        ecs.init_world().expect("init succeeds");

        let world = ecs.world.as_ref().expect("world initialized");
        assert!(world.contains_resource::<GameState>());
        assert_eq!(world.resource::<GameState>().frame, FrameId(0));
    }

    #[test]
    fn tick_runs_systems_and_advances_frame() {
        let mut ecs = CoreEcs::new();
        ecs.init_world().expect("init succeeds");

        let first = ecs
            .tick(InputSnapshot {
                frame: FrameId(0),
                inputs: Vec::new(),
            })
            .expect("first tick");
        let second = ecs
            .tick(InputSnapshot {
                frame: FrameId(1),
                inputs: Vec::new(),
            })
            .expect("second tick");

        assert_eq!(first.frame, FrameId(1));
        assert_eq!(second.frame, FrameId(2));

        let world = ecs.world.as_ref().expect("world initialized");
        assert_eq!(world.resource::<GameState>().frame, FrameId(2));
    }

    #[test]
    fn tick_fails_when_world_not_initialized() {
        let mut ecs = CoreEcs::new();

        let result = ecs.tick(InputSnapshot {
            frame: FrameId(0),
            inputs: Vec::new(),
        });

        assert!(matches!(result, Err(CoreError::NotInitialized)));
    }
}
