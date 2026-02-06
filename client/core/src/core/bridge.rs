use crate::core::ecs::{EcsCore, FrameClock, FrameId, InputEvent, InputSnapshot, RenderFrame};
use crate::core::errors::{BridgeError, FrameError, ShutdownError, StartError};
use crate::core::ports::{InputPort, OutputPort, RenderFrameBuffer};
use crate::core::xr::XrRuntime;

pub trait GodotBridge {
    fn on_start(&mut self) -> Result<(), BridgeError>;
    fn on_shutdown(&mut self) -> Result<(), BridgeError>;
    fn on_frame(&mut self, input: InputSnapshot) -> Result<RenderFrame, BridgeError>;
}

pub trait ClientLifecycle {
    fn start(&mut self) -> Result<(), StartError>;
    fn shutdown(&mut self) -> Result<(), ShutdownError>;
}

pub struct ClientBootstrap<X: XrRuntime, B: GodotBridge> {
    pub(crate) xr: X,
    pub(crate) bridge: B,
    running: bool,
    frame_clock: FrameClock,
}

impl<X: XrRuntime, B: GodotBridge> ClientBootstrap<X, B> {
    pub fn new(xr: X, bridge: B) -> Self {
        Self {
            xr,
            bridge,
            running: false,
            frame_clock: FrameClock::default(),
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

#[derive(Debug, Clone, PartialEq)]
pub struct StateOverrideRequest {
    pub reason: String,
}

pub trait StateOverride {
    fn request_state_override(&mut self, request: StateOverrideRequest) -> Result<(), BridgeError>;
}

pub struct GodotBridgeAdapter<C: EcsCore> {
    pub(crate) core: C,
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

impl<C: EcsCore> StateOverride for GodotBridgeAdapter<C> {
    fn request_state_override(&mut self, request: StateOverrideRequest) -> Result<(), BridgeError> {
        GodotBridgeAdapter::request_state_override(self, request)
    }
}

impl<C: EcsCore> GodotBridge for GodotBridgeAdapter<C> {
    fn on_start(&mut self) -> Result<(), BridgeError> {
        self.core.init_world().map_err(BridgeError::CoreInit)?;
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

pub struct BridgePipeline<B: GodotBridge, O: OutputPort> {
    bridge: B,
    output: O,
}

impl<B: GodotBridge, O: OutputPort> BridgePipeline<B, O> {
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

impl<B: GodotBridge + StateOverride, O: OutputPort> BridgePipeline<B, O> {
    pub fn request_state_override(
        &mut self,
        request: StateOverrideRequest,
    ) -> Result<(), BridgeError> {
        self.bridge.request_state_override(request)
    }
}

impl<B: GodotBridge> BridgePipeline<B, RenderFrameBuffer> {
    pub fn last_frame(&self) -> Option<&RenderFrame> {
        self.output.last()
    }
}
