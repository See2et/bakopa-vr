use godot::prelude::{Basis, Vector3};

use super::ports::GodotInputPort;
use super::render::tests_support;
use crate::core::ecs::{FrameClock, FrameId, InputEvent, Pose, RenderFrame, UnitQuat, Vec3};
use crate::core::ports::InputPort;

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
    assert_eq!(snapshot.inputs, vec![InputEvent::Noop]);
}
