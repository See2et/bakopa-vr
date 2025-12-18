mod common;

use bloom_core::ParticipantId;
use syncer::{
    webrtc_transport::RealWebrtcTransport, BasicSyncer, StreamKind, Syncer, SyncerEvent,
    SyncerRequest,
};

/// RED: 実PC経路で受信イベントのTracingContextが送信者/stream_kindに一致することを確認する。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tracing_context_matches_sender_and_stream_kind_real_transport() {
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    let (mut ta, mut tb) = RealWebrtcTransport::pair_with_datachannel_real(a.clone(), b.clone())
        .await
        .expect("pc setup");

    let timeout = std::time::Duration::from_secs(5);
    ta.wait_data_channel_open(timeout).await.expect("open a");
    tb.wait_data_channel_open(timeout).await.expect("open b");

    let room = bloom_core::RoomId::new();
    let mut syncer_a = BasicSyncer::new(a.clone(), ta);
    let mut syncer_b = BasicSyncer::new(b.clone(), tb);

    // join both peers
    syncer_a.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: a.clone(),
    });
    let mut ev_b = syncer_b.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: b.clone(),
    });
    let mut ev_a = Vec::new();

    // 互いにPeerJoinedが届くまで軽くポーリング
    let mut a_seen_b = ev_a
        .iter()
        .any(|e| matches!(e, SyncerEvent::PeerJoined { participant_id } if participant_id == &b));
    let mut b_seen_a = ev_b
        .iter()
        .any(|e| matches!(e, SyncerEvent::PeerJoined { participant_id } if participant_id == &a));
    for _ in 0..60 {
        if a_seen_b && b_seen_a {
            break;
        }
        ev_a = syncer_a.handle(SyncerRequest::SendChat {
            chat: common::sample_chat(&a),
            ctx: syncer::TracingContext::for_chat(&room, &a),
        });
        ev_b = syncer_b.handle(SyncerRequest::SendChat {
            chat: common::sample_chat(&b),
            ctx: syncer::TracingContext::for_chat(&room, &b),
        });
        a_seen_b |= ev_a.iter().any(
            |e| matches!(e, SyncerEvent::PeerJoined { participant_id } if participant_id == &b),
        );
        b_seen_a |= ev_b.iter().any(
            |e| matches!(e, SyncerEvent::PeerJoined { participant_id } if participant_id == &a),
        );
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(
        a_seen_b && b_seen_a,
        "peers should observe each other's join before messaging"
    );

    // 本番のChat送信
    let chat = common::sample_chat(&a);
    syncer_a.handle(SyncerRequest::SendChat {
        chat: chat.clone(),
        ctx: syncer::TracingContext::for_chat(&room, &a),
    });

    // 本番のPose送信
    let pose = common::sample_pose();
    syncer_a.handle(SyncerRequest::SendPose {
        from: a.clone(),
        pose: pose.clone(),
        ctx: common::sample_tracing_context(&room, &a),
    });

    // B側で受信をポーリング
    let mut chat_ctx_ok = false;
    let mut pose_ctx_ok = false;
    for _ in 0..40 {
        let events = syncer_b.handle(SyncerRequest::SendChat {
            chat: common::sample_chat(&b),
            ctx: syncer::TracingContext::for_chat(&room, &b),
        });
        for ev in events {
            match ev {
                SyncerEvent::ChatReceived { chat: recv, ctx } => {
                    if recv.message == chat.message && recv.sender == chat.sender {
                        chat_ctx_ok = ctx.room_id == room
                            && ctx.participant_id == a
                            && ctx.stream_kind == StreamKind::Chat;
                    }
                }
                SyncerEvent::PoseReceived { from, pose: p, ctx } => {
                    if from == a && p == pose {
                        pose_ctx_ok = ctx.room_id == room
                            && ctx.participant_id == a
                            && ctx.stream_kind == StreamKind::Pose;
                    }
                }
                _ => {}
            }
        }
        if chat_ctx_ok && pose_ctx_ok {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    assert!(
        chat_ctx_ok,
        "chat tracing context should match sender and stream_kind"
    );
    assert!(
        pose_ctx_ok,
        "pose tracing context should match sender and stream_kind"
    );
}
