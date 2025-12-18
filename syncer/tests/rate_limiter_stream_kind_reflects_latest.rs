use std::time::Duration;

use syncer::{
    rate_limiter::{RateLimitDecision, RateLimiter},
    StreamKind,
};

#[test]
fn rate_limited_reports_latest_stream_kind() {
    // Poseで上限到達後、次に送るChatがRateLimitedになり、そのstream_kindがChatで返ること。
    let mut limiter = RateLimiter::new(20, Duration::from_secs(1));
    let session_id = "ipc-session-kind";

    for _ in 0..20 {
        assert_eq!(
            limiter.check_and_record(session_id, StreamKind::Pose),
            RateLimitDecision::Allowed
        );
    }

    let decision = limiter.check_and_record(session_id, StreamKind::Chat);
    assert_eq!(
        decision,
        RateLimitDecision::RateLimited {
            stream_kind: StreamKind::Chat
        },
        "RateLimited should reflect the stream kind of the triggering request"
    );
}
