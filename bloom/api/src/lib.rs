//! Bloom signaling protocol types (WIP)
//! フェーズ1: CreateRoom要求のラウンドトリップテストをRedで用意する。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ClientToServer {
    /// Roomを新規作成する要求（フィールドなし）。
    CreateRoom,
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
        assert_eq!(json, r#"{\"type\":\"CreateRoom\"}"#);

        let back: ClientToServer = serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(back, msg);
    }
}
