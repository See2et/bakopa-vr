mod common;

use std::time::Duration;

use bloom_core::{ParticipantId, RoomId};
use syncer::{
    webrtc_transport::RealWebrtcTransport, BasicSyncer, StreamKind, Syncer, SyncerEvent,
    SyncerRequest, TracingContext,
};

/// RED: PeerLeft 後に同じ participant_id で再接続し直せることを検証する統合テスト。
/// 必要なフック（transport 再バインドなど）が未実装のため、まずはコンパイル失敗で Red を作る。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reconnects_after_failure_and_delivers_again() {
    let a = ParticipantId::new();
    let b = ParticipantId::new();
    let room = RoomId::new();

    // 1st connection: fail fast to trigger PeerLeft
    let (ta1, tb1) =
        RealWebrtcTransport::pair_with_datachannel_real_failfast(a.clone(), b.clone())
            .await
            .expect("pc setup");

    let mut syncer_a = BasicSyncer::new(a.clone(), ta1);
    let mut syncer_b = BasicSyncer::new(b.clone(), tb1);

    // initial join
    syncer_a.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: a.clone(),
    });
    syncer_b.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: b.clone(),
    });

    // wait until A observes PeerLeft for B due to transport failure
    let mut left_seen = false;
    for _ in 0..40 {
        let events = syncer_a.handle(SyncerRequest::SendChat {
            chat: common::sample_chat(&a),
            ctx: TracingContext::for_chat(&room, &a),
        });
        left_seen |= events.iter().any(|e| {
            matches!(
                e,
                SyncerEvent::PeerLeft { participant_id } if participant_id == &b
            )
        });
        if left_seen {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(left_seen, "precondition: PeerLeft should be observed");

    // 2nd connection: new PC pair reuses same participant_id
    let (mut ta2, mut tb2) =
        RealWebrtcTransport::pair_with_datachannel_real(a.clone(), b.clone())
            .await
            .expect("pc setup 2");

    let timeout = Duration::from_secs(5);
    ta2.wait_data_channel_open(timeout).await.expect("open a second pc");
    tb2.wait_data_channel_open(timeout).await.expect("open b second pc");

    // 未実装のフック: 既存 Syncer に新Transportを再バインドする前提
    syncer_a.rebind_transport(ta2);
    syncer_b.rebind_transport(tb2);

    // re-join with same ids
    syncer_a.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: a.clone(),
    });
    syncer_b.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: b.clone(),
    });

    // 再接続後に互いの PeerJoined を観測するまでポーリング
    let mut a_seen_b = false;
    let mut b_seen_a = false;
    for _ in 0..60 {
        let ev_a = syncer_a.handle(SyncerRequest::SendChat {
            chat: common::sample_chat(&a),
            ctx: TracingContext::for_chat(&room, &a),
        });
        let ev_b = syncer_b.handle(SyncerRequest::SendChat {
            chat: common::sample_chat(&b),
            ctx: TracingContext::for_chat(&room, &b),
        });
        a_seen_b |= ev_a.iter().any(|e| matches!(e, SyncerEvent::PeerJoined { participant_id } if participant_id == &b));
        b_seen_a |= ev_b.iter().any(|e| matches!(e, SyncerEvent::PeerJoined { participant_id } if participant_id == &a));
        if a_seen_b && b_seen_a {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    assert!(a_seen_b, "A should observe B rejoin once");
    assert!(b_seen_a, "B should observe A rejoin once");

    // chat roundtrip after reconnection
    syncer_a.handle(SyncerRequest::SendChat {
        chat: common::sample_chat(&a),
        ctx: TracingContext::for_chat(&room, &a),
    });

    let mut received_chat = false;
    for _ in 0..40 {
        let events = syncer_b.handle(SyncerRequest::SendChat {
            chat: common::sample_chat(&b),
            ctx: TracingContext::for_chat(&room, &b),
        });
        received_chat |= events.iter().any(|e| matches!(e, SyncerEvent::ChatReceived { ctx, .. } if ctx.stream_kind == StreamKind::Chat && ctx.participant_id == a));
        if received_chat {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    assert!(
        received_chat,
        "chat should be delivered after reconnection with the same participant_id"
    );
}
