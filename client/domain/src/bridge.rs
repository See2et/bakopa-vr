use crate::ecs::{
    EcsCore, FrameClock, FrameId, InputEvent, InputSnapshot, RenderFrame, DEFAULT_INPUT_DT_SECONDS,
};
use crate::errors::{BridgeError, FrameError, ShutdownError, StartError};
use crate::ports::{InputPort, OutputPort, RenderFrameBuffer};
use crate::xr::XrRuntime;
use tracing::{info, instrument, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuntimeMode {
    Vr,
    #[default]
    Desktop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuntimeModePreference {
    #[default]
    Vr,
    Desktop,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StartDiagnostics {
    xr_failure_reason: Option<String>,
}

impl StartDiagnostics {
    pub fn xr_failure_reason(&self) -> Option<&str> {
        self.xr_failure_reason.as_deref()
    }
}

pub trait RuntimeBridge {
    fn on_start(&mut self) -> Result<(), BridgeError>;
    fn on_shutdown(&mut self) -> Result<(), BridgeError>;
    fn on_frame(&mut self, input: InputSnapshot) -> Result<RenderFrame, BridgeError>;
}

pub trait ClientLifecycle {
    fn start(&mut self) -> Result<(), StartError>;
    fn shutdown(&mut self) -> Result<(), ShutdownError>;
}

pub struct ClientBootstrap<X: XrRuntime, B: RuntimeBridge> {
    pub(crate) xr: X,
    pub(crate) bridge: B,
    mode_preference: RuntimeModePreference,
    active_mode: RuntimeMode,
    start_diagnostics: StartDiagnostics,
    running: bool,
    frame_clock: FrameClock,
}

impl<X: XrRuntime, B: RuntimeBridge> ClientBootstrap<X, B> {
    pub fn new(xr: X, bridge: B) -> Self {
        Self::new_with_mode(xr, bridge, RuntimeModePreference::Vr)
    }

    pub fn new_with_mode(xr: X, bridge: B, mode_preference: RuntimeModePreference) -> Self {
        Self {
            xr,
            bridge,
            mode_preference,
            active_mode: RuntimeMode::Desktop,
            start_diagnostics: StartDiagnostics::default(),
            running: false,
            frame_clock: FrameClock::default(),
        }
    }

    #[instrument(skip(self), fields(running_before = self.running))]
    pub fn start(&mut self) -> Result<(), StartError> {
        info!("client bootstrap start requested");
        self.start_diagnostics = StartDiagnostics::default();
        self.active_mode = self.resolve_runtime_mode();
        self.bridge.on_start().map_err(StartError::BridgeInit)?;
        self.frame_clock.reset(FrameId(0));
        self.running = true;
        info!(
            running = self.running,
            mode = ?self.active_mode,
            "client bootstrap started"
        );
        Ok(())
    }

    pub fn runtime_mode(&self) -> RuntimeMode {
        self.active_mode
    }

    pub fn start_diagnostics(&self) -> &StartDiagnostics {
        &self.start_diagnostics
    }

    fn resolve_runtime_mode(&mut self) -> RuntimeMode {
        match self.mode_preference {
            RuntimeModePreference::Desktop => RuntimeMode::Desktop,
            RuntimeModePreference::Vr => match self.xr.enable() {
                Ok(()) if self.xr.is_ready() => RuntimeMode::Vr,
                Ok(()) => {
                    let reason = "xr runtime not ready".to_string();
                    self.start_diagnostics.xr_failure_reason = Some(reason.clone());
                    warn!(reason = %reason, "falling back to desktop mode");
                    RuntimeMode::Desktop
                }
                Err(error) => {
                    let reason = error.to_string();
                    self.start_diagnostics.xr_failure_reason = Some(reason.clone());
                    warn!(reason = %reason, "falling back to desktop mode");
                    RuntimeMode::Desktop
                }
            },
        }
    }

    /// Advances the frame clock and returns the new frame id.
    pub fn tick_frame(&mut self) -> Result<FrameId, FrameError> {
        if !self.running {
            return Err(FrameError::NotRunning);
        }
        Ok(self.frame_clock.next_frame())
    }

    /// Uses the current frame id from `tick_frame` without advancing the clock.
    pub fn tick(&mut self, inputs: Vec<InputEvent>) -> Result<RenderFrame, FrameError> {
        if !self.running {
            return Err(FrameError::NotRunning);
        }
        let input = InputSnapshot {
            frame: self.frame_clock.current_frame(),
            dt_seconds: DEFAULT_INPUT_DT_SECONDS,
            inputs,
        };
        self.bridge.on_frame(input).map_err(FrameError::Bridge)
    }
}

impl<X: XrRuntime, B: RuntimeBridge> ClientLifecycle for ClientBootstrap<X, B> {
    fn start(&mut self) -> Result<(), StartError> {
        ClientBootstrap::start(self)
    }

    #[instrument(skip(self), fields(running_before = self.running))]
    fn shutdown(&mut self) -> Result<(), ShutdownError> {
        info!("client shutdown requested");
        if !self.running {
            info!("client shutdown completed (already stopped)");
            return Ok(());
        }

        let bridge_err = self
            .bridge
            .on_shutdown()
            .err()
            .map(ShutdownError::BridgeShutdown);
        let xr_err = self.xr.shutdown().err().map(ShutdownError::XrShutdown);

        let bridge_failed = bridge_err.is_some();
        let xr_failed = xr_err.is_some();
        let shutdown_err = match (bridge_err, xr_err) {
            (Some(bridge_err), Some(xr_err)) => {
                warn!(
                    bridge_error = %bridge_err,
                    ?bridge_err,
                    xr_error = %xr_err,
                    ?xr_err,
                    "both shutdown operations failed; returning bridge error and discarding xr error"
                );
                Some(bridge_err)
            }
            (Some(bridge_err), None) => Some(bridge_err),
            (None, Some(xr_err)) => Some(xr_err),
            (None, None) => None,
        };

        self.running = false;
        self.active_mode = RuntimeMode::Desktop;
        info!(
            running = self.running,
            bridge_failed,
            xr_failed,
            has_error = shutdown_err.is_some(),
            "client shutdown completed"
        );

        if let Some(err) = shutdown_err {
            return Err(err);
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StateOverrideRequest {
    pub reason: String,
}

pub trait StateOverride {
    fn request_state_override(&mut self, request: StateOverrideRequest) -> Result<(), BridgeError>;
}

pub struct RuntimeBridgeAdapter<C: EcsCore> {
    pub(crate) core: C,
    started: bool,
}

impl<C: EcsCore> RuntimeBridgeAdapter<C> {
    pub fn new(core: C) -> Self {
        Self {
            core,
            started: false,
        }
    }

    pub fn request_state_override(
        &mut self,
        request: StateOverrideRequest,
    ) -> Result<(), BridgeError> {
        warn!(
            reason = %request.reason,
            started = self.started,
            "state override request denied"
        );
        Err(BridgeError::DirectStateMutationDenied)
    }
}

impl<C: EcsCore> StateOverride for RuntimeBridgeAdapter<C> {
    fn request_state_override(&mut self, request: StateOverrideRequest) -> Result<(), BridgeError> {
        RuntimeBridgeAdapter::request_state_override(self, request)
    }
}

impl<C: EcsCore> RuntimeBridge for RuntimeBridgeAdapter<C> {
    fn on_start(&mut self) -> Result<(), BridgeError> {
        self.core.init_world().map_err(BridgeError::CoreInit)?;
        self.started = true;
        Ok(())
    }

    fn on_shutdown(&mut self) -> Result<(), BridgeError> {
        self.started = false;
        Ok(())
    }

    #[instrument(
        skip(self, input),
        fields(frame = ?input.frame, input_events = input.inputs.len(), started = self.started)
    )]
    fn on_frame(&mut self, input: InputSnapshot) -> Result<RenderFrame, BridgeError> {
        info!("runtime bridge frame processing started");
        if !self.started {
            return Err(BridgeError::NotStarted);
        }
        let frame = self.core.tick(input).map_err(BridgeError::Core)?;
        info!("runtime bridge frame processing completed");
        Ok(frame)
    }
}

pub struct BridgePipeline<B: RuntimeBridge, O: OutputPort> {
    bridge: B,
    output: O,
}

impl<B: RuntimeBridge, O: OutputPort> BridgePipeline<B, O> {
    pub fn new(bridge: B, output: O) -> Self {
        Self { bridge, output }
    }

    pub fn on_start(&mut self) -> Result<(), BridgeError> {
        self.bridge.on_start()
    }

    pub fn on_shutdown(&mut self) -> Result<(), BridgeError> {
        self.bridge.on_shutdown()
    }

    pub fn on_frame(&mut self, input: InputSnapshot) -> Result<(), BridgeError> {
        let frame = self.bridge.on_frame(input)?;
        self.output.project(frame);
        Ok(())
    }

    pub fn on_port_input<P: InputPort>(
        &mut self,
        frame_clock: &mut FrameClock,
        input_port: &mut P,
    ) -> Result<(), BridgeError> {
        let input = input_port.snapshot(frame_clock);
        self.on_frame(input)
    }
}

impl<B: RuntimeBridge + StateOverride, O: OutputPort> BridgePipeline<B, O> {
    pub fn request_state_override(
        &mut self,
        request: StateOverrideRequest,
    ) -> Result<(), BridgeError> {
        self.bridge.request_state_override(request)
    }
}

impl<B: RuntimeBridge> BridgePipeline<B, RenderFrameBuffer> {
    pub fn last_frame(&self) -> Option<&RenderFrame> {
        self.output.last()
    }
}
