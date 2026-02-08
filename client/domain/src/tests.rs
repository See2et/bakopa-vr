use super::bridge::{
    BridgePipeline, ClientBootstrap, ClientLifecycle, RuntimeBridge, RuntimeBridgeAdapter,
    RuntimeMode, RuntimeModePreference,
};
use super::ecs::{
    CoreEcs, EcsCore, FrameClock, FrameId, InputEvent, InputSnapshot, Pose, RenderFrame, UnitQuat,
    Vec3, DEFAULT_INPUT_DT_SECONDS,
};
use super::errors::{
    BridgeError, BridgeErrorState, CoreError, FrameError, ShutdownError, StartError, XrError,
};
use super::ports::{InputPort, NoopInputPort, OutputPort, RenderFrameBuffer};
use super::sync::{
    runtime_mode_label, ParticipantId, PoseSyncCoordinator, PoseVersion, RemoteLiveness,
    RemotePoseRepository, RemotePoseUpdate, ScopeBoundaryError, ScopeBoundaryPolicy,
    SignalingRoute, SyncDelta, SyncSessionError, SyncSessionPort,
};
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
    let mut bootstrap = ClientBootstrap::new_with_mode(xr, bridge, RuntimeModePreference::Vr);

    let result = bootstrap.start();

    assert!(result.is_ok());
    assert_eq!(bootstrap.bridge.start_calls, 1);
    assert_eq!(bootstrap.runtime_mode(), RuntimeMode::Desktop);
    assert!(bootstrap.start_diagnostics().xr_failure_reason().is_some());
}

#[test]
fn start_reports_xr_not_ready() {
    let xr = FakeXr {
        ready: false,
        enable_result: Ok(()),
        ..FakeXr::default()
    };
    let bridge = FakeBridge::default();
    let mut bootstrap = ClientBootstrap::new_with_mode(xr, bridge, RuntimeModePreference::Vr);

    let result = bootstrap.start();

    assert!(result.is_ok());
    assert_eq!(bootstrap.bridge.start_calls, 1);
    assert_eq!(bootstrap.runtime_mode(), RuntimeMode::Desktop);
    assert_eq!(
        bootstrap.start_diagnostics().xr_failure_reason(),
        Some("xr runtime not ready")
    );
}

#[test]
fn start_uses_desktop_mode_without_xr_initialization() {
    let xr = FakeXr::default();
    let bridge = FakeBridge::default();
    let mut bootstrap = ClientBootstrap::new_with_mode(xr, bridge, RuntimeModePreference::Desktop);

    let result = bootstrap.start();

    assert!(result.is_ok());
    assert_eq!(bootstrap.xr.enable_calls, 0);
    assert_eq!(bootstrap.runtime_mode(), RuntimeMode::Desktop);
    assert!(bootstrap.start_diagnostics().xr_failure_reason().is_none());
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
    let mut bootstrap = ClientBootstrap::new_with_mode(xr, bridge, RuntimeModePreference::Vr);

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
fn tick_uses_current_frame_after_tick_frame_advance() {
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
    let frame = bootstrap.tick_frame().expect("frame advanced");
    bootstrap.tick(Vec::new()).expect("tick succeeds");

    assert_eq!(frame, FrameId(1));
    assert_eq!(
        bootstrap.bridge.last_input.map(|input| input.frame),
        Some(FrameId(1))
    );
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
            dt_seconds: DEFAULT_INPUT_DT_SECONDS,
            inputs: Vec::new(),
        })
        .expect("first tick");
    let second = ecs
        .tick(InputSnapshot {
            frame: FrameId(1),
            dt_seconds: DEFAULT_INPUT_DT_SECONDS,
            inputs: Vec::new(),
        })
        .expect("second tick");

    assert_eq!(first.frame, FrameId(1));
    assert_eq!(second.frame, FrameId(2));

    #[cfg(not(feature = "demo-motion"))]
    {
        assert_eq!(first.primary_pose(), &Pose::identity());
        assert_eq!(second.primary_pose(), &Pose::identity());
    }

    #[cfg(feature = "demo-motion")]
    {
        assert!((0.0..=0.5).contains(&first.primary_pose().position.x));
        assert!((0.0..=0.5).contains(&second.primary_pose().position.x));
        assert_eq!(first.primary_pose().position.y, 0.0);
        assert_eq!(first.primary_pose().position.z, 0.0);
        assert_eq!(second.primary_pose().position.y, 0.0);
        assert_eq!(second.primary_pose().position.z, 0.0);
        assert_eq!(first.primary_pose().orientation, UnitQuat::identity());
        assert_eq!(second.primary_pose().orientation, UnitQuat::identity());
    }
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
            dt_seconds: DEFAULT_INPUT_DT_SECONDS,
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
        dt_seconds: DEFAULT_INPUT_DT_SECONDS,
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
        dt_seconds: DEFAULT_INPUT_DT_SECONDS,
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

    assert!(result.is_ok());
    assert_eq!(bootstrap.runtime_mode(), RuntimeMode::Desktop);
    assert!(bootstrap.start_diagnostics().xr_failure_reason().is_some());
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
        dt_seconds: DEFAULT_INPUT_DT_SECONDS,
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

