//! Bloom signaling protocol types.

pub mod errors;
pub mod events;
pub mod payload;
pub mod requests;

pub use errors::ErrorCode;
pub use events::ServerToClient;
pub use payload::{RelayIce, RelaySdp};
pub use requests::ClientToServer;

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::DeserializeOwned;
    use serde::Serialize;

    const ROOM_ID: &str = "room-1";
    const SELF_ID: &str = "self-1";
    const PEER_A: &str = "peer-a";
    const PEER_B: &str = "peer-b";
    const SDP_OFFER: &str = "v=0 offer";
    const SDP_ANSWER: &str = "v=0 answer";
    const CANDIDATE: &str = "cand1";

    fn assert_roundtrip<T>(value: T, expected_json: &str)
    where
        T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
    {
        let json = serde_json::to_string(&value).expect("serialize");
        assert_eq!(json, expected_json);
        let back: T = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, value);
    }

    mod client_to_server {
        use super::*;

        #[test]
        fn create_room_roundtrip() {
            assert_roundtrip(ClientToServer::CreateRoom, r#"{"type":"CreateRoom"}"#);
        }

        #[test]
        fn join_room_roundtrip_and_missing_room_id_errors() {
            assert_roundtrip(
                ClientToServer::JoinRoom {
                    room_id: ROOM_ID.into(),
                },
                r#"{"type":"JoinRoom","room_id":"room-1"}"#,
            );

            let json = r#"{"type":"JoinRoom"}"#;
            let result: Result<ClientToServer, _> = serde_json::from_str(json);
            assert!(result.is_err(), "room_id欠落はエラー");
        }

        #[test]
        fn leave_room_roundtrip() {
            assert_roundtrip(ClientToServer::LeaveRoom, r#"{"type":"LeaveRoom"}"#);
        }

        #[test]
        fn offer_roundtrip_and_rejects_unknown() {
            assert_roundtrip(
                ClientToServer::Offer {
                    to: PEER_B.into(),
                    payload: RelaySdp {
                        sdp: SDP_OFFER.into(),
                    },
                },
                r#"{"type":"Offer","to":"peer-b","sdp":"v=0 offer"}"#,
            );

            let with_extra = r#"{"type":"Offer","to":"peer-b","sdp":"v=0 offer","extra":1}"#;
            assert!(serde_json::from_str::<ClientToServer>(with_extra).is_err());
        }

        #[test]
        fn answer_roundtrip_and_rejects_unknown() {
            assert_roundtrip(
                ClientToServer::Answer {
                    to: PEER_A.into(),
                    payload: RelaySdp {
                        sdp: SDP_ANSWER.into(),
                    },
                },
                r#"{"type":"Answer","to":"peer-a","sdp":"v=0 answer"}"#,
            );

            let with_extra = r#"{"type":"Answer","to":"peer-a","sdp":"v=0 answer","extra":true}"#;
            assert!(serde_json::from_str::<ClientToServer>(with_extra).is_err());
        }

        #[test]
        fn ice_candidate_roundtrip_and_missing_candidate_errors() {
            assert_roundtrip(
                ClientToServer::IceCandidate {
                    to: PEER_B.into(),
                    payload: RelayIce {
                        candidate: CANDIDATE.into(),
                    },
                },
                r#"{"type":"IceCandidate","to":"peer-b","candidate":"cand1"}"#,
            );

            let missing = r#"{"type":"IceCandidate","to":"peer-b"}"#;
            assert!(serde_json::from_str::<ClientToServer>(missing).is_err());
        }
    }

    mod server_to_client {
        use super::*;

        #[test]
        fn room_created_roundtrip() {
            assert_roundtrip(
                ServerToClient::RoomCreated {
                    room_id: ROOM_ID.into(),
                    self_id: SELF_ID.into(),
                },
                r#"{"type":"RoomCreated","room_id":"room-1","self_id":"self-1"}"#,
            );
        }

        #[test]
        fn room_participants_roundtrip_empty_and_multi() {
            assert_roundtrip(
                ServerToClient::RoomParticipants {
                    room_id: ROOM_ID.into(),
                    participants: vec![],
                },
                r#"{"type":"RoomParticipants","room_id":"room-1","participants":[]}"#,
            );

            assert_roundtrip(
                ServerToClient::RoomParticipants {
                    room_id: ROOM_ID.into(),
                    participants: vec!["a".into(), "b".into()],
                },
                r#"{"type":"RoomParticipants","room_id":"room-1","participants":["a","b"]}"#,
            );
        }

        #[test]
        fn peer_connected_and_disconnected_roundtrip() {
            assert_roundtrip(
                ServerToClient::PeerConnected {
                    participant_id: "p1".into(),
                },
                r#"{"type":"PeerConnected","participant_id":"p1"}"#,
            );

            assert_roundtrip(
                ServerToClient::PeerDisconnected {
                    participant_id: "p1".into(),
                },
                r#"{"type":"PeerDisconnected","participant_id":"p1"}"#,
            );
        }

        #[test]
        fn offer_answer_ice_roundtrip_and_reject_unknown() {
            assert_roundtrip(
                ServerToClient::Offer {
                    from: PEER_A.into(),
                    payload: RelaySdp {
                        sdp: SDP_OFFER.into(),
                    },
                },
                r#"{"type":"Offer","from":"peer-a","sdp":"v=0 offer"}"#,
            );
            assert_roundtrip(
                ServerToClient::Answer {
                    from: PEER_B.into(),
                    payload: RelaySdp {
                        sdp: SDP_ANSWER.into(),
                    },
                },
                r#"{"type":"Answer","from":"peer-b","sdp":"v=0 answer"}"#,
            );
            assert_roundtrip(
                ServerToClient::IceCandidate {
                    from: PEER_B.into(),
                    payload: RelayIce {
                        candidate: CANDIDATE.into(),
                    },
                },
                r#"{"type":"IceCandidate","from":"peer-b","candidate":"cand1"}"#,
            );

            let extra_offer = r#"{"type":"Offer","from":"p1","sdp":"offer","x":1}"#;
            assert!(serde_json::from_str::<ServerToClient>(extra_offer).is_err());
            let extra_answer = r#"{"type":"Answer","from":"p2","sdp":"answer","unexpected":true}"#;
            assert!(serde_json::from_str::<ServerToClient>(extra_answer).is_err());
            let extra_ice = r#"{"type":"IceCandidate","from":"p3","candidate":"cand","foo":"bar"}"#;
            assert!(serde_json::from_str::<ServerToClient>(extra_ice).is_err());
        }

        #[test]
        fn error_event_roundtrip_and_unknown_code_fails() {
            assert_roundtrip(
                ServerToClient::Error {
                    code: ErrorCode::ParticipantNotFound,
                    message: "target missing".into(),
                },
                r#"{"type":"Error","code":"ParticipantNotFound","message":"target missing"}"#,
            );

            let unknown_code = r#"{"type":"Error","code":"TotallyUnknown","message":"oops"}"#;
            assert!(serde_json::from_str::<ServerToClient>(unknown_code).is_err());
        }
    }

    mod smoke {
        use super::*;

        #[test]
        fn roundtrip_all_messages() {
            let client_samples: Vec<ClientToServer> = vec![
                ClientToServer::CreateRoom,
                ClientToServer::JoinRoom {
                    room_id: ROOM_ID.into(),
                },
                ClientToServer::LeaveRoom,
                ClientToServer::Offer {
                    to: PEER_B.into(),
                    payload: RelaySdp {
                        sdp: SDP_OFFER.into(),
                    },
                },
                ClientToServer::Answer {
                    to: PEER_A.into(),
                    payload: RelaySdp {
                        sdp: SDP_ANSWER.into(),
                    },
                },
                ClientToServer::IceCandidate {
                    to: PEER_B.into(),
                    payload: RelayIce {
                        candidate: CANDIDATE.into(),
                    },
                },
            ];

            for msg in client_samples {
                let json = serde_json::to_string(&msg).expect("serialize");
                let back: ClientToServer = serde_json::from_str(&json).expect("deserialize");
                assert_eq!(back, msg);
            }

            let server_samples: Vec<ServerToClient> = vec![
                ServerToClient::RoomCreated {
                    room_id: ROOM_ID.into(),
                    self_id: SELF_ID.into(),
                },
                ServerToClient::RoomParticipants {
                    room_id: ROOM_ID.into(),
                    participants: vec!["a".into(), "b".into()],
                },
                ServerToClient::PeerConnected {
                    participant_id: "a".into(),
                },
                ServerToClient::PeerDisconnected {
                    participant_id: "b".into(),
                },
                ServerToClient::Offer {
                    from: PEER_A.into(),
                    payload: RelaySdp {
                        sdp: SDP_OFFER.into(),
                    },
                },
                ServerToClient::Answer {
                    from: PEER_B.into(),
                    payload: RelaySdp {
                        sdp: SDP_ANSWER.into(),
                    },
                },
                ServerToClient::IceCandidate {
                    from: PEER_B.into(),
                    payload: RelayIce {
                        candidate: CANDIDATE.into(),
                    },
                },
                ServerToClient::Error {
                    code: ErrorCode::RoomFull,
                    message: "full".into(),
                },
            ];

            for ev in server_samples {
                let json = serde_json::to_string(&ev).expect("serialize");
                let back: ServerToClient = serde_json::from_str(&json).expect("deserialize");
                assert_eq!(back, ev);
            }
        }
    }
}
