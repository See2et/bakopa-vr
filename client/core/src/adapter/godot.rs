use godot::classes::{INode, Node, Node3D};
use godot::prelude::*;

use crate::adapter::ports::{GodotInputPort, GodotOutputPort};
use crate::adapter::render::RenderStateProjector;
use crate::core::bridge::{BridgePipeline, GodotBridgeAdapter, StateOverrideRequest};
use crate::core::ecs::{CoreEcs, FrameClock};
use crate::core::errors::BridgeErrorState;
use crate::core::ports::RenderFrameBuffer;

#[derive(GodotClass)]
#[class(base=Node)]
pub struct SuteraClientBridge {
    base: Base<Node>,
    pipeline: BridgePipeline<GodotBridgeAdapter<CoreEcs>, RenderFrameBuffer>,
    frame_clock: FrameClock,
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
            frame_clock: FrameClock::default(),
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
        let mut input_port = GodotInputPort::empty();
        match self
            .pipeline
            .on_port_input(&mut self.frame_clock, &mut input_port)
        {
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
        let mut output = GodotOutputPort::new(&mut self.projector, &mut self.target_node);
        output.apply(frame);
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
                godot_error!("{err}");
                false
            }
        }
    }
}

#[allow(dead_code)]
fn _hold_base_reference(bridge: &SuteraClientBridge) -> &Base<Node> {
    &bridge.base
}

struct SuteraClientCore;

#[gdextension]
unsafe impl ExtensionLibrary for SuteraClientCore {}
