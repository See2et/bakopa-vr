use bloom_core::RoomId;

use crate::{
    messages::SyncMessage,
    participant_table::ParticipantTable,
    StreamKind, SyncerError, SyncerEvent, TracingContext, TransportEvent,
};

/// 受信したTransportEventをSyncerEventへ変換する小さなバッファ。
#[derive(Default, Clone)]
pub struct TransportInbox {
    events: Vec<TransportEvent>,
}

impl TransportInbox {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn from_events(events: Vec<TransportEvent>) -> Self {
        Self { events }
    }

    pub fn push(&mut self, ev: TransportEvent) {
        self.events.push(ev);
    }

    /// 受信イベントをパースし、SyncerEventへ変換して返す。
    pub fn drain_into_events(
        &mut self,
        room_id: &RoomId,
        _participants: &ParticipantTable,
    ) -> Vec<SyncerEvent> {
        let mut out = Vec::new();

        let events = std::mem::take(&mut self.events);

        for event in events {
            match event {
                TransportEvent::Received { from, payload } => {
                    let parsed = payload.parse_sync_message();
                    match parsed {
                        Ok(sync_msg) => {
                            let ctx = TracingContext {
                                room_id: room_id.clone(),
                                participant_id: from.clone(),
                                stream_kind: stream_kind_of(&sync_msg),
                            };

                            match sync_msg {
                                SyncMessage::Pose(pose) => out.push(SyncerEvent::PoseReceived {
                                    from,
                                    pose,
                                    ctx,
                                }),
                                SyncMessage::Chat(chat) => out.push(SyncerEvent::ChatReceived {
                                    chat,
                                    ctx,
                                }),
                                SyncMessage::Control(_) | SyncMessage::Signaling(_) => {
                                    out.push(SyncerEvent::Error {
                                        kind: SyncerError::InvalidPayload(
                                            crate::messages::SyncMessageError::UnknownKind {
                                                value: "control_or_signaling_in_data_channel"
                                                    .to_string(),
                                            },
                                        ),
                                    })
                                }
                            }
                        }
                        Err(err) => out.push(SyncerEvent::Error {
                            kind: SyncerError::InvalidPayload(err),
                        }),
                    }
                }
            }
        }

        out
    }
}

fn stream_kind_of(msg: &SyncMessage) -> StreamKind {
    match msg {
        SyncMessage::Pose(_) => StreamKind::Pose,
        SyncMessage::Chat(_) => StreamKind::Chat,
        SyncMessage::Control(control) => control.kind_stream(),
        SyncMessage::Signaling(signaling) => signaling.kind_stream(),
    }
}
