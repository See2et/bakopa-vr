use client_domain::errors::XrError;
use client_domain::xr::XrRuntime;
use tracing::warn;

#[derive(Default)]
pub struct GodotXrRuntime {
    ready: bool,
}

impl GodotXrRuntime {
    pub fn new() -> Self {
        Self::default()
    }
}

impl XrRuntime for GodotXrRuntime {
    fn enable(&mut self) -> Result<(), XrError> {
        self.ready = false;
        let reason = "OpenXR classes are not available in lightweight API mode".to_string();
        warn!(
            ready = self.ready,
            reason = %reason,
            "failed to enable xr runtime: {reason}"
        );
        Err(XrError::InitializationFailed { reason })
    }

    fn is_ready(&self) -> bool {
        self.ready
    }

    fn shutdown(&mut self) -> Result<(), XrError> {
        self.ready = false;
        Ok(())
    }
}