#[test]
fn movement_system_updates_position_frame_rate_independently() {
    let mut single_step = CoreEcs::new();
    let single = single_step
        .tick(InputSnapshot {
            frame: FrameId(0),
            dt_seconds: 1.0,
            inputs: vec![InputEvent::Move {
                axis_x: 1.0,
                axis_y: 0.0,
            }],
        })
        .expect("single step tick");

    let mut split_step = CoreEcs::new();
    split_step
        .tick(InputSnapshot {
            frame: FrameId(0),
            dt_seconds: 0.5,
            inputs: vec![InputEvent::Move {
                axis_x: 1.0,
                axis_y: 0.0,
            }],
        })
        .expect("split step 1");
    let split = split_step
        .tick(InputSnapshot {
            frame: FrameId(1),
            dt_seconds: 0.5,
            inputs: vec![InputEvent::Move {
                axis_x: 1.0,
                axis_y: 0.0,
            }],
        })
        .expect("split step 2");

    assert!((single.primary_pose().position.x - split.primary_pose().position.x).abs() < 1.0e-6);
    assert!((single.primary_pose().position.z - split.primary_pose().position.z).abs() < 1.0e-6);
}

#[test]
fn movement_system_keeps_pose_unchanged_without_input() {
    let mut ecs = CoreEcs::new();
    let frame = ecs
        .tick(InputSnapshot {
            frame: FrameId(0),
            dt_seconds: 0.25,
            inputs: Vec::new(),
        })
        .expect("tick without input");

    assert_eq!(frame.primary_pose(), &Pose::identity());
}

#[derive(Default)]
struct FakeSyncSessionPort {
    sent: Vec<(FrameId, RuntimeMode, Pose)>,
}

impl SyncSessionPort for FakeSyncSessionPort {
    fn send_local_pose(
        &mut self,
        frame: FrameId,
        mode: RuntimeMode,
        pose: Pose,
    ) -> Result<(), SyncSessionError> {
        self.sent.push((frame, mode, pose));
        Ok(())
    }
}

#[test]
fn pose_sync_coordinator_sends_updated_local_pose_after_frame_update() {
    let mut coordinator = PoseSyncCoordinator::new(CoreEcs::new(), FakeSyncSessionPort::default());
    let frame = coordinator
        .apply_frame(
            RuntimeMode::Desktop,
            InputSnapshot {
                frame: FrameId(0),
                dt_seconds: 1.0,
                inputs: vec![InputEvent::Move {
                    axis_x: 1.0,
                    axis_y: 0.0,
                }],
            },
        )
        .expect("coordinator apply frame");

    assert_eq!(frame.frame, FrameId(1));
    assert!(frame.primary_pose().position.x > 0.0);
    assert_eq!(coordinator.sync_port().sent.len(), 1);
    let (_, mode, sent_pose) = coordinator.sync_port().sent[0];
    assert_eq!(mode, RuntimeMode::Desktop);
    assert_eq!(sent_pose, *frame.primary_pose());
}

#[derive(Default)]
struct FailingSyncSessionPort;

impl SyncSessionPort for FailingSyncSessionPort {
    fn send_local_pose(
        &mut self,
        _frame: FrameId,
        _mode: RuntimeMode,
        _pose: Pose,
    ) -> Result<(), SyncSessionError> {
        Err(SyncSessionError::TransportUnavailable {
            reason: "test transport down".to_string(),
        })
    }
}

