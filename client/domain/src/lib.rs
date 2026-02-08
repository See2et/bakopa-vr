pub mod bridge;
pub mod ecs;
pub mod errors;
pub mod ports;
pub mod sync;
pub mod xr;

pub use bridge::{
    BridgePipeline, ClientBootstrap, ClientLifecycle, RuntimeBridge, RuntimeBridgeAdapter,
    RuntimeMode, RuntimeModePreference, StartDiagnostics, StateOverride, StateOverrideRequest,
};
pub use ecs::{
    CoreEcs, EcsCore, FrameClock, FrameId, InputEvent, InputSnapshot, Pose, RemoteRenderPose,
    RenderFrame, UnitQuat, Vec3,
};
pub use errors::{
    BridgeError, BridgeErrorState, CoreError, FrameError, ShutdownError, StartError, XrError,
};
pub use ports::{InputPort, NoopInputPort, OutputPort, RenderFrameBuffer};
pub use sync::{
    ParticipantId, PoseSyncCoordinator, PoseVersion, RemoteLiveness, RemoteParticipantState,
    RemotePoseRepository, RemotePoseState, RemotePoseUpdate, ScopeBoundaryError,
    ScopeBoundaryPolicy, ShutdownDrainReport, SignalingRoute, SyncDelta, SyncSessionError,
    SyncSessionPort,
};
pub use xr::XrRuntime;

#[cfg(test)]
mod tests;
