use godot::classes::{InputEvent as GodotInputEvent, Node3D};
use godot::prelude::*;
use tracing::error;

use crate::render::{ProjectionError, RenderStateProjector};
use client_domain::ecs::{FrameClock, InputEvent, InputSnapshot, RenderFrame};
use client_domain::ports::{InputPort, OutputPort};

#[derive(Default)]
pub struct GodotInputPort {
    events: Vec<Gd<GodotInputEvent>>,
}

/// Temporary placeholder mapping.
///
/// TODO(godot-adapter): Replace this with real conversion from each incoming
/// `Gd<GodotInputEvent>` into domain `InputEvent` values:
/// - map motion/axis values to `InputEvent::Move { axis_x, axis_y }`
/// - map look deltas to `InputEvent::Look { yaw_delta, pitch_delta }`
/// - map button/action fields to `InputEvent::Action { name, pressed }`
///
/// For now this function intentionally returns placeholder events based only on
/// `event_count` so the input pipeline can be exercised end-to-end.
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
        // TODO(godot-adapter): Use actual Godot event payload once
        // map_event_slots_to_input_events supports concrete event parsing.
        let events = std::mem::take(&mut self.events);
        let inputs = map_event_slots_to_input_events(events.len());

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

    pub fn apply(&mut self, frame: &RenderFrame) -> Result<(), ProjectionError> {
        self.projector.project(frame, self.target)
    }
}

impl<'a> OutputPort for GodotOutputPort<'a> {
    fn project(&mut self, frame: RenderFrame) {
        if let Err(err) = self.apply(&frame) {
            error!(
                target: "godot_adapter",
                frame_id = ?frame.frame,
                error = %err,
                "failed to project render frame"
            );
        }
    }
}
