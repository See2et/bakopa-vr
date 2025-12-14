mod common;

use bloom_core::ParticipantId;
use common::sample_chat;
use syncer::{BasicSyncer, Syncer, SyncerEvent, SyncerRequest, messages::ChatMessage};

/// RED: RealWebrtcTransport をBasicSyncerに差し替え、実PC経路でチャット往復するE2E。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_webrtc_syncer_chat_roundtrip() {
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    // 実PC経路のTransportペア
    let (mut ta, mut tb) = syncer::webrtc_transport::RealWebrtcTransport::pair_with_datachannel_real(a.clone(), b.clone())
        .await
        .expect("pc setup");

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
}
