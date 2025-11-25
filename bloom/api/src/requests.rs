use serde::{Deserialize, Serialize};

use crate::payload::{RelayIce, RelaySdp};

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
    Offer {
        to: String,
        #[serde(flatten)]
        payload: RelaySdp,
    },
    /// WebRTC Answer を特定participantへ中継要求。
    Answer {
        to: String,
        #[serde(flatten)]
        payload: RelaySdp,
    },
    /// ICE candidate を特定participantへ中継要求。
    IceCandidate {
        to: String,
        #[serde(flatten)]
        payload: RelayIce,
    },
}
