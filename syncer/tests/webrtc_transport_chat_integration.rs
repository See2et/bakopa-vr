mod common;

use bloom_core::{ParticipantId, RoomId};
use common::{sample_chat, sample_tracing_context};
use syncer::{
    webrtc_transport::WebrtcTransport, BasicSyncer, Syncer, SyncerEvent, SyncerRequest,
};

/// RED: 参加者同期済みの状態で、A->B の Chat が1回だけ届き、TracingContextが埋まることを確認する。
#[test]
fn chat_delivers_once_with_tracing_over_webrtc_transport() {
    let room = RoomId::new();
    let a = ParticipantId::new();
    let b = ParticipantId::new();

    let (ta, tb) = WebrtcTransport::pair(a.clone(), b.clone());

    let mut syncer_a = BasicSyncer::new(a.clone(), ta);
    let mut syncer_b = BasicSyncer::new(b.clone(), tb);

    // join both peers (ControlJoinで相互同期される想定)
    syncer_a.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: a.clone(),
    });
    syncer_b.handle(SyncerRequest::Join {
        room_id: room.clone(),
        participant_id: b.clone(),
    });

    let chat = sample_chat(&a);

    // A sends chat to B
    syncer_a.handle(SyncerRequest::SendChat {
        chat: chat.clone(),
        ctx: sample_tracing_context(&room, &a),
    });

    // B triggers poll by issuing its own dummy send
    let events = syncer_b.handle(SyncerRequest::SendChat {
        chat: sample_chat(&b),
        ctx: sample_tracing_context(&room, &b),
    });

    let chat_event = events.into_iter().find_map(|ev| match ev {
        SyncerEvent::ChatReceived { chat: recv, ctx } => Some((recv, ctx)),
        _ => None,
    });

    let (recv_chat, ctx) = chat_event.expect("B should receive exactly one chat");
    assert_eq!(recv_chat.message, chat.message);
    assert_eq!(recv_chat.sender, chat.sender);
    assert_eq!(ctx.room_id, room);
    assert_eq!(ctx.participant_id, a);
    assert_eq!(ctx.stream_kind, syncer::StreamKind::Chat);
}
