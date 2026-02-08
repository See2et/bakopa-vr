use godot::classes::{InputEvent as GodotInputEvent, Node3D};
use godot::prelude::*;
use tracing::{error, warn};

use crate::render::{ProjectionError, RenderStateProjector};
use client_domain::ecs::{
    FrameClock, InputEvent, InputSnapshot, RenderFrame, DEFAULT_INPUT_DT_SECONDS,
};
use client_domain::ports::{InputPort, OutputPort};

const MIN_FRAME_DT_SECONDS: f32 = 1.0 / 240.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DesktopInputState {
    pub move_left: bool,
    pub move_right: bool,
    pub move_forward: bool,
    pub move_back: bool,
    pub mouse_delta_x: f32,
    pub mouse_delta_y: f32,
    pub dt_seconds: f32,
}

impl Default for DesktopInputState {
    fn default() -> Self {
        Self {
            move_left: false,
            move_right: false,
            move_forward: false,
            move_back: false,
            mouse_delta_x: 0.0,
            mouse_delta_y: 0.0,
            dt_seconds: MIN_FRAME_DT_SECONDS,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DesktopInputSnapshot {
    pub move_axis_x: f32,
    pub move_axis_y: f32,
    pub turn_yaw: f32,
    pub look_pitch: f32,
    pub dt_seconds: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VrInputState {
    pub move_axis_x: f32,
    pub move_axis_y: f32,
    pub yaw_delta: f32,
    pub pitch_delta: f32,
    pub dt_seconds: f32,
}

#[derive(Default)]
pub struct GodotInputPort {
    events: Vec<Gd<GodotInputEvent>>,
    desktop_state: Option<DesktopInputState>,
    vr_state: Option<VrInputState>,
    vr_input_failure_reason: Option<String>,
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

pub(crate) fn normalize_desktop_input(state: DesktopInputState) -> DesktopInputSnapshot {
    normalize_input_intent(
        bool_axis(state.move_right, state.move_left),
        bool_axis(state.move_forward, state.move_back),
        state.mouse_delta_x,
        state.mouse_delta_y,
        state.dt_seconds,
    )
}

pub(crate) fn normalize_vr_input(state: VrInputState) -> DesktopInputSnapshot {
    normalize_input_intent(
        state.move_axis_x,
        state.move_axis_y,
        state.yaw_delta,
        state.pitch_delta,
        state.dt_seconds,
    )
}

pub(crate) fn desktop_snapshot_to_input_events(snapshot: DesktopInputSnapshot) -> Vec<InputEvent> {
    vec![
        InputEvent::Move {
            axis_x: snapshot.move_axis_x,
            axis_y: snapshot.move_axis_y,
        },
        InputEvent::Look {
            yaw_delta: snapshot.turn_yaw,
            pitch_delta: snapshot.look_pitch,
        },
    ]
}

fn bool_axis(positive: bool, negative: bool) -> f32 {
    match (positive, negative) {
        (true, false) => 1.0,
        (false, true) => -1.0,
        _ => 0.0,
    }
}

fn sanitize_dt(dt_seconds: f32) -> f32 {
    if dt_seconds.is_finite() && dt_seconds > 0.0 {
        dt_seconds
    } else {
        MIN_FRAME_DT_SECONDS
    }
}

fn normalize_input_intent(
    raw_axis_x: f32,
    raw_axis_y: f32,
    yaw_delta: f32,
    pitch_delta: f32,
    dt_seconds: f32,
) -> DesktopInputSnapshot {
    let mut axis_x = raw_axis_x.clamp(-1.0, 1.0);
    let mut axis_y = raw_axis_y.clamp(-1.0, 1.0);
    let magnitude = (axis_x * axis_x + axis_y * axis_y).sqrt();
    if magnitude > 1.0 {
        axis_x /= magnitude;
        axis_y /= magnitude;
    }

    let dt_seconds = sanitize_dt(dt_seconds);

    DesktopInputSnapshot {
        move_axis_x: axis_x,
        move_axis_y: axis_y,
        turn_yaw: yaw_delta / dt_seconds,
        look_pitch: pitch_delta / dt_seconds,
        dt_seconds,
    }
}

impl GodotInputPort {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn from_events(events: Vec<Gd<GodotInputEvent>>) -> Self {
        Self {
            events,
            desktop_state: None,
            vr_state: None,
            vr_input_failure_reason: None,
        }
    }

    pub fn from_desktop_state(state: DesktopInputState) -> Self {
        Self {
            events: Vec::new(),
            desktop_state: Some(state),
            vr_state: None,
            vr_input_failure_reason: None,
        }
    }

    pub fn from_vr_state(state: VrInputState) -> Self {
        Self {
            events: Vec::new(),
            desktop_state: None,
            vr_state: Some(state),
            vr_input_failure_reason: None,
        }
    }

    pub fn from_vr_input_failure(reason: impl Into<String>) -> Self {
        Self {
            events: Vec::new(),
            desktop_state: None,
            vr_state: None,
            vr_input_failure_reason: Some(reason.into()),
        }
    }
}

impl InputPort for GodotInputPort {
    fn snapshot(&mut self, frame_clock: &mut FrameClock) -> InputSnapshot {
        let (inputs, dt_seconds) = if let Some(reason) = self.vr_input_failure_reason.take() {
            warn!(
                target: "godot_adapter",
                reason = %reason,
                "failed to capture vr input; continuing with empty input snapshot"
            );
            (Vec::new(), DEFAULT_INPUT_DT_SECONDS)
        } else if let Some(state) = self.vr_state.take() {
            let normalized = normalize_vr_input(state);
            let dt_seconds = normalized.dt_seconds;
            let inputs = desktop_snapshot_to_input_events(normalized);
            (inputs, dt_seconds)
        } else if let Some(state) = self.desktop_state.take() {
            let normalized = normalize_desktop_input(state);
            let dt_seconds = normalized.dt_seconds;
            let inputs = desktop_snapshot_to_input_events(normalized);
            (inputs, dt_seconds)
        } else {
            // TODO(godot-adapter): Use actual Godot event payload once
            // map_event_slots_to_input_events supports concrete event parsing.
            let events = std::mem::take(&mut self.events);
            (
                map_event_slots_to_input_events(events.len()),
                DEFAULT_INPUT_DT_SECONDS,
            )
        };

        InputSnapshot {
            frame: frame_clock.next_frame(),
            dt_seconds,
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
