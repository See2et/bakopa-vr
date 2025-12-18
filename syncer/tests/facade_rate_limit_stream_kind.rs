mod common;

use std::time::Duration;

use bloom_core::{ParticipantId, RoomId};
use common::bus_transport::{new_bus, BusTransport};
use common::fake_clock::FakeClock;
use syncer::rate_limiter::RateLimiter;
use syncer::{BasicSyncer, StreamKind, Syncer, SyncerEvent, SyncerRequest, TracingContext};

#[test]
fn rate_limited_stream_kind_reflects_latest_request() {
    let room = RoomId::new();
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    let bus = new_bus();
    let clock = FakeClock::new(std::time::Instant::now());
    let limiter = RateLimiter::with_clock(20, Duration::from_secs(1), clock);

    let ta = BusTransport::new(a.clone(), bus.clone());
    let tb = BusTransport::new(b.clone(), bus.clone());

    let mut syncer_a = BasicSyncer::with_rate_limiter(a.clone(), ta, limiter);
    let mut syncer_b = BasicSyncer::new(b.clone(), tb);

    // join and drain control joins
    syncer_a.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: a.clone(),
    });
    syncer_b.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: b.clone(),
    });
    let _ = syncer_a.poll_only();
    let _ = syncer_b.poll_only();
    bus.borrow_mut().messages.clear();

    // 20 Pose を送って上限に到達させる
    for _ in 0..20 {
        let ev = syncer_a.handle(SyncerRequest::SendPose {
            from: a.clone(),
            pose: common::sample_pose(),
            ctx: common::sample_tracing_context(&room, &a),
        });
        assert!(
            !ev.iter()
                .any(|e| matches!(e, SyncerEvent::RateLimited { .. })),
            "first 20 should be allowed"
        );
    }

    // 21件目はPoseでRateLimited（確認だけ）
    let ev = syncer_a.handle(SyncerRequest::SendPose {
        from: a.clone(),
        pose: common::sample_pose(),
        ctx: common::sample_tracing_context(&room, &a),
    });
    assert!(
        ev.iter().any(|e| matches!(e, SyncerEvent::RateLimited { stream_kind } if *stream_kind == StreamKind::Pose)),
        "21st pose should be rate limited (pose)"
    );

    // 続けてChatを送ると、RateLimitedのstream_kindがChatになるはず
    let ev = syncer_a.handle(SyncerRequest::SendChat {
        chat: common::sample_chat(&a),
        ctx: TracingContext::for_chat(&room, &a),
    });
    assert!(
        ev.iter().any(|e| matches!(e, SyncerEvent::RateLimited { stream_kind } if *stream_kind == StreamKind::Chat)),
        "rate limited event should reflect latest stream kind (chat)"
    );
}
