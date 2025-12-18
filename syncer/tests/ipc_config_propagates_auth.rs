use bloom_api::payload::RelaySdp;
use bloom_api::ServerToClient;
use bloom_core::ParticipantId;

use syncer::config::IpcConfig;
use syncer::messages::{SignalingMessage, SyncMessage, SyncMessageEnvelope};
use syncer::signaling_adapter::{BloomSignalingAdapter, ClientToServerSender, SignalingContext};
use syncer::TransportPayload;

#[derive(Default)]
struct NoopSender;
impl ClientToServerSender for NoopSender {
    fn send(&mut self, _message: bloom_api::ClientToServer) {}
}

/// RED: IpcConfig の auth_token が SignalingContext 経由で BloomSignalingAdapter に反映されることを期待する。
#[test]
fn auth_token_propagates_into_adapter() {
    let token = "dev-token-123";
    let room = "room-xyz";

    let cfg = IpcConfig::new(token).expect("ipc config should construct");
    let ctx: SignalingContext = cfg.to_signaling_ctx(room);

    let mut adapter =
        BloomSignalingAdapter::<NoopSender>::with_context(NoopSender::default(), ctx);

    // inbound offer を流し、内側で埋められる auth_token を検査
    adapter.push_incoming(ServerToClient::Offer {
        from: ParticipantId::new().to_string(),
        payload: RelaySdp {
            sdp: "v=0".to_string(),
        },
    });

    let polled = adapter.poll();
    let payload = polled
        .payloads
        .into_iter()
        .find_map(|p| match p {
            TransportPayload::Bytes(b) => Some(b),
            _ => None,
        })
        .expect("payload must exist");

    let envelope =
        SyncMessageEnvelope::from_slice(&payload).expect("envelope must parse from payload");
    let msg = SyncMessage::from_envelope(envelope).expect("envelope should decode");
    let offer = match msg {
        SyncMessage::Signaling(SignalingMessage::Offer(o)) => o,
        other => panic!("expected signaling offer, got {:?}", other),
    };

    assert_eq!(
        offer.auth_token, token,
        "auth_token should be injected from IpcConfig"
    );
}

/// RED: 空 auth_token は拒否されるべき。
#[test]
fn empty_auth_token_is_rejected() {
    assert!(
        IpcConfig::new("").is_err(),
        "empty auth_token should be an error"
    );
}
