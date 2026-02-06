use godot::prelude::{Basis, Vector3};

use super::ports::{map_event_slots_to_input_events, GodotInputPort};
use super::render::tests_support;
use super::render::RenderStateProjector;
use client_domain::ecs::{FrameClock, FrameId, InputEvent, Pose, RenderFrame, UnitQuat, Vec3};
use client_domain::ports::InputPort;

#[test]
fn pose_to_transform3d_maps_translation_and_rotation() {
    let pose = Pose {
        position: Vec3 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        },
        orientation: UnitQuat::identity(),
    };

    let transform = tests_support::transform_from_pose(&pose);

    assert_eq!(transform.origin, Vector3::new(1.0, 2.0, 3.0));
    assert_eq!(transform.basis, Basis::IDENTITY);
}

#[test]
fn render_frame_transform_uses_primary_pose() {
    let pose = Pose {
        position: Vec3 {
            x: 0.2,
            y: 0.4,
            z: 0.6,
        },
        orientation: UnitQuat::identity(),
    };
    let frame = RenderFrame::from_primary_pose(FrameId(1), pose);

    let transform = tests_support::transform_from_frame(&frame);

    assert_eq!(transform.origin, Vector3::new(0.2, 0.4, 0.6));
}

#[test]
fn godot_input_port_empty_maps_to_noop_snapshot() {
    let mut port = GodotInputPort::empty();
    let mut clock = FrameClock::default();

    let snapshot = port.snapshot(&mut clock);

    assert_eq!(snapshot.frame, FrameId(1));
    assert!(snapshot.inputs.is_empty());
}

#[test]
fn event_slots_convert_to_domain_input_variants() {
    let inputs = map_event_slots_to_input_events(3);

    assert!(matches!(inputs[0], InputEvent::Move { .. }));
    assert!(matches!(inputs[1], InputEvent::Look { .. }));
    assert!(matches!(inputs[2], InputEvent::Action { .. }));
}

#[test]
fn render_state_projector_returns_false_for_invalid_target_node() {
    let mut projector = RenderStateProjector;
    let frame = RenderFrame::from_primary_pose(FrameId(1), Pose::identity());
    let mut target = godot::obj::OnEditor::<godot::obj::Gd<godot::classes::Node3D>>::default();

    assert!(!projector.project(&frame, &mut target));
}
