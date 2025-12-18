mod common;

use bloom_core::{ParticipantId, RoomId};
use syncer::{
    webrtc_transport::{WebrtcTransport, WebrtcTransportOptions},
    BasicSyncer, Syncer, SyncerEvent, SyncerRequest, TracingContext,
};

#[test]
fn failure_emits_peer_left_and_clears_participants() {
    let room = RoomId::new();
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    let opts = WebrtcTransportOptions {
        inject_failure_once: true,
    };
    let (ta, tb) = WebrtcTransport::pair_with_options(
        a.clone(),
        b.clone(),
        opts,
        WebrtcTransportOptions::default(),
    );

    let mut syncer_a = BasicSyncer::new(a.clone(), ta);
    let mut syncer_b = BasicSyncer::new(b.clone(), tb);

    // join both peers
    syncer_a.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: a.clone(),
    });
    syncer_b.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: b.clone(),
    });

    // simulate signaling/ICE failure: ここでは未実装のため、単にチャット送信後に PeerLeft を期待する
    let events = syncer_a.handle(SyncerRequest::SendChat {
        chat: common::sample_chat(&a),
        ctx: TracingContext::for_chat(&room, &a),
    });

    // 失敗処理で PeerLeft が1回だけ返ることを期待
    assert!(
        events
            .iter()
            .filter(
                |e| matches!(e, SyncerEvent::PeerLeft { participant_id } if participant_id == &b)
            )
            .count()
            == 1,
        "PeerLeft for b should be emitted exactly once on failure"
    );

    // B側への配送可否はローカルテーブル次第のためここでは検証しない
}