struct NotReadySyncSessionPort;

impl SyncSessionPort for NotReadySyncSessionPort {
    fn send_local_pose(
        &mut self,
        _frame: FrameId,
        _mode: RuntimeMode,
        _pose: Pose,
    ) -> Result<(), SyncSessionError> {
        Err(SyncSessionError::NotReady {
            reason: "room not joined".to_string(),
        })
    }
}

#[test]
fn pose_sync_coordinator_continues_frame_when_send_fails() {
    let mut coordinator = PoseSyncCoordinator::new(CoreEcs::new(), FailingSyncSessionPort);

    let frame = coordinator
        .apply_frame(
            RuntimeMode::Vr,
            InputSnapshot {
                frame: FrameId(0),
                dt_seconds: 0.5,
                inputs: vec![InputEvent::Move {
                    axis_x: 1.0,
                    axis_y: 0.0,
                }],
            },
        )
        .expect("frame must continue even if sync send fails");

    assert_eq!(frame.frame, FrameId(1));
    assert!(matches!(
        coordinator.last_sync_error_ref(),
        Some(SyncSessionError::TransportUnavailable { .. })
    ));
}

#[test]
fn pose_sync_coordinator_skips_send_when_sync_session_not_ready() {
    let mut coordinator = PoseSyncCoordinator::new(CoreEcs::new(), NotReadySyncSessionPort);
    let frame = coordinator
        .apply_frame(
            RuntimeMode::Desktop,
            InputSnapshot {
                frame: FrameId(0),
                dt_seconds: 0.25,
                inputs: vec![InputEvent::Move {
                    axis_x: 1.0,
                    axis_y: 0.0,
                }],
            },
        )
        .expect("frame must continue when sync session is not ready");

    assert_eq!(frame.frame, FrameId(1));
    assert!(coordinator.last_sync_error_ref().is_none());
}

#[derive(Default)]
struct PollingSyncSessionPort {
    pending_deltas: Vec<SyncDelta>,
}

impl SyncSessionPort for PollingSyncSessionPort {
    fn send_local_pose(
        &mut self,
        _frame: FrameId,
        _mode: RuntimeMode,
        _pose: Pose,
    ) -> Result<(), SyncSessionError> {
        Ok(())
    }

    fn poll_events(&mut self) -> Vec<SyncDelta> {
        std::mem::take(&mut self.pending_deltas)
    }
}

#[test]
fn pose_sync_coordinator_applies_polled_sync_events_before_composing_frame() {
    let participant_id = ParticipantId::new("peer-polled");
    let remote_pose = Pose {
        position: Vec3 {
            x: 3.0,
            y: 0.5,
            z: -1.0,
        },
        orientation: UnitQuat::identity(),
    };
    let mut coordinator = PoseSyncCoordinator::new(
        CoreEcs::new(),
        PollingSyncSessionPort {
            pending_deltas: vec![
                SyncDelta::PeerJoined {
                    participant_id: participant_id.clone(),
                    session_epoch: 7,
                },
                SyncDelta::PoseReceived {
                    participant_id,
                    pose: remote_pose,
                    version: PoseVersion {
                        session_epoch: 7,
                        pose_seq: 1,
                    },
                },
            ],
        },
    );

    let frame = coordinator
        .apply_frame(
            RuntimeMode::Desktop,
            InputSnapshot {
                frame: FrameId(0),
                dt_seconds: 0.1,
                inputs: Vec::new(),
            },
        )
        .expect("frame should include polled remote pose");

    assert_eq!(frame.remote_poses().len(), 1);
    assert_eq!(frame.remote_poses()[0].participant_id, "peer-polled");
    assert_eq!(frame.remote_poses()[0].pose, remote_pose);
}

