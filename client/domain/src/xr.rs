use crate::errors::XrError;

pub trait XrRuntime {
    fn enable(&mut self) -> Result<(), XrError>;
    fn is_ready(&self) -> bool;
    fn shutdown(&mut self) -> Result<(), XrError>;
}

#[cfg(test)]
pub(crate) trait XrInterfaceAccess {
    fn initialize(&mut self) -> bool;
    fn is_initialized(&self) -> bool;
    fn uninitialize(&mut self);
}

#[cfg(test)]
pub(crate) trait XrInterfaceProvider {
    type Interface: XrInterfaceAccess;
    fn find_openxr(&self) -> Option<Self::Interface>;
}

#[cfg(test)]
pub(crate) struct OpenXrRuntime<P: XrInterfaceProvider> {
    provider: P,
    interface: Option<P::Interface>,
    ready: bool,
}

#[cfg(test)]
impl<P: XrInterfaceProvider> OpenXrRuntime<P> {
    pub(crate) fn new(provider: P) -> Self {
        Self {
            provider,
            interface: None,
            ready: false,
        }
    }
}

#[cfg(test)]
impl<P: XrInterfaceProvider> XrRuntime for OpenXrRuntime<P> {
    fn enable(&mut self) -> Result<(), XrError> {
        let mut interface =
            self.provider
                .find_openxr()
                .ok_or_else(|| XrError::InitializationFailed {
                    reason: "openxr interface not found; enable OpenXR in project settings"
                        .to_string(),
                })?;

        if !interface.initialize() {
            self.ready = false;
            return Err(XrError::InitializationFailed {
                reason: "openxr initialization failed; ensure SteamVR is running".to_string(),
            });
        }

        self.ready = interface.is_initialized();
        if !self.ready {
            return Err(XrError::InitializationFailed {
                reason: "openxr runtime not ready; ensure SteamVR is running".to_string(),
            });
        }

        self.interface = Some(interface);
        Ok(())
    }

    fn is_ready(&self) -> bool {
        self.ready
    }

    fn shutdown(&mut self) -> Result<(), XrError> {
        if let Some(mut interface) = self.interface.take() {
            interface.uninitialize();
        }
        self.ready = false;
        Ok(())
    }
}
