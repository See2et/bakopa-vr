use syncer::{StreamKind, TransportSendParams};

#[test]
fn pose_uses_unordered_unreliable_data_channel() {
    let params = TransportSendParams::for_stream(StreamKind::Pose);

    match params {
        TransportSendParams::DataChannel {
            ordered,
            reliable,
            label,
        } => {
            assert!(!ordered, "Pose should be unordered");
            assert!(!reliable, "Pose should be unreliable");
            assert_eq!(label, "sutera-data");
        }
        _ => panic!("Pose should use data channel params"),
    }
}

#[test]
fn chat_uses_ordered_reliable_data_channel() {
    let params = TransportSendParams::for_stream(StreamKind::Chat);

    match params {
        TransportSendParams::DataChannel {
            ordered,
            reliable,
            label,
        } => {
            assert!(ordered, "Chat should be ordered");
            assert!(reliable, "Chat should be reliable");
            assert_eq!(label, "sutera-data");
        }
        _ => panic!("Chat should use data channel params"),
    }
}

#[test]
fn control_join_uses_ordered_reliable_data_channel() {
    let params = TransportSendParams::for_stream(StreamKind::ControlJoin);

    match params {
        TransportSendParams::DataChannel {
            ordered,
            reliable,
            label,
        } => {
            assert!(ordered, "ControlJoin should be ordered");
            assert!(reliable, "ControlJoin should be reliable");
            assert_eq!(label, "sutera-data");
        }
        _ => panic!("ControlJoin should use data channel params"),
    }
}

#[test]
fn control_leave_uses_ordered_reliable_data_channel() {
    let params = TransportSendParams::for_stream(StreamKind::ControlLeave);

    match params {
        TransportSendParams::DataChannel {
            ordered,
            reliable,
            label,
        } => {
            assert!(ordered, "ControlLeave should be ordered");
            assert!(reliable, "ControlLeave should be reliable");
            assert_eq!(label, "sutera-data");
        }
        _ => panic!("ControlLeave should use data channel params"),
    }
}

#[test]
fn voice_uses_audio_track() {
    let params = TransportSendParams::for_stream(StreamKind::Voice);

    match params {
        TransportSendParams::AudioTrack => {}
        _ => panic!("Voice should use audio track params"),
    }
}
