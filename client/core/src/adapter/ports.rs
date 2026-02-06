use godot::classes::{InputEvent as GodotInputEvent, Node3D};
use godot::prelude::*;

use crate::adapter::render::RenderStateProjector;
use crate::core::ecs::{FrameClock, InputEvent, InputSnapshot, RenderFrame};
use crate::core::ports::InputPort;

#[derive(Default)]
pub struct GodotInputPort {
    events: Vec<Gd<GodotInputEvent>>,
}

impl GodotInputPort {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn from_events(events: Vec<Gd<GodotInputEvent>>) -> Self {
        Self { events }
    }
}

impl InputPort for GodotInputPort {
    fn snapshot(&mut self, frame_clock: &mut FrameClock) -> InputSnapshot {
        let inputs = if self.events.is_empty() {
            vec![InputEvent::Noop]
        } else {
            self.events.iter().map(|_event| InputEvent::Noop).collect()
        };

        InputSnapshot {
            frame: frame_clock.next_frame(),
            inputs,
        }
    }
}

pub struct GodotOutputPort<'a> {
    projector: &'a mut RenderStateProjector,
    target: &'a mut OnEditor<Gd<Node3D>>,
}

impl<'a> GodotOutputPort<'a> {
    pub fn new(
        projector: &'a mut RenderStateProjector,
        target: &'a mut OnEditor<Gd<Node3D>>,
    ) -> Self {
        Self { projector, target }
    }

    pub fn apply(&mut self, frame: &RenderFrame) -> bool {
        self.projector.project(frame, self.target)
    }
}
