use godot::classes::{InputEvent as GodotInputEvent, InputEventAction, Node3D};
use godot::prelude::*;
use tracing::{error, info, warn};

use crate::render::{ProjectionError, RenderStateProjector};
use client_domain::bridge::RuntimeMode;
use client_domain::ecs::{
    FrameClock, InputEvent, InputSnapshot, RenderFrame, DEFAULT_INPUT_DT_SECONDS,
};
use client_domain::ports::{InputPort, OutputPort};
use client_domain::sync::runtime_mode_label;

const MIN_FRAME_DT_SECONDS: f32 = 1.0 / 240.0;

pub(crate) fn input_log_contract_fields(
    mode: RuntimeMode,
) -> (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
) {
    (
        "input",
        "unknown",
        "local",
        "pose",
        runtime_mode_label(mode),
    )
}

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
    mode: RuntimeMode,
}

fn apply_desktop_action_state(event: &Gd<GodotInputEvent>, state: &mut DesktopInputState) {
    let move_left = StringName::from("move_left");
    let move_right = StringName::from("move_right");
    let move_forward = StringName::from("move_forward");
    let move_back = StringName::from("move_back");
    let look_left = StringName::from("look_left");
    let look_right = StringName::from("look_right");
    let look_up = StringName::from("look_up");
    let look_down = StringName::from("look_down");

    if event.is_action_pressed_ex(&move_left).done() {
        state.move_left = true;
    }
    if event.is_action_released_ex(&move_left).done() {
        state.move_left = false;
    }
    if event.is_action_pressed_ex(&move_right).done() {
        state.move_right = true;
    }
    if event.is_action_released_ex(&move_right).done() {
        state.move_right = false;
    }
    if event.is_action_pressed_ex(&move_forward).done() {
        state.move_forward = true;
    }
    if event.is_action_released_ex(&move_forward).done() {
        state.move_forward = false;
    }
    if event.is_action_pressed_ex(&move_back).done() {
        state.move_back = true;
    }
    if event.is_action_released_ex(&move_back).done() {
        state.move_back = false;
    }
    if event.is_action_pressed_ex(&look_left).done() {
        state.mouse_delta_x -= 1.0;
    }
    if event.is_action_pressed_ex(&look_right).done() {
        state.mouse_delta_x += 1.0;
    }
    if event.is_action_pressed_ex(&look_up).done() {
        state.mouse_delta_y -= 1.0;
    }
    if event.is_action_pressed_ex(&look_down).done() {
        state.mouse_delta_y += 1.0;
    }
}

fn extract_action_event(event: &Gd<GodotInputEvent>) -> Option<InputEvent> {
    let action = event.clone().try_cast::<InputEventAction>().ok()?;
    Some(InputEvent::Action {
        name: action.get_action().to_string(),
        pressed: action.is_pressed(),
    })
}

pub(crate) fn desktop_state_from_events(
    events: &[Gd<GodotInputEvent>],
    dt_seconds: f32,
) -> DesktopInputState {
    let mut state = DesktopInputState {
        dt_seconds,
        ..DesktopInputState::default()
    };
    for event in events {
        apply_desktop_action_state(event, &mut state);
    }
    state
}

pub(crate) fn map_event_slots_to_input_events(
    events: &[Gd<GodotInputEvent>],
    dt_seconds: f32,
) -> Vec<InputEvent> {
    let normalized = normalize_desktop_input(desktop_state_from_events(events, dt_seconds));
    let mut inputs = desktop_snapshot_to_input_events(normalized);
    inputs.extend(events.iter().filter_map(extract_action_event));
    inputs
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
        Self::from_events_with_mode(events, RuntimeMode::Desktop)
    }

    pub fn from_events_with_mode(events: Vec<Gd<GodotInputEvent>>, mode: RuntimeMode) -> Self {
        Self {
            events,
            desktop_state: None,
            vr_state: None,
            vr_input_failure_reason: None,
            mode,
        }
    }

    pub fn from_desktop_state(state: DesktopInputState) -> Self {
        Self::from_desktop_state_with_mode(state, RuntimeMode::Desktop)
    }

    pub fn from_desktop_state_with_mode(state: DesktopInputState, mode: RuntimeMode) -> Self {
        Self {
            events: Vec::new(),
            desktop_state: Some(state),
            vr_state: None,
            vr_input_failure_reason: None,
            mode,
        }
    }

    pub fn from_vr_state(state: VrInputState) -> Self {
        Self::from_vr_state_with_mode(state, RuntimeMode::Vr)
    }

    pub fn from_vr_state_with_mode(state: VrInputState, mode: RuntimeMode) -> Self {
        Self {
            events: Vec::new(),
            desktop_state: None,
            vr_state: Some(state),
            vr_input_failure_reason: None,
            mode,
        }
    }

    pub fn from_vr_input_failure(reason: impl Into<String>) -> Self {
        Self::from_vr_input_failure_with_mode(reason, RuntimeMode::Vr)
    }

    pub fn from_vr_input_failure_with_mode(reason: impl Into<String>, mode: RuntimeMode) -> Self {
        Self {
            events: Vec::new(),
            desktop_state: None,
            vr_state: None,
            vr_input_failure_reason: Some(reason.into()),
            mode,
        }
    }
}

impl InputPort for GodotInputPort {
    fn snapshot(&mut self, frame_clock: &mut FrameClock) -> InputSnapshot {
        let (stage, room_id, participant_id, stream_kind, mode) =
            input_log_contract_fields(self.mode);
        let (inputs, dt_seconds) = if let Some(reason) = self.vr_input_failure_reason.take() {
            warn!(
                target: "godot_adapter",
                stage,
                room_id,
                participant_id,
                stream_kind,
                mode,
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
            let events = std::mem::take(&mut self.events);
            (
                map_event_slots_to_input_events(&events, DEFAULT_INPUT_DT_SECONDS),
                DEFAULT_INPUT_DT_SECONDS,
            )
        };

        info!(
            target: "godot_adapter",
            stage,
            room_id,
            participant_id,
            stream_kind,
            mode,
            dt_seconds,
            input_events = inputs.len(),
            "normalized input snapshot"
        );

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
