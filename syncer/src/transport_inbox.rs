use bloom_core::RoomId;
use tracing::warn;

use crate::{
    messages::SyncMessage, participant_table::ParticipantTable, StreamKind, SyncerError,
    SyncerEvent, TracingContext, TransportEvent, TransportPayload,
};

/// 受信したTransportEventをSyncerEventへ変換する小さなバッファ。
#[derive(Default, Clone)]
pub struct TransportInbox {
    events: Vec<TransportEvent>,
    failure_emitted: std::collections::HashSet<bloom_core::ParticipantId>,
}

impl TransportInbox {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            failure_emitted: std::collections::HashSet::new(),
        }
    }

    pub fn from_events(events: Vec<TransportEvent>) -> Self {
        Self {
            events,
            failure_emitted: std::collections::HashSet::new(),
        }
    }

    pub fn push(&mut self, ev: TransportEvent) {
        self.events.push(ev);
    }

    /// 受信イベントをパースし、SyncerEventへ変換して返す。
    pub fn drain_into_events(
        &mut self,
        room_id: &RoomId,
        participants: &mut ParticipantTable,
    ) -> Vec<SyncerEvent> {
        let mut out = Vec::new();
        let events = std::mem::take(&mut self.events);

        for event in events {
            match event {
                TransportEvent::Received { from, payload } => match payload {
                    TransportPayload::AudioFrame(frame) => {
                        let ctx = TracingContext {
                            room_id: room_id.clone(),
                            participant_id: from.clone(),
                            stream_kind: StreamKind::Voice,
                        };

                        out.push(SyncerEvent::VoiceFrameReceived { from, frame, ctx });
                    }
                    TransportPayload::Bytes(_) => {
                        let parsed = payload.parse_sync_message();
                        match parsed {
                            Ok(sync_msg) => {
                                let ctx = TracingContext {
                                    room_id: room_id.clone(),
                                    participant_id: from.clone(),
                                    stream_kind: stream_kind_of(&sync_msg),
                                };

                                match sync_msg {
                                    SyncMessage::Pose(pose) => {
                                        out.push(SyncerEvent::PoseReceived { from, pose, ctx })
                                    }
                                    SyncMessage::Chat(chat) => {
                                        out.push(SyncerEvent::ChatReceived { chat, ctx })
                                    }
                                    SyncMessage::Control(control) => {
                                        let pending = crate::PendingPeerEvent::from(control);
                                        let mut events =
                                            participants.apply_pending_peer_event(pending);
                                        out.append(&mut events);
                                    }
                                    SyncMessage::Signaling(_) => {
                                        out.push(control_or_signaling_error())
                                    }
                                }
                            }
                            Err(err) => out.push(SyncerEvent::Error {
                                kind: SyncerError::InvalidPayload(err),
                            }),
                        }
                    }
                },
                TransportEvent::Failure { peer } => {
                    // 同一peerのFailureはInbox全体で1度だけ扱う（重複発火防止）
                    if !self.failure_emitted.insert(peer.clone()) {
                        continue;
                    }

                    warn!(room_id = %room_id, participant_id = %peer, "transport failure observed; cleaning up peer");

                    let mut evs = participants.apply_leave(peer.clone());
                    if evs.is_empty() {
                        // 未登録でも「離脱した」というシグナルは1回だけ出す
                        evs.push(SyncerEvent::PeerLeft {
                            participant_id: peer,
                        });
                    }
                    out.extend(evs);
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

fn control_or_signaling_error() -> SyncerEvent {
    SyncerEvent::Error {
        kind: SyncerError::InvalidPayload(crate::messages::SyncMessageError::UnknownKind {
            value: "control_or_signaling_in_data_channel".to_string(),
        }),
    }
}