#[test]
fn pose_sync_coordinator_prioritizes_lifecycle_events_before_pose_events() {
    let participant_id = ParticipantId::new("peer-lifecycle-priority");
    let remote_pose = Pose {
        position: Vec3 {
            x: 4.0,
            y: 0.0,
            z: -3.0,
        },
        orientation: UnitQuat::identity(),
    };
    let mut coordinator = PoseSyncCoordinator::new(
        CoreEcs::new(),
        PollingSyncSessionPort {
            pending_deltas: vec![
                SyncDelta::PoseReceived {
                    participant_id: participant_id.clone(),
                    pose: remote_pose,
                    version: PoseVersion {
                        session_epoch: 9,
                        pose_seq: 1,
                    },
                },
                SyncDelta::PeerJoined {
                    participant_id,
                    session_epoch: 9,
                },
            ],
        },
    );

    let frame = coordinator
        .apply_frame(
            RuntimeMode::Desktop,
            InputSnapshot {
                frame: FrameId(0),
                dt_seconds: 0.1,
                inputs: Vec::new(),
            },
        )
        .expect("frame should apply lifecycle before pose");

    assert_eq!(frame.remote_poses().len(), 1);
    assert_eq!(frame.remote_poses()[0].pose, remote_pose);
}

#[derive(Default)]
struct ShutdownAwareSyncSessionPort {
    begin_shutdown_called: bool,
    drain_deltas: Vec<SyncDelta>,
}

impl SyncSessionPort for ShutdownAwareSyncSessionPort {
    fn send_local_pose(
        &mut self,
        _frame: FrameId,
        _mode: RuntimeMode,
        _pose: Pose,
    ) -> Result<(), SyncSessionError> {
        Ok(())
    }

    fn begin_shutdown(&mut self) {
        self.begin_shutdown_called = true;
    }

    fn drain_pending_events(&mut self) -> Vec<SyncDelta> {
        std::mem::take(&mut self.drain_deltas)
    }
}

#[test]
fn pose_sync_coordinator_shutdown_drains_control_events_and_drops_pose_events() {
    let participant_id = ParticipantId::new("peer-shutdown");
    let remote_pose = Pose {
        position: Vec3 {
            x: 11.0,
            y: 0.0,
            z: 0.0,
        },
        orientation: UnitQuat::identity(),
    };
    let mut coordinator = PoseSyncCoordinator::new(
        CoreEcs::new(),
        ShutdownAwareSyncSessionPort {
            begin_shutdown_called: false,
            drain_deltas: vec![
                SyncDelta::PeerJoined {
                    participant_id: participant_id.clone(),
                    session_epoch: 3,
                },
                SyncDelta::PoseReceived {
                    participant_id: participant_id.clone(),
                    pose: remote_pose,
                    version: PoseVersion {
                        session_epoch: 3,
                        pose_seq: 1,
                    },
                },
            ],
        },
    );

    let report = coordinator.shutdown_sync_session();

    assert!(coordinator.sync_port().begin_shutdown_called);
    assert_eq!(report.applied_control_events, 1);
    assert_eq!(report.dropped_pose_events, 1);
    assert_eq!(coordinator.remotes().render_snapshot().len(), 0);
}

#[test]
fn movement_system_input_application_is_deterministic() {
    let input = InputSnapshot {
        frame: FrameId(0),
        dt_seconds: 0.25,
        inputs: vec![
            InputEvent::Move {
                axis_x: 0.4,
                axis_y: -0.8,
            },
            InputEvent::Look {
                yaw_delta: 0.5,
                pitch_delta: -0.1,
            },
        ],
    };
    let mut first = CoreEcs::new();
    let mut second = CoreEcs::new();

    let first_frame = first.tick(input.clone()).expect("first deterministic tick");
    let second_frame = second.tick(input).expect("second deterministic tick");

    assert_eq!(first_frame.primary_pose(), second_frame.primary_pose());
}

#[test]
fn remote_pose_repository_applies_newer_and_drops_stale_versions() {
    let mut repository = RemotePoseRepository::new();
    let participant_id = ParticipantId::new("peer-1");
    let older_pose = Pose {
        position: Vec3 {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        },
        orientation: UnitQuat::identity(),
    };
    let newer_pose = Pose {
        position: Vec3 {
            x: 2.0,
            y: 0.0,
            z: 0.0,
        },
        orientation: UnitQuat::identity(),
    };

    let first = repository.apply_if_newer(
        participant_id.clone(),
        older_pose,
        PoseVersion {
            session_epoch: 1,
            pose_seq: 1,
        },
    );
    let stale = repository.apply_if_newer(
        participant_id.clone(),
        Pose {
            position: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            orientation: UnitQuat::identity(),
        },
        PoseVersion {
            session_epoch: 1,
            pose_seq: 1,
        },
    );
    let newer = repository.apply_if_newer(
        participant_id.clone(),
        newer_pose,
        PoseVersion {
            session_epoch: 1,
            pose_seq: 2,
        },
    );

    assert_eq!(first, RemotePoseUpdate::Applied);
    assert_eq!(stale, RemotePoseUpdate::StaleDropped);
    assert_eq!(newer, RemotePoseUpdate::Applied);
    assert_eq!(
        repository
            .get(&participant_id)
            .and_then(|state| state.pose_state)
            .map(|state| state.pose),
        Some(newer_pose)
    );
}

