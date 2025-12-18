mod common;

use bloom_core::{ParticipantId, RoomId};
use common::{sample_chat, timeouts};
use syncer::{
    webrtc_transport::WebrtcTransport, BasicSyncer, Syncer, SyncerEvent, SyncerRequest,
    TracingContext,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn chat_roundtrip_over_webrtc_transport() {
    let room = RoomId::new();
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    let (ta, tb) = WebrtcTransport::pair(a.clone(), b.clone());

    let mut syncer_a = BasicSyncer::new(a.clone(), ta);
    let mut syncer_b = BasicSyncer::new(b.clone(), tb);

    // join room (現時点ではローカル登録のみ)
    syncer_a.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: a.clone(),
    });
    syncer_b.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: b.clone(),
    });

    // A -> B にチャットを送信
    let chat = sample_chat(&a);
    syncer_a.handle(SyncerRequest::SendChat {
        chat: chat.clone(),
        ctx: TracingContext::for_chat(&room, &a),
    });

    // B 側で受信を待つ（短時間タイムアウト）
    let deadline = tokio::time::Instant::now() + timeouts::WAIT_TIMEOUT;
    let mut received = None;
    while tokio::time::Instant::now() < deadline {
        let events = syncer_b.handle(SyncerRequest::SendChat {
            chat: sample_chat(&b),
            ctx: TracingContext::for_chat(&room, &b),
        });

        if let Some(ev) = events.into_iter().find_map(|e| match e {
            SyncerEvent::ChatReceived { chat, ctx } => Some((chat, ctx)),
            _ => None,
        }) {
            received = Some(ev);
            break;
        }

        tokio::time::sleep(timeouts::POLL_INTERVAL).await;
    }

    let (chat, ctx) = received.expect("chat should be delivered over WebRTC transport");
    assert_eq!(ctx.participant_id, a);
    assert_eq!(chat.sender, a.to_string());
}
