use godot::classes::{InputEvent as GodotInputEvent, Node3D};
use godot::prelude::*;

use crate::render::RenderStateProjector;
use client_domain::ecs::{FrameClock, InputEvent, InputSnapshot, RenderFrame};
use client_domain::ports::InputPort;

#[derive(Default)]
pub struct GodotInputPort {
    events: Vec<Gd<GodotInputEvent>>,
}

pub(crate) fn map_event_slots_to_input_events(event_count: usize) -> Vec<InputEvent> {
    (0..event_count)
        .map(|index| match index % 3 {
            0 => InputEvent::Move {
                axis_x: 0.0,
                axis_y: 0.0,
            },
            1 => InputEvent::Look {
                yaw_delta: 0.0,
                pitch_delta: 0.0,
            },
            _ => InputEvent::Action {
                name: "godot_input".to_string(),
                pressed: true,
            },
        })
        .collect()
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
        let inputs = map_event_slots_to_input_events(self.events.len());

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
