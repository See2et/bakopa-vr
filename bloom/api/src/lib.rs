//! Bloom signaling protocol types (WIP)
//! フェーズ1: CreateRoom要求のラウンドトリップテストをRedで用意する。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "PascalCase", deny_unknown_fields)]
pub enum ClientToServer {
    /// Roomを新規作成する要求（フィールドなし）。
    CreateRoom,
    /// 既存Roomに参加する要求（room_id必須）。
    JoinRoom { room_id: String },
    /// Roomから離脱する要求（フィールドなし）。
    LeaveRoom,
    /// WebRTC Offer を特定participantへ中継要求。
    Offer { to: String, sdp: String },
    /// WebRTC Answer を特定participantへ中継要求。
    Answer { to: String, sdp: String },
    /// ICE candidate を特定participantへ中継要求。
    IceCandidate { to: String, candidate: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ServerToClient {
    RoomCreated { room_id: String, self_id: String },
    RoomParticipants { room_id: String, participants: Vec<String> },
    PeerConnected { participant_id: String },
    PeerDisconnected { participant_id: String },
    Offer { from: String, sdp: String },
    Answer { from: String, sdp: String },
    IceCandidate { from: String, candidate: String },
    Error { code: ErrorCode, message: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ErrorCode {
    RoomFull,
    InvalidPayload,
    ParticipantNotFound,
    RateLimited,
    Internal,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_room_roundtrip_uses_type_tagged_json() {
        // Arrange
        let msg = ClientToServer::CreateRoom;

        // Act
        let json = serde_json::to_string(&msg).expect("should serialize");

        // Assert: 仕様では {"type":"CreateRoom"} を期待する。
        assert_eq!(json, r#"{"type":"CreateRoom"}"#);

        let back: ClientToServer = serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(back, msg);
    }

    #[test]
    fn join_room_roundtrip_uses_type_tagged_json() {
        // Arrange
        let msg = ClientToServer::JoinRoom {
            room_id: "room-1".into(),
        };

        // Act
        let json = serde_json::to_string(&msg).expect("should serialize");

        // Assert: 仕様では {"type":"JoinRoom","room_id":"..."} を期待する。
        assert_eq!(json, r#"{"type":"JoinRoom","room_id":"room-1"}"#);

        let back: ClientToServer = serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(back, msg);
    }

    #[test]
    fn join_room_missing_room_id_is_error() {
        // Arrange: room_id欠落。
        let json = r#"{"type":"JoinRoom"}"#;

        // Act
        let result: Result<ClientToServer, _> = serde_json::from_str(json);

        // Assert
        assert!(result.is_err(), "room_id欠落はエラーであるべき");
    }

    #[test]
    fn leave_room_roundtrip_uses_type_tagged_json() {
        // Arrange
        let msg = ClientToServer::LeaveRoom;

        // Act
        let json = serde_json::to_string(&msg).expect("should serialize");

        // Assert: 仕様では {"type":"LeaveRoom"} を期待する。
        assert_eq!(json, r#"{"type":"LeaveRoom"}"#);

        let back: ClientToServer = serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(back, msg);
    }

    #[test]
    fn offer_roundtrip_and_rejects_unknown_field() {
        let msg = ClientToServer::Offer {
            to: "peer-b".into(),
            sdp: "v=0...".into(),
        };

        let json = serde_json::to_string(&msg).expect("serialize");
        assert_eq!(json, r#"{"type":"Offer","to":"peer-b","sdp":"v=0..."}"#);

        let back: ClientToServer = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, msg);

        // unknown field should error
        let with_extra = r#"{"type":"Offer","to":"peer-b","sdp":"v=0...","extra":1}"#;
        assert!(serde_json::from_str::<ClientToServer>(with_extra).is_err());
    }

    #[test]
    fn answer_roundtrip_and_rejects_unknown_field() {
        let msg = ClientToServer::Answer {
            to: "peer-a".into(),
            sdp: "v=0 ans".into(),
        };

        let json = serde_json::to_string(&msg).expect("serialize");
        assert_eq!(json, r#"{"type":"Answer","to":"peer-a","sdp":"v=0 ans"}"#);

        let back: ClientToServer = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, msg);

        let with_extra = r#"{"type":"Answer","to":"peer-a","sdp":"v=0 ans","extra":true}"#;
        assert!(serde_json::from_str::<ClientToServer>(with_extra).is_err());
    }

    #[test]
    fn ice_candidate_roundtrip_and_missing_candidate_is_error() {
        let msg = ClientToServer::IceCandidate {
            to: "peer-c".into(),
            candidate: "cand1".into(),
        };

        let json = serde_json::to_string(&msg).expect("serialize");
        assert_eq!(json, r#"{"type":"IceCandidate","to":"peer-c","candidate":"cand1"}"#);

        let back: ClientToServer = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, msg);

        let missing = r#"{"type":"IceCandidate","to":"peer-c"}"#;
        assert!(serde_json::from_str::<ClientToServer>(missing).is_err());
    }

    #[test]
    fn room_created_roundtrip() {
        let msg = ServerToClient::RoomCreated {
            room_id: "room-1".into(),
            self_id: "self-1".into(),
        };

        let json = serde_json::to_string(&msg).expect("serialize");
        assert_eq!(json, r#"{"type":"RoomCreated","room_id":"room-1","self_id":"self-1"}"#);

        let back: ServerToClient = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, msg);
    }

    #[test]
    fn room_participants_roundtrip_with_empty_and_multiple() {
        let msg_empty = ServerToClient::RoomParticipants {
            room_id: "room-1".into(),
            participants: vec![],
        };
        let json_empty = serde_json::to_string(&msg_empty).expect("serialize");
        assert_eq!(json_empty, r#"{"type":"RoomParticipants","room_id":"room-1","participants":[]}"#);
        let back_empty: ServerToClient = serde_json::from_str(&json_empty).expect("deserialize");
        assert_eq!(back_empty, msg_empty);

        let msg_many = ServerToClient::RoomParticipants {
            room_id: "room-1".into(),
            participants: vec!["a".into(), "b".into()],
        };
        let json_many = serde_json::to_string(&msg_many).expect("serialize");
        assert_eq!(json_many, r#"{"type":"RoomParticipants","room_id":"room-1","participants":["a","b"]}"#);
        let back_many: ServerToClient = serde_json::from_str(&json_many).expect("deserialize");
        assert_eq!(back_many, msg_many);
    }

    #[test]
    fn peer_connected_and_disconnected_roundtrip() {
        let connected = ServerToClient::PeerConnected {
            participant_id: "p1".into(),
        };
        let json_c = serde_json::to_string(&connected).expect("serialize");
        assert_eq!(json_c, r#"{"type":"PeerConnected","participant_id":"p1"}"#);
        let back_c: ServerToClient = serde_json::from_str(&json_c).expect("deserialize");
        assert_eq!(back_c, connected);

        let disconnected = ServerToClient::PeerDisconnected {
            participant_id: "p1".into(),
        };
        let json_d = serde_json::to_string(&disconnected).expect("serialize");
        assert_eq!(json_d, r#"{"type":"PeerDisconnected","participant_id":"p1"}"#);
        let back_d: ServerToClient = serde_json::from_str(&json_d).expect("deserialize");
        assert_eq!(back_d, disconnected);
    }

    #[test]
    fn server_offer_answer_ice_roundtrip() {
        let offer = ServerToClient::Offer {
            from: "p1".into(),
            sdp: "offer".into(),
        };
        let json_offer = serde_json::to_string(&offer).expect("serialize");
        assert_eq!(json_offer, r#"{"type":"Offer","from":"p1","sdp":"offer"}"#);
        let back_offer: ServerToClient = serde_json::from_str(&json_offer).expect("deserialize");
        assert_eq!(back_offer, offer);

        let answer = ServerToClient::Answer {
            from: "p2".into(),
            sdp: "answer".into(),
        };
        let json_answer = serde_json::to_string(&answer).expect("serialize");
        assert_eq!(json_answer, r#"{"type":"Answer","from":"p2","sdp":"answer"}"#);
        let back_answer: ServerToClient = serde_json::from_str(&json_answer).expect("deserialize");
        assert_eq!(back_answer, answer);

        let ice = ServerToClient::IceCandidate {
            from: "p3".into(),
            candidate: "cand".into(),
        };
        let json_ice = serde_json::to_string(&ice).expect("serialize");
        assert_eq!(json_ice, r#"{"type":"IceCandidate","from":"p3","candidate":"cand"}"#);
        let back_ice: ServerToClient = serde_json::from_str(&json_ice).expect("deserialize");
        assert_eq!(back_ice, ice);
    }
}
