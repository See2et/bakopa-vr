use godot::classes::{INode, Node, Node3D};
use godot::prelude::*;
use tracing::error;

use crate::ports::{GodotInputPort, GodotOutputPort};
use crate::render::RenderStateProjector;
use client_domain::bridge::{BridgePipeline, RuntimeBridgeAdapter, StateOverrideRequest};
use client_domain::ecs::{CoreEcs, FrameClock};
use client_domain::errors::{BridgeError, BridgeErrorState};
use client_domain::ports::RenderFrameBuffer;

#[derive(GodotClass)]
#[class(base=Node)]
pub struct SuteraClientBridge {
    base: Base<Node>,
    pipeline: BridgePipeline<RuntimeBridgeAdapter<CoreEcs>, RenderFrameBuffer>,
    frame_clock: FrameClock,
    error_state: BridgeErrorState,
    projector: RenderStateProjector,
    pending_input_events: Vec<Gd<godot::classes::InputEvent>>,
    #[export]
    target_node: OnEditor<Gd<Node3D>>,
}

#[godot_api]
impl INode for SuteraClientBridge {
    fn init(base: Base<Node>) -> Self {
        let core = CoreEcs::new();
        let bridge = RuntimeBridgeAdapter::new(core);
        Self {
            base,
            pipeline: BridgePipeline::new(bridge, RenderFrameBuffer::default()),
            frame_clock: FrameClock::default(),
            error_state: BridgeErrorState::default(),
            projector: RenderStateProjector,
            pending_input_events: Vec::new(),
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
                error!(target: "godot_adapter", error = %err, "on_start failed");
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
                error!(target: "godot_adapter", error = %err, "on_shutdown failed");
                godot_error!("{err}");
                false
            }
        }
    }

    #[func]
    pub fn on_frame(&mut self) -> bool {
        let mut input_port =
            GodotInputPort::from_events(std::mem::take(&mut self.pending_input_events));
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
        self.pending_input_events.push(event);
    }

    fn project_latest_frame(&mut self) -> Result<(), BridgeError> {
        let frame = match self.pipeline.last_frame() {
            Some(frame) => frame,
            None => return Ok(()),
        };
        let mut output = GodotOutputPort::new(&mut self.projector, &mut self.target_node);
        if output.apply(frame) {
            Ok(())
        } else {
            let err = BridgeError::ProjectionFailed {
                reason: "target node is invalid".to_string(),
            };
            Err(err)
        }
    }

    #[func]
    pub fn last_error(&self) -> GString {
        GString::from(self.error_state.last_message().unwrap_or_default().as_str())
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
