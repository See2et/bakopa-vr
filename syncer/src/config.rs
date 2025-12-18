use crate::signaling_adapter::SignalingContext;

#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Default)]
pub enum IcePolicy {
    #[default]
    Default,
}

impl IcePolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            IcePolicy::Default => "default",
        }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpcConfig {
    pub auth_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IpcConfigError {
    EmptyAuthToken,
}

impl std::fmt::Display for IpcConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IpcConfigError::EmptyAuthToken => write!(f, "auth_token must not be empty"),
        }
    }
}

impl std::error::Error for IpcConfigError {}

impl IpcConfig {
    pub fn new(auth_token: impl Into<String>) -> Result<Self, IpcConfigError> {
        let token = auth_token.into();
        if token.trim().is_empty() {
            return Err(IpcConfigError::EmptyAuthToken);
        }
        Ok(Self { auth_token: token })
    }

    pub fn to_signaling_ctx(&self, room_id: &str) -> SignalingContext {
        SignalingContext {
            room_id: room_id.to_string(),
            auth_token: self.auth_token.clone(),
            ice_policy: "default".to_string(),
        }
    }
}
