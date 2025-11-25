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
        assert_eq!(json, r#"{\"type\":\"JoinRoom\",\"room_id\":\"room-1\"}"#);

        let back: ClientToServer = serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(back, msg);
    }

    #[test]
    fn join_room_missing_room_id_is_error() {
        // Arrange: room_id欠落。
        let json = r#"{\"type\":\"JoinRoom\"}"#;

        // Act
        let result: Result<ClientToServer, _> = serde_json::from_str(json);

        // Assert
        assert!(result.is_err(), "room_id欠落はエラーであるべき");
    }
}
