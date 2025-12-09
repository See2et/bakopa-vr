mod common;

use bloom_core::{ParticipantId, RoomId};
use syncer::{
    webrtc_transport::WebrtcTransport, BasicSyncer, Syncer, SyncerEvent, SyncerRequest,
    TracingContext,
};

#[test]
fn failure_emits_peer_left_and_clears_participants() {
    let room = RoomId::new();
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    let (ta, tb) = WebrtcTransport::pair(a.clone(), b.clone());

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

    // simulate signaling/ICE failure: ここでは未実装のため、単にチャット送信後に PeerLeft を期待する（RED目的）
    let events = syncer_a.handle(SyncerRequest::SendChat {
        chat: common::sample_chat(&a),
        ctx: TracingContext::for_chat(&room, &a),
    });

    // 失敗処理で PeerLeft が1回だけ返ることを期待（現実装では返らないためRED）
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

    // 参加者テーブルが空になっていれば、AからのチャットはBに届かないはず
    let events_b = syncer_b.handle(SyncerRequest::SendChat {
        chat: common::sample_chat(&b),
        ctx: TracingContext::for_chat(&room, &b),
    });
    assert!(
        !events_b
            .iter()
            .any(|e| matches!(e, SyncerEvent::ChatReceived { .. })),
        "No chat should be delivered to b after failure cleanup"
    );
}
