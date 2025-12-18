mod common;

use std::time::Duration;

use bloom_core::{ParticipantId, RoomId};
use common::bus_transport::{new_bus, BusTransport};
use common::fake_clock::FakeClock;
use syncer::rate_limiter::RateLimiter;
use syncer::{BasicSyncer, StreamKind, Syncer, SyncerEvent, SyncerRequest, TracingContext};

/// RED→GREEN: 20件まで許容し、21件目で RateLimited を返し Transport 送信が増えない。
#[test]
fn chat_is_rate_limited_per_session_after_20_messages() {
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

    // join both peers to register participants
    syncer_a.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: a.clone(),
    });
    syncer_b.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: b.clone(),
    });

    // ControlJoin を相互に取り込んで参加者表を同期させる（送信は伴わない）
    let _ = syncer_a.poll_only();
    let _ = syncer_b.poll_only();
    bus.borrow_mut().messages.clear(); // カウンタをクリーンにして計測開始

    // 20件までは許容され、送信キューが増える
    for _ in 0..20 {
        let events = syncer_a.handle(SyncerRequest::SendChat {
            chat: common::sample_chat(&a),
            ctx: TracingContext::for_chat(&room, &a),
        });
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, SyncerEvent::RateLimited { .. })),
            "first 20 messages should not be rate limited"
        );
    }
    let sent_before = bus.borrow().messages.len();
    assert_eq!(
        sent_before, 20,
        "20 chats should have been enqueued to transport"
    );

    // 21件目でRateLimitedが返り、送信は増えない
    let events = syncer_a.handle(SyncerRequest::SendChat {
        chat: common::sample_chat(&a),
        ctx: TracingContext::for_chat(&room, &a),
    });
    assert!(
        events.iter().any(|e| matches!(e, SyncerEvent::RateLimited { stream_kind } if *stream_kind == StreamKind::Chat)),
        "21st message should return RateLimited(Chat)"
    );
    let sent_after = bus.borrow().messages.len();
    assert_eq!(
        sent_after, sent_before,
        "transport should not send when rate limited"
    );
}
