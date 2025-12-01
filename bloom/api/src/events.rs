use serde::{Deserialize, Serialize};

use crate::errors::ErrorCode;
use crate::payload::{RelayIce, RelaySdp};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "PascalCase", deny_unknown_fields)]
pub enum ServerToClient {
    RoomCreated {
        room_id: String,
        self_id: String,
    },
    RoomParticipants {
        room_id: String,
        participants: Vec<String>,
    },
    PeerConnected {
        participant_id: String,
    },
    PeerDisconnected {
        participant_id: String,
    },
    Offer {
        from: String,
        #[serde(flatten)]
        payload: RelaySdp,
    },
    Answer {
        from: String,
        #[serde(flatten)]
        payload: RelaySdp,
    },
    IceCandidate {
        from: String,
        #[serde(flatten)]
        payload: RelayIce,
    },
    Error {
        code: ErrorCode,
        message: String,
    },
}
