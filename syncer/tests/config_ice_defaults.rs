use syncer::config::{IceConfig, IcePolicy};
use syncer::signaling_adapter::SignalingContext;

/// RED: IceConfig のデフォルトと SignalingContext への伝播を固定する。
#[test]
fn ice_config_defaults_and_signaling_projection() {
    // default は policy = "default", servers 空
    let cfg = IceConfig::default();
    assert_eq!(cfg.policy, IcePolicy::Default);
    assert!(cfg.servers.is_empty(), "servers should be empty by default");

    // SignalingContext への投影
    let room = "room-xyz".to_string();
    let auth = "token-abc".to_string();
    let ctx: SignalingContext = cfg.to_signaling_ctx(&room, &auth);
    assert_eq!(ctx.room_id, room);
    assert_eq!(ctx.auth_token, auth);
    assert_eq!(ctx.ice_policy, "default");
}
