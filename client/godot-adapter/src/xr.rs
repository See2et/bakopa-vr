use client_domain::errors::XrError;
use client_domain::xr::XrRuntime;

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
        Err(XrError::InitializationFailed {
            reason: "OpenXR classes are not available in lightweight API mode".to_string(),
        })
    }

    fn is_ready(&self) -> bool {
        self.ready
    }

    fn shutdown(&mut self) -> Result<(), XrError> {
        self.ready = false;
        Ok(())
    }
}
