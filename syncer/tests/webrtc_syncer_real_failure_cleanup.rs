mod common;

use std::time::Duration;

use bloom_core::{ParticipantId, RoomId};
use syncer::{
    webrtc_transport::RealWebrtcTransport, BasicSyncer, Syncer, SyncerEvent, SyncerRequest,
    TracingContext,
};

/// その後の送信が相手に届かない（参加者テーブルから除去される）ことを確認する。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failure_emits_single_peer_left_and_stops_delivery() {
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    // fail-fast な実PCペアを用意（ICE失敗を強制）
    let (ta, tb) = RealWebrtcTransport::pair_with_datachannel_real_failfast(a.clone(), b.clone())
        .await
        .expect("pc setup");

    let room = RoomId::new();
    let mut syncer_a = BasicSyncer::new(a.clone(), ta);
    let mut syncer_b = BasicSyncer::new(b.clone(), tb);

    // join 両者
    syncer_a.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: a.clone(),
    });
    syncer_b.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: b.clone(),
    });

    // A側でFailure→PeerLeftを観測するまでポーリング
    let mut peer_left_count = 0usize;
    for _ in 0..40 {
        let events = syncer_a.handle(SyncerRequest::SendChat {
            chat: common::sample_chat(&a),
            ctx: TracingContext::for_chat(&room, &a),
        });
        peer_left_count += events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    SyncerEvent::PeerLeft { participant_id } if participant_id == &b
                )
            })
            .count();
        if peer_left_count > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    assert_eq!(
        peer_left_count, 1,
        "PeerLeft should be emitted exactly once on transport failure"
    );

    // PeerLeft 後は相手への配送が行われないことを確認（参加者テーブルから除去されているはず）
    let mut delivered_after_failure = false;
    for _ in 0..10 {
        // A→B 送信（実際には宛先なしのはず）
        let _ = syncer_a.handle(SyncerRequest::SendChat {
            chat: common::sample_chat(&a),
            ctx: TracingContext::for_chat(&room, &a),
        });

        // B側で受信が無いことを確認
        let events_b = syncer_b.handle(SyncerRequest::SendChat {
            chat: common::sample_chat(&b),
            ctx: TracingContext::for_chat(&room, &b),
        });
        delivered_after_failure |= events_b.iter().any(|e| {
            matches!(
                e,
                SyncerEvent::ChatReceived { .. } | SyncerEvent::PoseReceived { .. }
            )
        });
        if delivered_after_failure {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    assert!(
        !delivered_after_failure,
        "messages should not be delivered after peer removal"
    );
}