#[test]
fn remote_pose_repository_handles_join_left_rejoin_and_inactive() {
    let mut repository = RemotePoseRepository::new();
    let participant_id = ParticipantId::new("peer-2");

    repository.on_peer_joined(participant_id.clone(), 1);
    assert_eq!(
        repository
            .get(&participant_id)
            .map(|state| state.session_epoch),
        Some(1)
    );

    assert!(repository.mark_inactive(&participant_id));
    assert_eq!(
        repository.get(&participant_id).map(|state| state.liveness),
        Some(RemoteLiveness::SuspectedDisconnected)
    );

    repository.on_peer_joined(participant_id.clone(), 2);
    assert_eq!(
        repository
            .get(&participant_id)
            .map(|state| state.session_epoch),
        Some(2)
    );
    assert_eq!(
        repository
            .get(&participant_id)
            .map(|state| state.pose_state),
        Some(None)
    );

    assert!(repository.on_peer_left(&participant_id));
    assert!(!repository.on_peer_left(&participant_id));
    assert!(repository.get(&participant_id).is_none());
}

#[test]
fn pose_sync_coordinator_reflects_remote_updates_on_next_frame_and_removes_left_peer() {
    let mut coordinator = PoseSyncCoordinator::new(CoreEcs::new(), FakeSyncSessionPort::default());
    let participant_id = ParticipantId::new("peer-render");
    coordinator.on_peer_joined(participant_id.clone(), 1);
    let remote_pose = Pose {
        position: Vec3 {
            x: 9.0,
            y: 1.0,
            z: -2.0,
        },
        orientation: UnitQuat::identity(),
    };
    let update = coordinator.apply_remote_pose(
        participant_id.clone(),
        remote_pose,
        PoseVersion {
            session_epoch: 1,
            pose_seq: 1,
        },
    );
    assert_eq!(update, RemotePoseUpdate::Applied);

    let frame = coordinator
        .apply_frame(
            RuntimeMode::Desktop,
            InputSnapshot {
                frame: FrameId(0),
                dt_seconds: 0.1,
                inputs: Vec::new(),
            },
        )
        .expect("frame with remote pose");
    assert_eq!(frame.remote_poses().len(), 1);
    assert_eq!(
        frame.remote_poses()[0].participant_id,
        "peer-render".to_string()
    );
    assert_eq!(frame.remote_poses()[0].pose, remote_pose);

    assert!(coordinator.on_peer_left(&participant_id));
    let after_left = coordinator
        .apply_frame(
            RuntimeMode::Desktop,
            InputSnapshot {
                frame: FrameId(1),
                dt_seconds: 0.1,
                inputs: Vec::new(),
            },
        )
        .expect("frame after peer left");
    assert!(after_left.remote_poses().is_empty());
}

#[test]
fn runtime_mode_label_is_stable_for_sync_trace_fields() {
    assert_eq!(runtime_mode_label(RuntimeMode::Desktop), "desktop");
    assert_eq!(runtime_mode_label(RuntimeMode::Vr), "vr");
}

#[test]
fn scope_boundary_policy_rejects_voice_stream_kind() {
    let result = ScopeBoundaryPolicy::default().ensure_stream_kind("voice");

    assert!(matches!(
        result,
        Err(ScopeBoundaryError::UnsupportedStreamKind { .. })
    ));
}

#[test]
fn scope_boundary_policy_rejects_bloom_production_signaling() {
    let result = ScopeBoundaryPolicy::with_signaling_route(SignalingRoute::BloomProduction)
        .ensure_signaling_route();

    assert_eq!(
        result,
        Err(ScopeBoundaryError::ProductionBloomSignalingOutOfScope)
    );
}
