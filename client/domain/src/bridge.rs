use crate::ecs::{EcsCore, FrameClock, FrameId, InputEvent, InputSnapshot, RenderFrame};
use crate::errors::{BridgeError, FrameError, ShutdownError, StartError};
use crate::ports::{InputPort, OutputPort, RenderFrameBuffer};
use crate::xr::XrRuntime;
use tracing::{info, instrument};

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
    running: bool,
    frame_clock: FrameClock,
}

impl<X: XrRuntime, B: RuntimeBridge> ClientBootstrap<X, B> {
    pub fn new(xr: X, bridge: B) -> Self {
        Self {
            xr,
            bridge,
            running: false,
            frame_clock: FrameClock::default(),
        }
    }

    #[instrument(skip(self), fields(running_before = self.running))]
    pub fn start(&mut self) -> Result<(), StartError> {
        info!("client bootstrap start requested");
        self.xr.enable().map_err(StartError::XrInit)?;
        if !self.xr.is_ready() {
            return Err(StartError::XrNotReady);
        }
        self.bridge.on_start().map_err(StartError::BridgeInit)?;
        self.running = true;
        info!(running = self.running, "client bootstrap started");
        Ok(())
    }

    pub fn tick_frame(&mut self) -> Result<FrameId, FrameError> {
        if !self.running {
            return Err(FrameError::NotRunning);
        }
        Ok(self.frame_clock.next_frame())
    }

    pub fn tick(&mut self, inputs: Vec<InputEvent>) -> Result<RenderFrame, FrameError> {
        if !self.running {
            return Err(FrameError::NotRunning);
        }
        let input = InputSnapshot {
            frame: self.frame_clock.next_frame(),
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

        if let Some(err) = bridge_err.or(xr_err) {
            return Err(err);
        }

        self.running = false;
        info!(running = self.running, "client shutdown completed");
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
        _request: StateOverrideRequest,
    ) -> Result<(), BridgeError> {
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
