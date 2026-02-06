#[derive(Debug, Clone, thiserror::Error)]
pub enum XrError {
    #[error("xr initialization failed: {reason}")]
    InitializationFailed { reason: String },
    #[error("xr shutdown failed: {reason}")]
    ShutdownFailed { reason: String },
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum CoreError {
    #[error("ecs world initialization failed: {reason}")]
    InitFailed { reason: String },
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum BridgeError {
    #[error("bridge initialization failed: {reason}")]
    InitializationFailed { reason: String },
    #[error("bridge shutdown failed: {reason}")]
    ShutdownFailed { reason: String },
    #[error("bridge is not started")]
    NotStarted,
    #[error("direct state mutation is not allowed")]
    DirectStateMutationDenied,
    #[error("render projection failed: {reason}")]
    ProjectionFailed { reason: String },
    #[error("core initialization failed")]
    CoreInit(#[source] CoreError),
    #[error("core update failed")]
    Core(#[source] CoreError),
}

#[derive(Debug, thiserror::Error)]
pub enum StartError {
    #[error("xr runtime initialization failed")]
    XrInit(#[source] XrError),
    #[error("xr runtime is not ready")]
    XrNotReady,
    #[error("runtime bridge initialization failed")]
    BridgeInit(#[source] BridgeError),
}

#[derive(Debug, thiserror::Error)]
pub enum ShutdownError {
    #[error("xr shutdown failed")]
    XrShutdown(#[source] XrError),
    #[error("bridge shutdown failed")]
    BridgeShutdown(#[source] BridgeError),
}

#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error("client is not running")]
    NotRunning,
    #[error("frame update failed")]
    Bridge(#[source] BridgeError),
}

#[derive(Debug, Default, Clone)]
pub struct BridgeErrorState {
    last_error: Option<BridgeError>,
}

impl BridgeErrorState {
    pub fn record(&mut self, error: &BridgeError) {
        self.last_error = Some(error.clone());
    }

    pub fn last(&self) -> Option<BridgeError> {
        self.last_error.clone()
    }

    pub fn last_message(&self) -> Option<String> {
        self.last_error.as_ref().map(ToString::to_string)
    }
}
