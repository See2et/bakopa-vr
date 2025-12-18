mod common;

use std::time::Duration;

use bloom_core::{ParticipantId, RoomId};
use common::bus_transport::{new_bus, BusTransport};
use common::fake_clock::FakeClock;
use syncer::rate_limiter::RateLimiter;
use syncer::{BasicSyncer, Syncer, SyncerEvent, SyncerRequest, TracingContext};

/// RED→GREEN: 21件目でRateLimited、1.1秒進めると再び許可される。
#[test]
fn rate_limit_resets_after_window_elapsed() {
    let room = RoomId::new();
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    let bus = new_bus();

    let clock = FakeClock::new(std::time::Instant::now());
    let limiter = RateLimiter::with_clock(20, Duration::from_secs(1), clock.clone());

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

    // 20件は許容
    for _ in 0..20 {
        let ev = syncer_a.handle(SyncerRequest::SendChat {
            chat: common::sample_chat(&a),
            ctx: TracingContext::for_chat(&room, &a),
        });
        assert!(
            !ev.iter()
                .any(|e| matches!(e, SyncerEvent::RateLimited { .. })),
            "first 20 should be allowed"
        );
    }
    let sent_before = bus.borrow().messages.len();
    assert_eq!(sent_before, 20, "20 messages should be sent before limit");

    // 21件目はRateLimitedで送信増えない
    let ev = syncer_a.handle(SyncerRequest::SendChat {
        chat: common::sample_chat(&a),
        ctx: TracingContext::for_chat(&room, &a),
    });
    assert!(
        ev.iter()
            .any(|e| matches!(e, SyncerEvent::RateLimited { .. })),
        "21st should be rate limited"
    );
    assert_eq!(
        bus.borrow().messages.len(),
        sent_before,
        "should not send when rate limited"
    );

    // 1.1秒進めてリセット
    clock.advance(Duration::from_millis(1100));

    let ev = syncer_a.handle(SyncerRequest::SendChat {
        chat: common::sample_chat(&a),
        ctx: TracingContext::for_chat(&room, &a),
    });
    assert!(
        !ev.iter()
            .any(|e| matches!(e, SyncerEvent::RateLimited { .. })),
        "after window reset, should be allowed"
    );
    assert_eq!(
        bus.borrow().messages.len(),
        sent_before + 1,
        "one message should be sent after reset"
    );
}
