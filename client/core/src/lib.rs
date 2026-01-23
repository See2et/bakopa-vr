use godot::prelude::*;

#[derive(Debug, Clone, thiserror::Error)]
pub enum XrError {
    #[error("xr initialization failed: {reason}")]
    InitializationFailed { reason: String },
    #[error("xr shutdown failed: {reason}")]
    ShutdownFailed { reason: String },
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum BridgeError {
    #[error("bridge initialization failed: {reason}")]
    InitializationFailed { reason: String },
    #[error("bridge shutdown failed: {reason}")]
    ShutdownFailed { reason: String },
}

#[derive(Debug, thiserror::Error)]
pub enum StartError {
    #[error("xr runtime initialization failed")]
    XrInit(#[source] XrError),
    #[error("xr runtime is not ready")]
    XrNotReady,
    #[error("godot bridge initialization failed")]
    BridgeInit(#[source] BridgeError),
}

pub trait XrRuntime {
    fn enable(&mut self) -> Result<(), XrError>;
    fn is_ready(&self) -> bool;
    fn shutdown(&mut self) -> Result<(), XrError>;
}

pub trait GodotBridge {
    fn on_start(&mut self) -> Result<(), BridgeError>;
    fn on_shutdown(&mut self) -> Result<(), BridgeError>;
}

pub trait ClientLifecycle {
    fn start(&mut self) -> Result<(), StartError>;
    fn shutdown(&mut self) -> Result<(), ShutdownError>;
}

#[derive(Debug, thiserror::Error)]
pub enum ShutdownError {
    #[error("shutdown failed: {reason}")]
    Failed { reason: String },
    #[error("xr shutdown failed")]
    XrShutdown(#[source] XrError),
    #[error("bridge shutdown failed")]
    BridgeShutdown(#[source] BridgeError),
}

pub struct ClientBootstrap<X: XrRuntime, B: GodotBridge> {
    xr: X,
    bridge: B,
    running: bool,
    frame_id: FrameId,
}

impl<X: XrRuntime, B: GodotBridge> ClientBootstrap<X, B> {
    pub fn new(xr: X, bridge: B) -> Self {
        Self {
            xr,
            bridge,
            running: false,
            frame_id: FrameId(0),
        }
    }

    pub fn start(&mut self) -> Result<(), StartError> {
        self.xr.enable().map_err(StartError::XrInit)?;
        if !self.xr.is_ready() {
            return Err(StartError::XrNotReady);
        }
        self.bridge.on_start().map_err(StartError::BridgeInit)?;
        self.running = true;
        Ok(())
    }

    pub fn tick_frame(&mut self) -> Result<FrameId, FrameError> {
        if !self.running {
            return Err(FrameError::NotRunning);
        }
        self.frame_id.0 += 1;
        Ok(self.frame_id)
    }
}

impl<X: XrRuntime, B: GodotBridge> ClientLifecycle for ClientBootstrap<X, B> {
    fn start(&mut self) -> Result<(), StartError> {
        ClientBootstrap::start(self)
    }

    fn shutdown(&mut self) -> Result<(), ShutdownError> {
        if !self.running {
            return Ok(());
        }
        self.bridge
            .on_shutdown()
            .map_err(ShutdownError::BridgeShutdown)?;
        self.xr
            .shutdown()
            .map_err(ShutdownError::XrShutdown)?;
        self.running = false;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameId(u64);

#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error("client is not running")]
    NotRunning,
}

struct SuteraClientCore;

#[gdextension]
unsafe impl ExtensionLibrary for SuteraClientCore {}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeXr {
        enable_calls: usize,
        ready: bool,
        enable_result: Result<(), XrError>,
        shutdown_calls: usize,
        shutdown_result: Result<(), XrError>,
    }

    impl XrRuntime for FakeXr {
        fn enable(&mut self) -> Result<(), XrError> {
            self.enable_calls += 1;
            self.enable_result.clone()
        }

        fn is_ready(&self) -> bool {
            self.ready
        }

        fn shutdown(&mut self) -> Result<(), XrError> {
            self.shutdown_calls += 1;
            self.shutdown_result.clone()
        }
    }

    struct FakeBridge {
        start_calls: usize,
        start_result: Result<(), BridgeError>,
        shutdown_calls: usize,
        shutdown_result: Result<(), BridgeError>,
    }

    impl Default for FakeXr {
        fn default() -> Self {
            Self {
                enable_calls: 0,
                ready: false,
                enable_result: Ok(()),
                shutdown_calls: 0,
                shutdown_result: Ok(()),
            }
        }
    }

    impl Default for FakeBridge {
        fn default() -> Self {
            Self {
                start_calls: 0,
                start_result: Ok(()),
                shutdown_calls: 0,
                shutdown_result: Ok(()),
            }
        }
    }

    impl GodotBridge for FakeBridge {
        fn on_start(&mut self) -> Result<(), BridgeError> {
            self.start_calls += 1;
            self.start_result.clone()
        }

        fn on_shutdown(&mut self) -> Result<(), BridgeError> {
            self.shutdown_calls += 1;
            self.shutdown_result.clone()
        }
    }

    #[test]
    fn start_initializes_dependencies_on_request() {
        let xr = FakeXr {
            ready: true,
            enable_result: Ok(()),
            ..FakeXr::default()
        };
        let bridge = FakeBridge {
            start_result: Ok(()),
            ..FakeBridge::default()
        };
        let mut bootstrap = ClientBootstrap::new(xr, bridge);

        let result = bootstrap.start();

        assert!(result.is_ok());
        assert_eq!(bootstrap.xr.enable_calls, 1);
        assert_eq!(bootstrap.bridge.start_calls, 1);
    }

    #[test]
    fn start_reports_xr_initialization_failure() {
        let xr = FakeXr {
            ready: false,
            enable_result: Err(XrError::InitializationFailed {
                reason: "no runtime".to_string(),
            }),
            ..FakeXr::default()
        };
        let bridge = FakeBridge::default();
        let mut bootstrap = ClientBootstrap::new(xr, bridge);

        let result = bootstrap.start();

        assert!(matches!(result, Err(StartError::XrInit(_))));
        assert_eq!(bootstrap.bridge.start_calls, 0);
    }

    #[test]
    fn start_reports_xr_not_ready() {
        let xr = FakeXr {
            ready: false,
            enable_result: Ok(()),
            ..FakeXr::default()
        };
        let bridge = FakeBridge::default();
        let mut bootstrap = ClientBootstrap::new(xr, bridge);

        let result = bootstrap.start();

        assert!(matches!(result, Err(StartError::XrNotReady)));
        assert_eq!(bootstrap.bridge.start_calls, 0);
    }

    #[test]
    fn start_reports_bridge_initialization_failure() {
        let xr = FakeXr {
            ready: true,
            enable_result: Ok(()),
            ..FakeXr::default()
        };
        let bridge = FakeBridge {
            start_result: Err(BridgeError::InitializationFailed {
                reason: "gdext missing".to_string(),
            }),
            ..FakeBridge::default()
        };
        let mut bootstrap = ClientBootstrap::new(xr, bridge);

        let result = bootstrap.start();

        assert!(matches!(result, Err(StartError::BridgeInit(_))));
        assert_eq!(bootstrap.bridge.start_calls, 1);
    }

    #[test]
    fn tick_frame_continues_while_running() {
        let xr = FakeXr {
            ready: true,
            enable_result: Ok(()),
            ..FakeXr::default()
        };
        let bridge = FakeBridge {
            start_result: Ok(()),
            ..FakeBridge::default()
        };
        let mut bootstrap = ClientBootstrap::new(xr, bridge);

        bootstrap.start().expect("start succeeds");
        let first = bootstrap.tick_frame().expect("first frame");
        let second = bootstrap.tick_frame().expect("second frame");

        assert_eq!(first, FrameId(1));
        assert_eq!(second, FrameId(2));
    }

    #[test]
    fn tick_frame_fails_when_not_running() {
        let xr = FakeXr::default();
        let bridge = FakeBridge::default();
        let mut bootstrap = ClientBootstrap::new(xr, bridge);

        let result = bootstrap.tick_frame();

        assert!(matches!(result, Err(FrameError::NotRunning)));
    }

    #[test]
    fn shutdown_releases_resources_and_stops_running() {
        let xr = FakeXr {
            ready: true,
            enable_result: Ok(()),
            ..FakeXr::default()
        };
        let bridge = FakeBridge {
            start_result: Ok(()),
            ..FakeBridge::default()
        };
        let mut bootstrap = ClientBootstrap::new(xr, bridge);

        bootstrap.start().expect("start succeeds");
        bootstrap.shutdown().expect("shutdown succeeds");

        assert_eq!(bootstrap.bridge.shutdown_calls, 1);
        assert_eq!(bootstrap.xr.shutdown_calls, 1);
        assert!(matches!(
            bootstrap.tick_frame(),
            Err(FrameError::NotRunning)
        ));
    }

    #[test]
    fn shutdown_reports_bridge_failure() {
        let xr = FakeXr {
            ready: true,
            enable_result: Ok(()),
            ..FakeXr::default()
        };
        let bridge = FakeBridge {
            start_result: Ok(()),
            shutdown_result: Err(BridgeError::ShutdownFailed {
                reason: "bridge busy".to_string(),
            }),
            ..FakeBridge::default()
        };
        let mut bootstrap = ClientBootstrap::new(xr, bridge);

        bootstrap.start().expect("start succeeds");
        let result = bootstrap.shutdown();

        assert!(matches!(result, Err(ShutdownError::BridgeShutdown(_))));
        assert_eq!(bootstrap.bridge.shutdown_calls, 1);
        assert_eq!(bootstrap.xr.shutdown_calls, 0);
    }
}
