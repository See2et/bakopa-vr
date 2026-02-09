use std::fs;
use std::path::PathBuf;

use godot::prelude::{Basis, Quaternion, Vector3};

use super::ports::{
    desktop_snapshot_to_input_events, input_log_contract_fields, map_event_slots_to_input_events,
    normalize_desktop_input, normalize_vr_input, vr_state_from_action_samples, DesktopInputState,
    GodotInputPort, VrActionSample, VrInputState,
};
use super::render::RenderStateProjector;
use super::render::{projection_log_contract_fields, tests_support};
use client_domain::bridge::RuntimeMode;
use client_domain::ecs::{
    FrameClock, FrameId, InputEvent, Pose, RemoteRenderPose, RenderFrame, UnitQuat, Vec3,
};
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
fn pose_to_transform3d_maps_non_identity_rotation() {
    let half_turn_component = std::f32::consts::FRAC_1_SQRT_2;
    let pose = Pose {
        position: Vec3 {
            x: -1.5,
            y: 0.25,
            z: 9.0,
        },
        orientation: UnitQuat {
            x: 0.0,
            y: half_turn_component,
            z: 0.0,
            w: half_turn_component,
        },
    };

    let transform = tests_support::transform_from_pose(&pose);
    let expected_quat = Quaternion::new(
        pose.orientation.x,
        pose.orientation.y,
        pose.orientation.z,
        pose.orientation.w,
    );

    assert_eq!(transform.origin, Vector3::new(-1.5, 0.25, 9.0));
    assert_eq!(transform.basis, Basis::from_quaternion(expected_quat));
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
fn remote_pose_projection_follows_frame_updates_and_removals() {
    let first_remote_pose = Pose {
        position: Vec3 {
            x: 7.0,
            y: 1.0,
            z: -2.0,
        },
        orientation: UnitQuat::identity(),
    };
    let second_remote_pose = Pose {
        position: Vec3 {
            x: -4.0,
            y: 0.5,
            z: 3.0,
        },
        orientation: UnitQuat::identity(),
    };
    let frame_with_remote = RenderFrame::from_primary_pose(FrameId(1), Pose::identity())
        .with_remote_poses(vec![RemoteRenderPose {
            participant_id: "peer-projection".to_string(),
            pose: first_remote_pose,
        }]);
    let frame_after_update = RenderFrame::from_primary_pose(FrameId(2), Pose::identity())
        .with_remote_poses(vec![RemoteRenderPose {
            participant_id: "peer-projection".to_string(),
            pose: second_remote_pose,
        }]);
    let frame_after_left = RenderFrame::from_primary_pose(FrameId(3), Pose::identity());

    let first = tests_support::remote_cache_after_project(&frame_with_remote);
    let updated = tests_support::remote_cache_after_project(&frame_after_update);
    let removed = tests_support::remote_cache_after_project(&frame_after_left);

    assert_eq!(first.len(), 1);
    assert_eq!(first[0].0, "peer-projection".to_string());
    assert_eq!(first[0].1.origin, Vector3::new(7.0, 1.0, -2.0));

    assert_eq!(updated.len(), 1);
    assert_eq!(updated[0].0, "peer-projection".to_string());
    assert_eq!(updated[0].1.origin, Vector3::new(-4.0, 0.5, 3.0));

    assert!(removed.is_empty());
}

#[test]
fn godot_input_port_empty_maps_to_noop_snapshot() {
    let mut port = GodotInputPort::empty();
    let mut clock = FrameClock::default();

    let snapshot = port.snapshot(&mut clock);

    assert_eq!(snapshot.frame, FrameId(1));
    assert_eq!(snapshot.inputs.len(), 2);
    assert!(snapshot.dt_seconds > 0.0);
    assert_eq!(
        snapshot.inputs[0],
        InputEvent::Move {
            axis_x: 0.0,
            axis_y: 0.0
        }
    );
    assert_eq!(
        snapshot.inputs[1],
        InputEvent::Look {
            yaw_delta: 0.0,
            pitch_delta: 0.0
        }
    );
}

#[test]
fn event_slots_convert_to_domain_input_variants() {
    let inputs = map_event_slots_to_input_events(&[], 0.1);

    assert!(matches!(inputs[0], InputEvent::Move { .. }));
    assert!(matches!(inputs[1], InputEvent::Look { .. }));
    assert_eq!(inputs.len(), 2);
}

#[test]
fn desktop_input_normalization_maps_wasd_and_mouse_to_common_semantics() {
    let state = DesktopInputState {
        move_left: false,
        move_right: true,
        move_forward: true,
        move_back: false,
        mouse_delta_x: 0.6,
        mouse_delta_y: -0.3,
        dt_seconds: 0.2,
    };

    let normalized = normalize_desktop_input(state);
    let inputs = desktop_snapshot_to_input_events(normalized);

    assert!((normalized.move_axis_x - std::f32::consts::FRAC_1_SQRT_2).abs() < 1.0e-6);
    assert!((normalized.move_axis_y - std::f32::consts::FRAC_1_SQRT_2).abs() < 1.0e-6);
    assert_eq!(normalized.turn_yaw, 3.0);
    assert_eq!(normalized.look_pitch, -1.5);
    assert!(matches!(
        inputs[0],
        InputEvent::Move {
            axis_x: _,
            axis_y: _
        }
    ));
    assert!(matches!(
        inputs[1],
        InputEvent::Look {
            yaw_delta: _,
            pitch_delta: _
        }
    ));
}

#[test]
fn desktop_input_without_events_keeps_move_and_look_zero() {
    let mut port = GodotInputPort::from_desktop_state(DesktopInputState::default());
    let mut clock = FrameClock::default();

    let snapshot = port.snapshot(&mut clock);

    assert_eq!(snapshot.frame, FrameId(1));
    assert_eq!(snapshot.inputs.len(), 2);
    assert!(snapshot.dt_seconds > 0.0);
    assert_eq!(
        snapshot.inputs[0],
        InputEvent::Move {
            axis_x: 0.0,
            axis_y: 0.0
        }
    );
    assert_eq!(
        snapshot.inputs[1],
        InputEvent::Look {
            yaw_delta: 0.0,
            pitch_delta: 0.0
        }
    );
}

#[test]
fn desktop_input_invalid_dt_is_sanitized_without_panicking() {
    let state = DesktopInputState {
        mouse_delta_x: 2.0,
        dt_seconds: 0.0,
        ..DesktopInputState::default()
    };

    let normalized = normalize_desktop_input(state);

    assert!(normalized.dt_seconds > 0.0);
    assert!(normalized.turn_yaw.is_finite());
}

#[test]
fn vr_input_normalization_maps_controller_input_to_common_semantics() {
    let state = VrInputState {
        move_axis_x: 0.8,
        move_axis_y: 0.8,
        yaw_delta: 0.4,
        pitch_delta: -0.2,
        dt_seconds: 0.1,
    };

    let normalized = normalize_vr_input(state);
    let mut port = GodotInputPort::from_vr_state(state);
    let mut clock = FrameClock::default();
    let snapshot = port.snapshot(&mut clock);

    assert!((normalized.move_axis_x - std::f32::consts::FRAC_1_SQRT_2).abs() < 1.0e-6);
    assert!((normalized.move_axis_y - std::f32::consts::FRAC_1_SQRT_2).abs() < 1.0e-6);
    assert_eq!(normalized.turn_yaw, 4.0);
    assert_eq!(normalized.look_pitch, -2.0);
    assert_eq!(snapshot.dt_seconds, 0.1);
    assert_eq!(snapshot.inputs.len(), 2);
}

#[test]
fn vr_action_samples_are_mapped_to_vr_axes() {
    let samples = vec![
        VrActionSample {
            action: "vr_move_right".to_string(),
            strength: 0.75,
        },
        VrActionSample {
            action: "vr_move_forward".to_string(),
            strength: 0.5,
        },
        VrActionSample {
            action: "vr_turn_left".to_string(),
            strength: 0.4,
        },
        VrActionSample {
            action: "vr_look_up".to_string(),
            strength: 0.2,
        },
    ];

    let state = vr_state_from_action_samples(&samples, 0.05);

    assert_eq!(state.move_axis_x, 0.75);
    assert_eq!(state.move_axis_y, 0.5);
    assert_eq!(state.yaw_delta, -0.4);
    assert_eq!(state.pitch_delta, -0.2);
    assert_eq!(state.dt_seconds, 0.05);
}

#[test]
fn vr_input_failure_keeps_running_with_empty_inputs_and_logsafe_dt() {
    let mut port = GodotInputPort::from_vr_input_failure("xr input unavailable");
    let mut clock = FrameClock::default();

    let snapshot = port.snapshot(&mut clock);

    assert_eq!(snapshot.frame, FrameId(1));
    assert!(snapshot.inputs.is_empty());
    assert!(snapshot.dt_seconds > 0.0);
}

#[test]
fn common_validation_applies_to_desktop_and_vr_inputs() {
    let desktop = normalize_desktop_input(DesktopInputState {
        move_left: true,
        move_right: false,
        move_forward: false,
        move_back: true,
        mouse_delta_x: 1.0,
        mouse_delta_y: 1.0,
        dt_seconds: -1.0,
    });
    let vr = normalize_vr_input(VrInputState {
        move_axis_x: -2.0,
        move_axis_y: 3.0,
        yaw_delta: 1.0,
        pitch_delta: 1.0,
        dt_seconds: 0.0,
    });

    assert!(desktop.dt_seconds > 0.0);
    assert!(vr.dt_seconds > 0.0);
    assert!(desktop.turn_yaw.is_finite());
    assert!(vr.turn_yaw.is_finite());
    assert!(vr.move_axis_x.abs() <= 1.0);
    assert!(vr.move_axis_y.abs() <= 1.0);
}

#[test]
fn input_and_projection_log_contract_fields_are_stable() {
    let input_desktop = input_log_contract_fields(RuntimeMode::Desktop);
    let input_vr = input_log_contract_fields(RuntimeMode::Vr);
    let projection_desktop = projection_log_contract_fields(RuntimeMode::Desktop);
    let projection_vr = projection_log_contract_fields(RuntimeMode::Vr);

    assert_eq!(
        input_desktop,
        ("input", "unknown", "local", "pose", "desktop")
    );
    assert_eq!(input_vr, ("input", "unknown", "local", "pose", "vr"));
    assert_eq!(
        projection_desktop,
        ("projection", "unknown", "local", "pose", "desktop")
    );
    assert_eq!(
        projection_vr,
        ("projection", "unknown", "local", "pose", "vr")
    );
}

#[test]
fn render_state_projector_returns_error_for_invalid_target_node() {
    let mut projector = RenderStateProjector::default();
    let frame = RenderFrame::from_primary_pose(FrameId(1), Pose::identity());
    let mut target = godot::obj::OnEditor::<godot::obj::Gd<godot::classes::Node3D>>::default();

    assert!(projector.project(&frame, &mut target).is_err());
}

#[test]
fn godot_scene_wires_sutera_client_bridge_for_frame_and_input_flow() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../godot/node.tscn");
    let scene = fs::read_to_string(path).expect("node.tscn must be readable");

    assert!(scene.contains("[node name=\"SuteraClientBridge\" type=\"SuteraClientBridge\""));
    assert!(scene.contains("target_node = NodePath(\"../NearBox\")"));
}

#[test]
fn godot_project_input_map_defines_pose_sync_actions() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../godot/project.godot");
    let project = fs::read_to_string(path).expect("project.godot must be readable");

    for action in [
        "move_left",
        "move_right",
        "move_forward",
        "move_back",
        "look_left",
        "look_right",
        "look_up",
        "look_down",
    ] {
        assert!(
            project.contains(&format!("{action}={{")),
            "missing InputMap action: {action}"
        );
    }
}

#[test]
fn verify_script_emits_stageful_startup_diagnostics() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../godot/scripts/verify_gdextension.gd");
    let script = fs::read_to_string(path).expect("verify_gdextension.gd must be readable");

    for stage in [
        "\"extension_loaded\"",
        "\"openxr_init\"",
        "\"bridge_start\"",
    ] {
        assert!(
            script.contains(stage),
            "missing startup diagnostic stage marker: {stage}"
        );
    }

    assert!(
        script.contains("stage=%s mode=%s library_path=%s detail=%s"),
        "missing structured startup diagnostic format"
    );
}
