use anyhow::Result;

/// Core application handle for the Sidecar service.
pub struct App;

impl App {
    /// Construct a new application instance.
    /// This is a placeholder; wiring to networking/runtime will be added in later phases.
    pub async fn new() -> Result<Self> {
        Ok(Self)
    }
}
