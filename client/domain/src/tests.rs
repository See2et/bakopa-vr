use super::bridge::{
    BridgePipeline, ClientBootstrap, ClientLifecycle, RuntimeBridge, RuntimeBridgeAdapter,
};
use super::ecs::{
    CoreEcs, EcsCore, FrameClock, FrameId, InputEvent, InputSnapshot, Pose, RenderFrame, UnitQuat,
    Vec3,
};
use super::errors::{
    BridgeError, BridgeErrorState, CoreError, FrameError, ShutdownError, StartError, XrError,
};
use super::ports::{InputPort, NoopInputPort, OutputPort, RenderFrameBuffer};
use super::xr::{OpenXrRuntime, XrInterfaceAccess, XrInterfaceProvider, XrRuntime};

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
            initialized: false,
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

impl RuntimeBridge for FakeBridge {
    fn on_start(&mut self) -> Result<(), BridgeError> {
        self.start_calls += 1;
        self.start_result.clone()
    }

    fn on_shutdown(&mut self) -> Result<(), BridgeError> {
        self.shutdown_calls += 1;
        self.shutdown_result.clone()
    }

    fn on_frame(&mut self, input: InputSnapshot) -> Result<RenderFrame, BridgeError> {
        self.frame_calls += 1;
        self.last_input = Some(input);
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
    let bridge = FakeBridge::default();
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
        frame_result: Ok(RenderFrame::from_primary_pose(FrameId(0), Pose::identity())),
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
fn restart_resets_frame_clock_to_origin() {
    let xr = FakeXr {
        ready: true,
        enable_result: Ok(()),
        ..FakeXr::default()
    };
    let bridge = FakeBridge {
        start_result: Ok(()),
        frame_result: Ok(RenderFrame::from_primary_pose(FrameId(0), Pose::identity())),
        ..FakeBridge::default()
    };
    let mut bootstrap = ClientBootstrap::new(xr, bridge);

    bootstrap.start().expect("first start succeeds");
    let before_restart = bootstrap.tick_frame().expect("frame before restart");
    bootstrap.shutdown().expect("shutdown succeeds");

    bootstrap.start().expect("second start succeeds");
    let after_restart = bootstrap.tick_frame().expect("frame after restart");

    assert_eq!(before_restart, FrameId(1));
    assert_eq!(after_restart, FrameId(1));
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
        frame_result: Ok(RenderFrame::from_primary_pose(FrameId(1), Pose::identity())),
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
    assert_eq!(bootstrap.xr.shutdown_calls, 1);
}

#[test]
fn shutdown_returns_bridge_error_when_both_shutdowns_fail() {
    let xr = FakeXr {
        ready: true,
        enable_result: Ok(()),
        shutdown_result: Err(XrError::ShutdownFailed {
            reason: "xr stuck".to_string(),
        }),
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
    assert_eq!(bootstrap.xr.shutdown_calls, 1);
    assert!(matches!(
        bootstrap.tick_frame(),
        Err(FrameError::NotRunning)
    ));
}

#[test]
fn core_ecs_initial_state_is_available() {
    let ecs = CoreEcs::new();
    let (frame, pose) = ecs.current_state();

    assert_eq!(frame, FrameId(0));
    assert_eq!(pose, Pose::identity());
}

#[test]
fn core_ecs_tick_runs_systems_and_advances_frame() {
    let mut ecs = CoreEcs::new();

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
    assert_eq!(first.primary_pose(), &Pose::identity());
    assert_eq!(second.primary_pose(), &Pose::identity());
}

#[test]
fn core_ecs_init_world_resets_state() {
    let mut ecs = CoreEcs::new();
    ecs.set_primary_pose_for_test(Pose {
        position: Vec3 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        },
        orientation: UnitQuat::identity(),
    });

    ecs.init_world().expect("init succeeds");
    let (frame, pose) = ecs.current_state();

    assert_eq!(frame, FrameId(0));
    assert_eq!(pose, Pose::identity());
}

#[test]
fn core_ecs_is_tickable_without_explicit_init_world_call() {
    let mut ecs = CoreEcs::new();

    let frame = ecs
        .tick(InputSnapshot {
            frame: FrameId(0),
            inputs: Vec::new(),
        })
        .expect("core is initialized by constructor");

    assert_eq!(frame.frame, FrameId(1));
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
        Self::with_tick_result(Ok(RenderFrame::from_primary_pose(
            FrameId(0),
            Pose::identity(),
        )))
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
fn runtime_bridge_initializes_on_start() {
    let core = FakeCore::default();
    let mut bridge = RuntimeBridgeAdapter::new(core);

    let result = bridge.on_start();

    assert!(result.is_ok());
    assert_eq!(bridge.core.init_calls, 1);
}

#[test]
fn runtime_bridge_reports_initialization_failure() {
    let core = FakeCore {
        init_result: Err(CoreError::InitFailed {
            reason: "init failed".to_string(),
        }),
        ..Default::default()
    };
    let mut bridge = RuntimeBridgeAdapter::new(core);

    let result = bridge.on_start();

    assert!(matches!(result, Err(BridgeError::CoreInit(_))));
    assert_eq!(bridge.core.init_calls, 1);
}

#[test]
fn runtime_bridge_rejects_frame_before_start() {
    let core = FakeCore::default();
    let mut bridge = RuntimeBridgeAdapter::new(core);

    let result = bridge.on_frame(InputSnapshot {
        frame: FrameId(0),
        inputs: Vec::new(),
    });

    assert!(matches!(result, Err(BridgeError::NotStarted)));
}

#[test]
fn runtime_bridge_forwards_frame_input_to_core() {
    let core = FakeCore::with_tick_result(Ok(RenderFrame::from_primary_pose(
        FrameId(1),
        Pose::identity(),
    )));
    let mut bridge = RuntimeBridgeAdapter::new(core);
    bridge.on_start().expect("start succeeds");

    let input = InputSnapshot {
        frame: FrameId(0),
        inputs: vec![InputEvent::Action {
            name: "jump".to_string(),
            pressed: true,
        }],
    };
    let result = bridge.on_frame(input.clone());

    assert!(result.is_ok());
    assert_eq!(bridge.core.tick_calls, 1);
    assert_eq!(bridge.core.last_input, Some(input));
}

#[test]
fn runtime_bridge_rejects_direct_state_override() {
    let core = FakeCore::default();
    let mut bridge = RuntimeBridgeAdapter::new(core);

    let result = bridge.request_state_override(super::bridge::StateOverrideRequest {
        reason: "direct change".to_string(),
    });

    assert!(matches!(
        result,
        Err(BridgeError::DirectStateMutationDenied)
    ));
}

#[test]
fn client_bootstrap_ticks_render_frame_with_core_and_openxr() {
    let provider = FakeXrProvider {
        interface: Some(FakeXrInterface::default()),
    };
    let xr = OpenXrRuntime::new(provider);
    let core = CoreEcs::new();
    let bridge = RuntimeBridgeAdapter::new(core);
    let mut bootstrap = ClientBootstrap::new(xr, bridge);

    bootstrap.start().expect("start succeeds");
    let render = bootstrap.tick(Vec::new()).expect("tick succeeds");

    assert_eq!(render.frame, FrameId(1));
    assert_eq!(render.primary_pose(), &Pose::identity());
}

#[test]
fn client_bootstrap_reports_openxr_missing_interface() {
    let provider = FakeXrProvider { interface: None };
    let xr = OpenXrRuntime::new(provider);
    let core = CoreEcs::new();
    let bridge = RuntimeBridgeAdapter::new(core);
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

impl OutputPort for CapturingSink {
    fn project(&mut self, frame: RenderFrame) {
        *self.captured.borrow_mut() = Some(frame);
    }
}

#[test]
fn bridge_pipeline_projects_render_frame() {
    let bridge = FakeBridge {
        frame_result: Ok(RenderFrame::from_primary_pose(FrameId(1), Pose::identity())),
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
        inputs: vec![InputEvent::Action {
            name: "jump".to_string(),
            pressed: true,
        }],
    };
    pipeline.on_frame(input).expect("frame succeeds");

    assert_eq!(
        *captured.borrow(),
        Some(RenderFrame::from_primary_pose(FrameId(1), Pose::identity()))
    );
}

#[test]
fn bridge_pipeline_on_port_input_uses_shared_frame_clock() {
    struct EchoFrameBridge;

    impl RuntimeBridge for EchoFrameBridge {
        fn on_start(&mut self) -> Result<(), BridgeError> {
            Ok(())
        }

        fn on_shutdown(&mut self) -> Result<(), BridgeError> {
            Ok(())
        }

        fn on_frame(&mut self, input: InputSnapshot) -> Result<RenderFrame, BridgeError> {
            Ok(RenderFrame::from_primary_pose(
                input.frame,
                Pose::identity(),
            ))
        }
    }

    let mut pipeline = BridgePipeline::new(EchoFrameBridge, RenderFrameBuffer::default());
    let mut clock = FrameClock::default();
    let mut input_port = NoopInputPort;

    pipeline.on_start().expect("start succeeds");
    pipeline
        .on_port_input(&mut clock, &mut input_port)
        .expect("first frame");
    pipeline
        .on_port_input(&mut clock, &mut input_port)
        .expect("second frame");

    assert_eq!(
        pipeline.last_frame(),
        Some(&RenderFrame::from_primary_pose(
            FrameId(2),
            Pose::identity()
        ))
    );
}

#[test]
fn gdextension_entry_pipeline_runs_ecs_and_buffers_frame() {
    let core = CoreEcs::new();
    let bridge = RuntimeBridgeAdapter::new(core);
    let mut pipeline = BridgePipeline::new(bridge, RenderFrameBuffer::default());
    let mut clock = FrameClock::default();
    let mut input_port = NoopInputPort;

    pipeline.on_start().expect("start succeeds");
    pipeline
        .on_port_input(&mut clock, &mut input_port)
        .expect("frame succeeds");

    assert_eq!(
        pipeline.last_frame(),
        Some(&RenderFrame::from_primary_pose(
            FrameId(1),
            Pose::identity()
        ))
    );
}

#[test]
fn bridge_error_state_records_last_error_message() {
    let mut state = BridgeErrorState::default();
    let err = BridgeError::InitializationFailed {
        reason: "gdext init failed".to_string(),
    };

    state.record(&err);

    assert_eq!(
        state.last_message().as_deref(),
        Some("bridge initialization failed: gdext init failed")
    );
}

#[test]
fn bridge_error_state_keeps_typed_error_information() {
    let mut state = BridgeErrorState::default();
    let err = BridgeError::DirectStateMutationDenied;

    state.record(&err);

    assert!(matches!(
        state.last(),
        Some(BridgeError::DirectStateMutationDenied)
    ));
}

#[test]
fn render_frame_exposes_primary_pose_accessor() {
    let pose = Pose::identity();
    let frame = RenderFrame::from_primary_pose(FrameId(1), pose);

    assert_eq!(frame.primary_pose(), &pose);
}

#[test]
fn noop_input_port_produces_domain_snapshot() {
    let mut port = NoopInputPort;
    let mut clock = FrameClock::default();

    let snapshot = port.snapshot(&mut clock);

    assert_eq!(snapshot.frame, FrameId(1));
    assert!(snapshot.inputs.is_empty());
}

#[test]
fn input_event_supports_minimal_domain_variants() {
    let move_event = InputEvent::Move {
        axis_x: 1.0,
        axis_y: -1.0,
    };
    let look_event = InputEvent::Look {
        yaw_delta: 0.25,
        pitch_delta: -0.5,
    };
    let action_event = InputEvent::Action {
        name: "jump".to_string(),
        pressed: true,
    };

    assert!(matches!(move_event, InputEvent::Move { .. }));
    assert!(matches!(look_event, InputEvent::Look { .. }));
    assert!(matches!(action_event, InputEvent::Action { .. }));
}
