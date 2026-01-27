use bevy_ecs::prelude::*;
use godot::classes::{XrInterface, XrServer};
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
    #[error("bridge is not started")]
    NotStarted,
    #[error("direct state mutation is not allowed")]
    DirectStateMutationDenied,
    #[error("core error")]
    Core(#[source] CoreError),
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

trait XrInterfaceAccess {
    fn initialize(&mut self) -> bool;
    fn is_initialized(&self) -> bool;
    fn uninitialize(&mut self);
}

trait XrInterfaceProvider {
    type Interface: XrInterfaceAccess;
    fn find_openxr(&self) -> Option<Self::Interface>;
}

struct OpenXrRuntime<P: XrInterfaceProvider> {
    provider: P,
    interface: Option<P::Interface>,
    ready: bool,
}

impl<P: XrInterfaceProvider> OpenXrRuntime<P> {
    fn new(provider: P) -> Self {
        Self {
            provider,
            interface: None,
            ready: false,
        }
    }
}

impl<P: XrInterfaceProvider> XrRuntime for OpenXrRuntime<P> {
    fn enable(&mut self) -> Result<(), XrError> {
        let mut interface = self.provider.find_openxr().ok_or_else(|| {
            XrError::InitializationFailed {
                reason: "openxr interface not found; enable OpenXR in project settings"
                    .to_string(),
            }
        })?;

        if !interface.initialize() {
            self.ready = false;
            return Err(XrError::InitializationFailed {
                reason: "openxr initialization failed; ensure SteamVR is running".to_string(),
            });
        }

        self.ready = interface.is_initialized();
        if !self.ready {
            return Err(XrError::InitializationFailed {
                reason: "openxr runtime not ready; ensure SteamVR is running".to_string(),
            });
        }

        self.interface = Some(interface);
        Ok(())
    }

    fn is_ready(&self) -> bool {
        self.ready
    }

    fn shutdown(&mut self) -> Result<(), XrError> {
        if let Some(mut interface) = self.interface.take() {
            interface.uninitialize();
        }
        self.ready = false;
        Ok(())
    }
}

struct GodotXrProvider;

impl XrInterfaceProvider for GodotXrProvider {
    type Interface = Gd<XrInterface>;

    fn find_openxr(&self) -> Option<Self::Interface> {
        let server = XrServer::singleton();
        server.find_interface("OpenXR")
    }
}

impl XrInterfaceAccess for Gd<XrInterface> {
    fn initialize(&mut self) -> bool {
        XrInterface::initialize(&mut *self)
    }

    fn is_initialized(&self) -> bool {
        XrInterface::is_initialized(&*self)
    }

    fn uninitialize(&mut self) {
        XrInterface::uninitialize(&mut *self)
    }
}

pub struct GodotXrRuntime {
    inner: OpenXrRuntime<GodotXrProvider>,
}

impl GodotXrRuntime {
    pub fn new() -> Self {
        Self {
            inner: OpenXrRuntime::new(GodotXrProvider),
        }
    }
}

impl XrRuntime for GodotXrRuntime {
    fn enable(&mut self) -> Result<(), XrError> {
        self.inner.enable()
    }

    fn is_ready(&self) -> bool {
        self.inner.is_ready()
    }

    fn shutdown(&mut self) -> Result<(), XrError> {
        self.inner.shutdown()
    }
}

pub trait GodotBridge {
    fn on_start(&mut self) -> Result<(), BridgeError>;
    fn on_shutdown(&mut self) -> Result<(), BridgeError>;
    fn on_frame(&mut self, input: InputSnapshot) -> Result<RenderFrame, BridgeError>;
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

#[derive(Debug, Clone, thiserror::Error)]
pub enum CoreError {
    #[error("ecs world is not initialized")]
    NotInitialized,
    #[error("ecs world initialization failed: {reason}")]
    InitFailed { reason: String },
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

#[derive(Debug, Clone, PartialEq)]
pub struct StateOverrideRequest {
    pub reason: String,
}

pub struct GodotBridgeAdapter<C: EcsCore> {
    core: C,
    started: bool,
}

impl<C: EcsCore> GodotBridgeAdapter<C> {
    pub fn new(core: C) -> Self {
        Self {
            core,
            started: false,
        }
    }

    pub fn request_state_override(
        &mut self,
        _request: StateOverrideRequest,
    ) -> Result<(), BridgeError> {
        Err(BridgeError::DirectStateMutationDenied)
    }
}

impl<C: EcsCore> GodotBridge for GodotBridgeAdapter<C> {
    fn on_start(&mut self) -> Result<(), BridgeError> {
        self.core
            .init_world()
            .map_err(|err| BridgeError::InitializationFailed {
                reason: err.to_string(),
            })?;
        self.started = true;
        Ok(())
    }

    fn on_shutdown(&mut self) -> Result<(), BridgeError> {
        self.started = false;
        Ok(())
    }

    fn on_frame(&mut self, input: InputSnapshot) -> Result<RenderFrame, BridgeError> {
        if !self.started {
            return Err(BridgeError::NotStarted);
        }
        self.core.tick(input).map_err(BridgeError::Core)
    }
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

    #[derive(Clone)]
    struct FakeXrInterface {
        initialize_calls: usize,
        initialize_result: bool,
        initialized: bool,
        uninitialize_calls: usize,
    }

    impl Default for FakeXrInterface {
        fn default() -> Self {
            Self {
                initialize_calls: 0,
                initialize_result: true,
                initialized: true,
                uninitialize_calls: 0,
            }
        }
    }

    impl XrInterfaceAccess for FakeXrInterface {
        fn initialize(&mut self) -> bool {
            self.initialize_calls += 1;
            self.initialized = self.initialize_result;
            self.initialize_result
        }

        fn is_initialized(&self) -> bool {
            self.initialized
        }

        fn uninitialize(&mut self) {
            self.uninitialize_calls += 1;
            self.initialized = false;
        }
    }

    struct FakeXrProvider {
        interface: Option<FakeXrInterface>,
    }

    impl XrInterfaceProvider for FakeXrProvider {
        type Interface = FakeXrInterface;

        fn find_openxr(&self) -> Option<Self::Interface> {
            self.interface.clone()
        }
    }

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

        fn on_frame(&mut self, _input: InputSnapshot) -> Result<RenderFrame, BridgeError> {
            Err(BridgeError::NotStarted)
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

    struct FakeCore {
        init_calls: usize,
        init_result: Result<(), CoreError>,
        tick_calls: usize,
        tick_result: Result<RenderFrame, CoreError>,
        last_input: Option<InputSnapshot>,
    }

    impl FakeCore {
        fn with_tick_result(result: Result<RenderFrame, CoreError>) -> Self {
            Self {
                init_calls: 0,
                init_result: Ok(()),
                tick_calls: 0,
                tick_result: result,
                last_input: None,
            }
        }
    }

    impl Default for FakeCore {
        fn default() -> Self {
            Self::with_tick_result(Ok(RenderFrame {
                frame: FrameId(0),
                poses: Vec::new(),
            }))
        }
    }

    impl EcsCore for FakeCore {
        fn init_world(&mut self) -> Result<(), CoreError> {
            self.init_calls += 1;
            self.init_result.clone()
        }

        fn tick(&mut self, input: InputSnapshot) -> Result<RenderFrame, CoreError> {
            self.tick_calls += 1;
            self.last_input = Some(input);
            self.tick_result.clone()
        }
    }

    #[test]
    fn godot_bridge_initializes_on_start() {
        let core = FakeCore::default();
        let mut bridge = GodotBridgeAdapter::new(core);

        let result = bridge.on_start();

        assert!(result.is_ok());
        assert_eq!(bridge.core.init_calls, 1);
    }

    #[test]
    fn godot_bridge_reports_initialization_failure() {
        let mut core = FakeCore::default();
        core.init_result = Err(CoreError::InitFailed {
            reason: "init failed".to_string(),
        });
        let mut bridge = GodotBridgeAdapter::new(core);

        let result = bridge.on_start();

        assert!(matches!(
            result,
            Err(BridgeError::InitializationFailed { .. })
        ));
        assert_eq!(bridge.core.init_calls, 1);
    }

    #[test]
    fn godot_bridge_rejects_frame_before_start() {
        let core = FakeCore::default();
        let mut bridge = GodotBridgeAdapter::new(core);

        let result = bridge.on_frame(InputSnapshot {
            frame: FrameId(0),
            inputs: Vec::new(),
        });

        assert!(matches!(result, Err(BridgeError::NotStarted)));
    }

    #[test]
    fn godot_bridge_forwards_frame_input_to_core() {
        let core = FakeCore::with_tick_result(Ok(RenderFrame {
            frame: FrameId(1),
            poses: Vec::new(),
        }));
        let mut bridge = GodotBridgeAdapter::new(core);
        bridge.on_start().expect("start succeeds");

        let input = InputSnapshot {
            frame: FrameId(0),
            inputs: vec![InputEvent::Noop],
        };
        let result = bridge.on_frame(input.clone());

        assert!(result.is_ok());
        assert_eq!(bridge.core.tick_calls, 1);
        assert_eq!(bridge.core.last_input, Some(input));
    }

    #[test]
    fn godot_bridge_rejects_direct_state_override() {
        let core = FakeCore::default();
        let mut bridge = GodotBridgeAdapter::new(core);

        let result = bridge.request_state_override(StateOverrideRequest {
            reason: "direct change".to_string(),
        });

        assert!(matches!(
            result,
            Err(BridgeError::DirectStateMutationDenied)
        ));
    }

    #[test]
    fn openxr_enable_succeeds_when_ready() {
        let provider = FakeXrProvider {
            interface: Some(FakeXrInterface::default()),
        };
        let mut runtime = OpenXrRuntime::new(provider);

        let result = runtime.enable();

        assert!(result.is_ok());
        assert!(runtime.is_ready());
    }

    #[test]
    fn openxr_enable_reports_missing_interface() {
        let provider = FakeXrProvider { interface: None };
        let mut runtime = OpenXrRuntime::new(provider);

        let result = runtime.enable();

        assert!(matches!(result, Err(XrError::InitializationFailed { .. })));
        assert!(!runtime.is_ready());
    }

    #[test]
    fn openxr_enable_reports_not_ready() {
        let provider = FakeXrProvider {
            interface: Some(FakeXrInterface {
                initialize_result: false,
                initialized: false,
                ..FakeXrInterface::default()
            }),
        };
        let mut runtime = OpenXrRuntime::new(provider);

        let result = runtime.enable();

        assert!(matches!(result, Err(XrError::InitializationFailed { .. })));
        assert!(!runtime.is_ready());
    }
}
