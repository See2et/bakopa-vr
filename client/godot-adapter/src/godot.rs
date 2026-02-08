use godot::classes::{INode, Node, Node3D};
use godot::prelude::*;
use tracing::{error, instrument, warn};

use crate::ports::{
    desktop_state_from_events, vr_state_from_events, GodotInputPort, GodotOutputPort,
};
use crate::render::RenderStateProjector;
use client_domain::bridge::{
    BridgePipeline, RuntimeBridgeWithSync, RuntimeMode, StateOverrideRequest,
};
use client_domain::ecs::{CoreEcs, FrameClock, DEFAULT_INPUT_DT_SECONDS};
use client_domain::errors::{BridgeError, BridgeErrorState};
use client_domain::ports::RenderFrameBuffer;
use client_domain::sync::SyncSessionAdapter;

/// Upper bound for buffered input events waiting to be consumed by `on_frame`.
const MAX_PENDING_INPUT_EVENTS: usize = 1024;

#[derive(GodotClass)]
#[class(base=Node)]
pub struct SuteraClientBridge {
    base: Base<Node>,
    pipeline: BridgePipeline<RuntimeBridgeWithSync<CoreEcs, SyncSessionAdapter>, RenderFrameBuffer>,
    frame_clock: FrameClock,
    error_state: BridgeErrorState,
    projector: RenderStateProjector,
    pending_input_events: Vec<Gd<godot::classes::InputEvent>>,
    runtime_mode: RuntimeMode,
    #[export]
    target_node: OnEditor<Gd<Node3D>>,
}

#[godot_api]
impl INode for SuteraClientBridge {
    fn init(base: Base<Node>) -> Self {
        let core = CoreEcs::new();
        let bridge = RuntimeBridgeWithSync::new(core, SyncSessionAdapter::new());
        Self {
            base,
            pipeline: BridgePipeline::new(bridge, RenderFrameBuffer::default()),
            frame_clock: FrameClock::default(),
            error_state: BridgeErrorState::default(),
            projector: RenderStateProjector::default(),
            pending_input_events: Vec::new(),
            runtime_mode: RuntimeMode::Desktop,
            target_node: OnEditor::default(),
        }
    }
}

#[godot_api]
impl SuteraClientBridge {
    #[func]
    #[instrument(skip(self), fields(pending_events = self.pending_input_events.len()))]
    pub fn on_start(&mut self) -> bool {
        match self.pipeline.on_start() {
            Ok(()) => true,
            Err(err) => {
                self.error_state.record(&err);
                error!(target: "godot_adapter", error = %err, "on_start failed");
                godot_error!("{err}");
                false
            }
        }
    }

    #[func]
    #[instrument(skip(self), fields(pending_events = self.pending_input_events.len()))]
    pub fn on_shutdown(&mut self) -> bool {
        match self.pipeline.on_shutdown() {
            Ok(()) => true,
            Err(err) => {
                self.error_state.record(&err);
                error!(target: "godot_adapter", error = %err, "on_shutdown failed");
                godot_error!("{err}");
                false
            }
        }
    }

    #[func]
    #[instrument(
        skip(self),
        fields(
            frame_before = self.frame_clock.current_frame().0,
            pending_events = self.pending_input_events.len()
        )
    )]
    pub fn on_frame(&mut self) -> bool {
        let events = std::mem::take(&mut self.pending_input_events);
        let mut input_port = match self.runtime_mode {
            RuntimeMode::Desktop => {
                let state = desktop_state_from_events(&events, DEFAULT_INPUT_DT_SECONDS);
                GodotInputPort::from_desktop_state_with_mode(state, self.runtime_mode)
            }
            RuntimeMode::Vr => {
                let state = vr_state_from_events(&events, DEFAULT_INPUT_DT_SECONDS);
                GodotInputPort::from_vr_state_with_mode(state, self.runtime_mode)
            }
        };
        match self
            .pipeline
            .on_port_input(&mut self.frame_clock, &mut input_port)
        {
            Ok(()) => match self.project_latest_frame() {
                Ok(()) => true,
                Err(err) => {
                    self.error_state.record(&err);
                    error!(
                        target: "godot_adapter",
                        error = %err,
                        "on_frame projection failed"
                    );
                    godot_error!("{err}");
                    false
                }
            },
            Err(err) => {
                self.error_state.record(&err);
                error!(target: "godot_adapter", error = %err, "on_frame failed");
                godot_error!("{err}");
                false
            }
        }
    }

    #[func]
    pub fn push_input_event(&mut self, event: Gd<godot::classes::InputEvent>) {
        if self.pending_input_events.len() >= MAX_PENDING_INPUT_EVENTS {
            self.pending_input_events.remove(0);
            warn!(
                target: "godot_adapter",
                max_pending_input_events = MAX_PENDING_INPUT_EVENTS,
                "pending input event buffer was full; dropped oldest event"
            );
        }
        self.pending_input_events.push(event);
    }

    fn project_latest_frame(&mut self) -> Result<(), BridgeError> {
        let frame = match self.pipeline.last_frame() {
            Some(frame) => frame,
            None => return Ok(()),
        };
        self.projector.set_runtime_mode(self.runtime_mode);
        let mut output = GodotOutputPort::new(&mut self.projector, &mut self.target_node);
        output
            .apply(frame)
            .map_err(|error| BridgeError::ProjectionFailed {
                reason: error.to_string(),
            })
    }

    #[func]
    pub fn last_error(&self) -> GString {
        GString::from(self.error_state.last_message().unwrap_or_default().as_str())
    }

    #[func]
    #[instrument(skip(self), fields(reason = %reason))]
    pub fn request_state_override(&mut self, reason: GString) -> bool {
        let request = StateOverrideRequest {
            reason: reason.to_string(),
        };
        match self.pipeline.request_state_override(request) {
            Ok(()) => true,
            Err(err) => {
                self.error_state.record(&err);
                error!(
                    target: "godot_adapter",
                    error = %err,
                    "request_state_override failed"
                );
                godot_error!("{err}");
                false
            }
        }
    }
}

struct SuteraClientCore;

#[gdextension]
unsafe impl ExtensionLibrary for SuteraClientCore {}
