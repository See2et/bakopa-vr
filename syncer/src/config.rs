use crate::signaling_adapter::SignalingContext;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IcePolicy {
    Default,
}

impl IcePolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            IcePolicy::Default => "default",
        }
    }
}

impl Default for IcePolicy {
    fn default() -> Self {
        IcePolicy::Default
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct IceConfig {
    pub policy: IcePolicy,
    pub servers: Vec<String>,
}

impl IceConfig {
    /// SignalingContext への投影。room/auth は呼び出し側が供給する。
    pub fn to_signaling_ctx(&self, room_id: &str, auth_token: &str) -> SignalingContext {
        SignalingContext {
            room_id: room_id.to_string(),
            auth_token: auth_token.to_string(),
            ice_policy: self.policy.as_str().to_string(),
        }
    }
}
