mod common;

use bloom_core::ParticipantId;
use common::sample_chat;
use syncer::{messages::ChatMessage, BasicSyncer, Syncer, SyncerEvent, SyncerRequest};

/// RED: RealWebrtcTransport をBasicSyncerに差し替え、実PC経路でチャット往復するE2E。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_webrtc_syncer_chat_roundtrip() {
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    // 実PC経路のTransportペア
    let (mut ta, mut tb) =
        syncer::webrtc_transport::RealWebrtcTransport::pair_with_datachannel_real(
            a.clone(),
            b.clone(),
        )
        .await
        .expect("pc setup");
    let params_handle = ta.created_params_handle();

    let timeout = std::time::Duration::from_secs(5);
    ta.wait_data_channel_open(timeout).await.expect("open a");
    tb.wait_data_channel_open(timeout).await.expect("open b");

    let mut syncer_a = BasicSyncer::new(a.clone(), ta);
    let mut syncer_b = BasicSyncer::new(b.clone(), tb);

    // join
    let room = bloom_core::RoomId::new();
    syncer_a.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: a.clone(),
    });
    syncer_b.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: b.clone(),
    });
    // 相互に相手を登録してルーティング先を持たせる（暫定措置）
    syncer_a.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: b.clone(),
    });
    syncer_b.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: a.clone(),
    });

    // chat送信
    let chat: ChatMessage = sample_chat(&a);
    syncer_a.handle(SyncerRequest::SendChat {
        chat: chat.clone(),
        ctx: syncer::TracingContext::for_chat(&room, &a),
    });

    // pose送信も行い、パラメータ順序を検証
    syncer_a.handle(SyncerRequest::SendPose {
        from: a.clone(),
        pose: common::sample_pose(),
        ctx: syncer::TracingContext {
            room_id: room.clone(),
            participant_id: a.clone(),
            stream_kind: syncer::StreamKind::Pose,
        },
    });

    // 受信をポーリング
    let mut received = false;
    for _ in 0..60 {
        let events = syncer_b.handle(SyncerRequest::SendChat {
            chat: sample_chat(&b),
            ctx: syncer::TracingContext::for_chat(&room, &b),
        });
        for ev in events {
            if let SyncerEvent::ChatReceived { chat: c, .. } = ev {
                if c.message == chat.message && c.sender == chat.sender {
                    received = true;
                    break;
                }
            }
        }
        if received {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    assert!(received, "chat should arrive via real webrtc transport");

    // 送信パラメータがPose用unordered/unreliableとChat用ordered/reliableを含むことを確認
    let params = params_handle.lock().unwrap().clone();
    let pose_param = syncer::TransportSendParams::for_stream(syncer::StreamKind::Pose);
    let chat_param = syncer::TransportSendParams::for_stream(syncer::StreamKind::Chat);
    assert!(
        params.contains(&pose_param),
        "pose should use unordered/unreliable params on real transport"
    );
    assert!(
        params.contains(&chat_param),
        "chat should use ordered/reliable params on real transport"
    );
}
