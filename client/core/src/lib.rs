use bevy_ecs::prelude::*;
use godot::classes::{Node3D, XrInterface, XrServer};
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
        let mut interface =
            self.provider
                .find_openxr()
                .ok_or_else(|| XrError::InitializationFailed {
                    reason: "openxr interface not found; enable OpenXR in project settings"
                        .to_string(),
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
        XrInterface::is_initialized(self)
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

impl Default for GodotXrRuntime {
    fn default() -> Self {
        Self::new()
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

    pub fn tick(&mut self, inputs: Vec<InputEvent>) -> Result<RenderFrame, FrameError> {
        if !self.running {
            return Err(FrameError::NotRunning);
        }
        self.frame_id.0 += 1;
        let input = InputSnapshot {
            frame: self.frame_id,
            inputs,
        };
        self.bridge.on_frame(input).map_err(FrameError::Bridge)
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
        self.xr.shutdown().map_err(ShutdownError::XrShutdown)?;
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
    #[error("frame update failed")]
    Bridge(#[source] BridgeError),
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

fn pose_to_transform3d(pose: &Pose) -> Transform3D {
    let origin = Vector3::new(pose.position.x, pose.position.y, pose.position.z);
    let quat = Quaternion::new(
        pose.orientation.x,
        pose.orientation.y,
        pose.orientation.z,
        pose.orientation.w,
    );
    let basis = Basis::from_quaternion(quat);
    Transform3D::new(basis, origin)
}

fn render_frame_first_transform(frame: &RenderFrame) -> Option<Transform3D> {
    frame.poses.first().map(pose_to_transform3d)
}

trait TransformTarget {
    fn set_transform(&mut self, transform: Transform3D);
}

impl TransformTarget for Gd<Node3D> {
    fn set_transform(&mut self, transform: Transform3D) {
        self.call("set_transform", &[transform.to_variant()]);
    }
}

fn project_render_frame_to_target(frame: &RenderFrame, target: &mut impl TransformTarget) -> bool {
    let transform = match render_frame_first_transform(frame) {
        Some(transform) => transform,
        None => return false,
    };
    target.set_transform(transform);
    true
}

#[derive(Debug, Default)]
pub struct RenderStateProjector;

impl RenderStateProjector {
    pub fn project(&mut self, frame: &RenderFrame, target: &mut OnEditor<Gd<Node3D>>) -> bool {
        if target.is_invalid() {
            return false;
        }
        let node = &mut **target;
        project_render_frame_to_target(frame, node)
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct BridgeErrorState {
    last_error: Option<String>,
}

impl BridgeErrorState {
    pub fn record(&mut self, error: &BridgeError) {
        self.last_error = Some(error.to_string());
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }
}

pub trait RenderSink {
    fn project(&mut self, frame: RenderFrame);
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct RenderFrameBuffer {
    last: Option<RenderFrame>,
}

impl RenderFrameBuffer {
    pub fn last(&self) -> Option<&RenderFrame> {
        self.last.as_ref()
    }
}

impl RenderSink for RenderFrameBuffer {
    fn project(&mut self, frame: RenderFrame) {
        self.last = Some(frame);
    }
}

pub struct BridgePipeline<B: GodotBridge, S: RenderSink> {
    api: GodotBridgeApi<B>,
    sink: S,
}

impl<B: GodotBridge, S: RenderSink> BridgePipeline<B, S> {
    pub fn new(bridge: B, sink: S) -> Self {
        Self {
            api: GodotBridgeApi::new(bridge),
            sink,
        }
    }

    pub fn on_start(&mut self) -> Result<(), BridgeError> {
        self.api.on_start()
    }

    pub fn on_shutdown(&mut self) -> Result<(), BridgeError> {
        self.api.on_shutdown()
    }

    pub fn on_frame(&mut self, input: InputSnapshot) -> Result<(), BridgeError> {
        let frame = self.api.on_frame(input)?;
        self.sink.project(frame);
        Ok(())
    }
}

impl<B: GodotBridge + StateOverride, S: RenderSink> BridgePipeline<B, S> {
    pub fn request_state_override(
        &mut self,
        request: StateOverrideRequest,
    ) -> Result<(), BridgeError> {
        self.api.request_state_override(request)
    }
}

impl<B: GodotBridge> BridgePipeline<B, RenderFrameBuffer> {
    pub fn last_frame(&self) -> Option<&RenderFrame> {
        self.sink.last()
    }
}

pub trait EcsCore {
    fn init_world(&mut self) -> Result<(), CoreError>;
    fn tick(&mut self, input: InputSnapshot) -> Result<RenderFrame, CoreError>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct StateOverrideRequest {
    pub reason: String,
}

pub trait StateOverride {
    fn request_state_override(&mut self, request: StateOverrideRequest) -> Result<(), BridgeError>;
}

pub struct GodotBridgeAdapter<C: EcsCore> {
    core: C,
    started: bool,
}

pub struct GodotBridgeApi<B: GodotBridge> {
    bridge: B,
}

impl<B: GodotBridge> GodotBridgeApi<B> {
    pub fn new(bridge: B) -> Self {
        Self { bridge }
    }

    pub fn on_start(&mut self) -> Result<(), BridgeError> {
        self.bridge.on_start()
    }

    pub fn on_shutdown(&mut self) -> Result<(), BridgeError> {
        self.bridge.on_shutdown()
    }

    pub fn on_frame(&mut self, input: InputSnapshot) -> Result<RenderFrame, BridgeError> {
        self.bridge.on_frame(input)
    }
}

impl<B: GodotBridge + StateOverride> GodotBridgeApi<B> {
    pub fn request_state_override(
        &mut self,
        request: StateOverrideRequest,
    ) -> Result<(), BridgeError> {
        self.bridge.request_state_override(request)
    }
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

impl<C: EcsCore> StateOverride for GodotBridgeAdapter<C> {
    fn request_state_override(&mut self, request: StateOverrideRequest) -> Result<(), BridgeError> {
        GodotBridgeAdapter::request_state_override(self, request)
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

impl Default for CoreEcs {
    fn default() -> Self {
        Self::new()
    }
}

impl EcsCore for CoreEcs {
    fn init_world(&mut self) -> Result<(), CoreError> {
        let mut world = World::new();
        world.insert_resource(GameState {
            frame: FrameId(0),
            poses: vec![Pose::identity()],
        });
        self.world = Some(world);
        Ok(())
    }

    fn tick(&mut self, input: InputSnapshot) -> Result<RenderFrame, CoreError> {
        let world = self.world.as_mut().ok_or(CoreError::NotInitialized)?;
        world.insert_resource(input);
        self.schedule.run(world);
        let state = world.resource::<GameState>();
        let frame = state.frame;
        let poses = state.poses.clone();
        world.remove_resource::<InputSnapshot>();
        Ok(RenderFrame { frame, poses })
    }
}

#[derive(Resource, Debug, Clone, PartialEq)]
struct GameState {
    frame: FrameId,
    poses: Vec<Pose>,
}

fn advance_frame(mut state: ResMut<GameState>) {
    state.frame.0 += 1;
}

#[derive(GodotClass)]
#[class(base=Node)]
pub struct SuteraClientBridge {
    base: Base<Node>,
    pipeline: BridgePipeline<GodotBridgeAdapter<CoreEcs>, RenderFrameBuffer>,
    frame_id: FrameId,
    error_state: BridgeErrorState,
    projector: RenderStateProjector,
    #[export]
    target_node: OnEditor<Gd<Node3D>>,
}

#[godot_api]
impl INode for SuteraClientBridge {
    fn init(base: Base<Node>) -> Self {
        let core = CoreEcs::new();
        let bridge = GodotBridgeAdapter::new(core);
        Self {
            base,
            pipeline: BridgePipeline::new(bridge, RenderFrameBuffer::default()),
            frame_id: FrameId(0),
            error_state: BridgeErrorState::default(),
            projector: RenderStateProjector,
            target_node: OnEditor::default(),
        }
    }
}

#[godot_api]
impl SuteraClientBridge {
    #[func]
    pub fn on_start(&mut self) -> bool {
        match self.pipeline.on_start() {
            Ok(()) => true,
            Err(err) => {
                self.error_state.record(&err);
                godot_error!("{err}");
                false
            }
        }
    }

    #[func]
    pub fn on_shutdown(&mut self) -> bool {
        match self.pipeline.on_shutdown() {
            Ok(()) => true,
            Err(err) => {
                self.error_state.record(&err);
                godot_error!("{err}");
                false
            }
        }
    }

    #[func]
    pub fn on_frame(&mut self) -> bool {
        self.frame_id.0 += 1;
        let input = InputSnapshot {
            frame: self.frame_id,
            inputs: vec![InputEvent::Noop],
        };
        match self.pipeline.on_frame(input) {
            Ok(()) => {
                self.project_latest_frame();
                true
            }
            Err(err) => {
                self.error_state.record(&err);
                godot_error!("{err}");
                false
            }
        }
    }

    fn project_latest_frame(&mut self) {
        let frame = match self.pipeline.last_frame() {
            Some(frame) => frame,
            None => return,
        };
        self.projector.project(frame, &mut self.target_node);
    }

    #[func]
    pub fn last_error(&self) -> GString {
        GString::from(self.error_state.last_error().unwrap_or_default())
    }

    #[func]
    pub fn request_state_override(&mut self, reason: GString) -> bool {
        let request = StateOverrideRequest {
            reason: reason.to_string(),
        };
        match self.pipeline.request_state_override(request) {
            Ok(()) => true,
            Err(err) => {
                self.error_state.record(&err);
                godot_error!("{err}");
                false
            }
        }
    }
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
        frame_calls: usize,
        frame_result: Result<RenderFrame, BridgeError>,
        last_input: Option<InputSnapshot>,
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
                frame_calls: 0,
                frame_result: Err(BridgeError::NotStarted),
                last_input: None,
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
            self.frame_calls += 1;
            self.last_input = Some(_input);
            self.frame_result.clone()
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
    fn tick_fails_when_not_running() {
        let xr = FakeXr::default();
        let bridge = FakeBridge::default();
        let mut bootstrap = ClientBootstrap::new(xr, bridge);

        let result = bootstrap.tick(Vec::new());

        assert!(matches!(result, Err(FrameError::NotRunning)));
    }

    #[test]
    fn tick_reports_bridge_failure() {
        let xr = FakeXr {
            ready: true,
            enable_result: Ok(()),
            ..FakeXr::default()
        };
        let bridge = FakeBridge::default();
        let mut bootstrap = ClientBootstrap::new(xr, bridge);

        bootstrap.start().expect("start succeeds");
        let result = bootstrap.tick(Vec::new());

        assert!(matches!(result, Err(FrameError::Bridge(_))));
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
        let state = world.resource::<GameState>();
        assert_eq!(state.frame, FrameId(0));
        assert_eq!(state.poses, vec![Pose::identity()]);
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
        assert_eq!(first.poses, vec![Pose::identity()]);
        assert_eq!(second.frame, FrameId(2));
        assert_eq!(second.poses, vec![Pose::identity()]);

        let world = ecs.world.as_ref().expect("world initialized");
        let state = world.resource::<GameState>();
        assert_eq!(state.frame, FrameId(2));
        assert_eq!(state.poses, vec![Pose::identity()]);
    }

    #[test]
    fn tick_projects_state_to_render_frame() {
        let mut ecs = CoreEcs::new();
        ecs.init_world().expect("init succeeds");

        let expected = vec![Pose {
            position: Vec3 {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            },
            orientation: UnitQuat::identity(),
        }];
        let world = ecs.world.as_mut().expect("world initialized");
        world.resource_mut::<GameState>().poses = expected.clone();

        let render = ecs
            .tick(InputSnapshot {
                frame: FrameId(0),
                inputs: Vec::new(),
            })
            .expect("tick succeeds");

        assert_eq!(render.poses, expected);
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
        let core = FakeCore {
            init_result: Err(CoreError::InitFailed {
                reason: "init failed".to_string(),
            }),
            ..Default::default()
        };
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
    fn godot_bridge_api_forwards_start_and_shutdown() {
        let bridge = FakeBridge::default();
        let mut api = GodotBridgeApi::new(bridge);

        let start = api.on_start();
        let shutdown = api.on_shutdown();

        assert!(start.is_ok());
        assert!(shutdown.is_ok());
        assert_eq!(api.bridge.start_calls, 1);
        assert_eq!(api.bridge.shutdown_calls, 1);
    }

    #[test]
    fn godot_bridge_api_forwards_frame_input() {
        let bridge = FakeBridge {
            frame_result: Ok(RenderFrame {
                frame: FrameId(1),
                poses: vec![Pose::identity()],
            }),
            ..Default::default()
        };
        let mut api = GodotBridgeApi::new(bridge);
        let input = InputSnapshot {
            frame: FrameId(0),
            inputs: vec![InputEvent::Noop],
        };

        let result = api.on_frame(input.clone());

        assert!(result.is_ok());
        assert_eq!(api.bridge.frame_calls, 1);
        assert_eq!(api.bridge.last_input, Some(input));
    }

    #[test]
    fn client_bootstrap_ticks_render_frame_with_core_and_openxr() {
        let provider = FakeXrProvider {
            interface: Some(FakeXrInterface::default()),
        };
        let xr = OpenXrRuntime::new(provider);
        let core = CoreEcs::new();
        let bridge = GodotBridgeAdapter::new(core);
        let mut bootstrap = ClientBootstrap::new(xr, bridge);

        bootstrap.start().expect("start succeeds");
        let render = bootstrap.tick(Vec::new()).expect("tick succeeds");

        assert_eq!(render.frame, FrameId(1));
        assert_eq!(render.poses, vec![Pose::identity()]);
    }

    #[test]
    fn client_bootstrap_reports_openxr_missing_interface() {
        let provider = FakeXrProvider { interface: None };
        let xr = OpenXrRuntime::new(provider);
        let core = CoreEcs::new();
        let bridge = GodotBridgeAdapter::new(core);
        let mut bootstrap = ClientBootstrap::new(xr, bridge);

        let result = bootstrap.start();

        assert!(matches!(result, Err(StartError::XrInit(_))));
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

    #[derive(Clone)]
    struct CapturingSink {
        captured: std::rc::Rc<std::cell::RefCell<Option<RenderFrame>>>,
    }

    impl RenderSink for CapturingSink {
        fn project(&mut self, frame: RenderFrame) {
            *self.captured.borrow_mut() = Some(frame);
        }
    }

    #[test]
    fn bridge_pipeline_projects_render_frame() {
        let bridge = FakeBridge {
            frame_result: Ok(RenderFrame {
                frame: FrameId(1),
                poses: vec![Pose::identity()],
            }),
            ..Default::default()
        };
        let captured = std::rc::Rc::new(std::cell::RefCell::new(None));
        let sink = CapturingSink {
            captured: captured.clone(),
        };
        let mut pipeline = BridgePipeline::new(bridge, sink);

        pipeline.on_start().expect("start succeeds");
        let input = InputSnapshot {
            frame: FrameId(0),
            inputs: vec![InputEvent::Noop],
        };
        pipeline.on_frame(input).expect("frame succeeds");

        assert_eq!(
            *captured.borrow(),
            Some(RenderFrame {
                frame: FrameId(1),
                poses: vec![Pose::identity()],
            })
        );
    }

    #[test]
    fn gdextension_entry_pipeline_runs_ecs_and_buffers_frame() {
        let core = CoreEcs::new();
        let bridge = GodotBridgeAdapter::new(core);
        let mut pipeline = BridgePipeline::new(bridge, RenderFrameBuffer::default());

        pipeline.on_start().expect("start succeeds");
        let input = InputSnapshot {
            frame: FrameId(0),
            inputs: vec![InputEvent::Noop],
        };
        pipeline.on_frame(input).expect("frame succeeds");

        assert_eq!(
            pipeline.last_frame(),
            Some(&RenderFrame {
                frame: FrameId(1),
                poses: vec![Pose::identity()],
            })
        );
    }

    #[test]
    fn bridge_error_state_records_last_error() {
        let mut state = BridgeErrorState::default();
        let err = BridgeError::InitializationFailed {
            reason: "gdext init failed".to_string(),
        };

        state.record(&err);

        assert_eq!(
            state.last_error(),
            Some("bridge initialization failed: gdext init failed")
        );
    }

    #[test]
    fn pose_to_transform3d_maps_translation_and_rotation() {
        let pose = Pose {
            position: Vec3 {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            },
            orientation: UnitQuat::identity(),
        };

        let transform = pose_to_transform3d(&pose);

        assert_eq!(transform.origin, Vector3::new(1.0, 2.0, 3.0));
        assert_eq!(transform.basis, Basis::IDENTITY);
    }

    #[test]
    fn render_frame_first_transform_picks_first_pose() {
        let frame = RenderFrame {
            frame: FrameId(1),
            poses: vec![
                Pose {
                    position: Vec3 {
                        x: 0.2,
                        y: 0.4,
                        z: 0.6,
                    },
                    orientation: UnitQuat::identity(),
                },
                Pose {
                    position: Vec3 {
                        x: 9.0,
                        y: 9.0,
                        z: 9.0,
                    },
                    orientation: UnitQuat::identity(),
                },
            ],
        };

        let transform = render_frame_first_transform(&frame).expect("first pose");

        assert_eq!(transform.origin, Vector3::new(0.2, 0.4, 0.6));
    }

    struct CapturingTarget {
        last: Option<Transform3D>,
    }

    impl TransformTarget for CapturingTarget {
        fn set_transform(&mut self, transform: Transform3D) {
            self.last = Some(transform);
        }
    }

    #[test]
    fn render_frame_projects_pose_to_target_transform() {
        let frame = RenderFrame {
            frame: FrameId(1),
            poses: vec![Pose {
                position: Vec3 {
                    x: 1.5,
                    y: 2.5,
                    z: 3.5,
                },
                orientation: UnitQuat::identity(),
            }],
        };
        let mut target = CapturingTarget { last: None };

        let applied = project_render_frame_to_target(&frame, &mut target);

        assert!(applied);
        assert_eq!(target.last.unwrap().origin, Vector3::new(1.5, 2.5, 3.5));
    }

    #[test]
    fn godot_bridge_api_rejects_state_override_request() {
        let core = FakeCore::default();
        let bridge = GodotBridgeAdapter::new(core);
        let mut api = GodotBridgeApi::new(bridge);

        let result = api.request_state_override(StateOverrideRequest {
            reason: "manual override".to_string(),
        });

        assert!(matches!(
            result,
            Err(BridgeError::DirectStateMutationDenied)
        ));
    }
}
