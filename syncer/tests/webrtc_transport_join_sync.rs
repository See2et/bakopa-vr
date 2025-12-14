mod common;

use bloom_core::{ParticipantId, RoomId};
use common::{sample_chat, sample_tracing_context};
use syncer::{BasicSyncer, Syncer, SyncerEvent, SyncerRequest};

/// RED: Joinだけで相互にPeerJoinedが届くことを保証したい（ダブルJoin禁止）。
#[test]
fn join_broadcasts_peer_join_once_each_side() {
    let room = RoomId::new();
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    let (ta, tb) = syncer::webrtc_transport::WebrtcTransport::pair(a.clone(), b.clone());

    let mut syncer_a = BasicSyncer::new(a.clone(), ta);
    let mut syncer_b = BasicSyncer::new(b.clone(), tb);

    let _events_a = syncer_a.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: a.clone(),
    });

    let events_b = syncer_b.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: b.clone(),
    });

    // B側はAのControlJoinを受信しているはず
    let peer_joined_seen_by_b = events_b.iter().any(|ev| {
        matches!(ev, SyncerEvent::PeerJoined { participant_id } if participant_id == &a)
    });
    assert!(
        peer_joined_seen_by_b,
        "B should see A join via ControlJoin broadcast"
    );

    // A側も追加のJoin呼び出しなしでBのJoin通知を受け取れることを確認（送信処理を兼ねたポーリング）
    let poll1 = syncer_a.handle(SyncerRequest::SendChat {
        chat: sample_chat(&a),
        ctx: sample_tracing_context(&room, &a),
    });
    let poll2 = syncer_a.handle(SyncerRequest::SendChat {
        chat: sample_chat(&a),
        ctx: sample_tracing_context(&room, &a),
    });
    let peer_joined_seen_by_a = poll1
        .iter()
        .chain(poll2.iter())
        .any(|ev| matches!(ev, SyncerEvent::PeerJoined { participant_id } if participant_id == &b));
    assert!(
        peer_joined_seen_by_a,
        "A should see B join without a second Join call"
    );
}
