use serde::{Deserialize, Serialize};

/// SDP を伴うシグナリング転送メッセージの共通ペイロード。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelaySdp {
    pub sdp: String,
}

/// ICE candidate を伴うシグナリング転送メッセージの共通ペイロード。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelayIce {
    pub candidate: String,
}
