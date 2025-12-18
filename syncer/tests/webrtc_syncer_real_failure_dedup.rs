mod common;

use std::time::Duration;

use bloom_core::{ParticipantId, RoomId};
use syncer::{
    webrtc_transport::RealWebrtcTransport, BasicSyncer, StreamKind, Syncer, SyncerEvent,
    SyncerRequest, TracingContext, TransportEvent,
};

/// RED→GREEN: 実PC経路で Failure が2度観測されても PeerLeft が1回だけになることを確認する。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failure_is_deduplicated_even_with_real_transport() {
    let a = ParticipantId::new();
    let b = ParticipantId::new();
    let room = RoomId::new();

    // 通常の実PCペア（故意に故障させない）。Failure は手動注入する。
    let (mut ta, mut tb) = RealWebrtcTransport::pair_with_datachannel_real(a.clone(), b.clone())
        .await
        .expect("pc setup");

    let timeout = Duration::from_secs(5);
    ta.wait_data_channel_open(timeout).await.expect("open a");
    tb.wait_data_channel_open(timeout).await.expect("open b");

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

    // DataChannel/AudioTrack close を模して Failure を2回注入
    syncer_a.push_transport_event(TransportEvent::Failure { peer: b.clone() });
    syncer_a.push_transport_event(TransportEvent::Failure { peer: b.clone() });

    // Failure が届き切るまでポーリング
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
        if peer_left_count >= 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    assert_eq!(
        peer_left_count, 1,
        "PeerLeft should be emitted exactly once even if Failure occurs twice"
    );

    // 参加者テーブルから相手が消えていること
    let snapshot = syncer_a.participants_snapshot();
    assert!(
        !snapshot.contains(&b),
        "peer should be removed from participant table after failures"
    );

    // 追加のFailureが来てもPeerLeftが増えないこと
    syncer_a.push_transport_event(TransportEvent::Failure { peer: b.clone() });
    let mut additional_peer_left = 0usize;
    for _ in 0..10 {
        let events = syncer_a.poll_only();
        additional_peer_left += events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    SyncerEvent::PeerLeft { participant_id } if participant_id == &b
                )
            })
            .count();
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        additional_peer_left, 0,
        "no extra PeerLeft should be emitted after the first one"
    );

    // 再送が止まっていることを念のため確認（Bに届かない＝イベント0）
    let events_b = syncer_b.handle(SyncerRequest::SendChat {
        chat: common::sample_chat(&b),
        ctx: TracingContext::for_chat(&room, &b),
    });
    assert!(
        !events_b.iter().any(|e| matches!(e, SyncerEvent::ChatReceived { ctx, .. } if ctx.stream_kind == StreamKind::Chat && ctx.participant_id == a)),
        "messages from removed peer should not be delivered"
    );
}
