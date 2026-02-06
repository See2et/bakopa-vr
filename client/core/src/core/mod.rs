pub mod bridge;
pub mod ecs;
pub mod errors;
pub mod ports;
pub mod xr;

pub use bridge::{
    BridgePipeline, ClientBootstrap, ClientLifecycle, GodotBridge, GodotBridgeAdapter,
    StateOverride, StateOverrideRequest,
};
pub use ecs::{
    CoreEcs, EcsCore, FrameClock, FrameId, InputEvent, InputSnapshot, Pose, RenderFrame, UnitQuat,
    Vec3,
};
pub use errors::{
    BridgeError, BridgeErrorState, CoreError, FrameError, ShutdownError, StartError, XrError,
};
pub use ports::{InputPort, NoopInputPort, OutputPort, RenderFrameBuffer};
pub use xr::XrRuntime;

#[cfg(test)]
mod tests;
