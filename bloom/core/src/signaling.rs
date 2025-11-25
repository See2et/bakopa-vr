use std::collections::HashMap;

use bloom_api::ServerToClient;

use crate::ParticipantId;

/// Bloom内でシグナリングイベントを宛先参加者へ届けるための送信口。
pub trait DeliverySink {
    fn send(&mut self, to: &ParticipantId, message: ServerToClient);
}

/// テスト用のインメモリ送信口。誰に何を送ったかを記録する。
#[derive(Default, Debug)]
pub struct MockDeliverySink {
    pub sent: HashMap<ParticipantId, Vec<ServerToClient>>,
}

impl MockDeliverySink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn messages_for(&self, participant: &ParticipantId) -> Option<&[ServerToClient]> {
        self.sent.get(participant).map(Vec::as_slice)
    }
}

impl DeliverySink for MockDeliverySink {
    fn send(&mut self, to: &ParticipantId, message: ServerToClient) {
        self.sent.entry(to.clone()).or_default().push(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bloom_api::ServerToClient;

    #[test]
    fn mock_delivery_sink_records_messages_by_recipient() {
        let mut sink = MockDeliverySink::new();
        let p1 = ParticipantId::new();
        let p2 = ParticipantId::new();

        sink.send(
            &p1,
            ServerToClient::PeerConnected {
                participant_id: p2.to_string(),
            },
        );
        sink.send(
            &p1,
            ServerToClient::PeerDisconnected {
                participant_id: p2.to_string(),
            },
        );
        sink.send(
            &p2,
            ServerToClient::PeerConnected {
                participant_id: p1.to_string(),
            },
        );

        let to_p1 = sink.messages_for(&p1).expect("p1 should have messages");
        assert_eq!(to_p1.len(), 2, "p1への送信が2件ある");
        assert!(matches!(to_p1[0], ServerToClient::PeerConnected { .. }));
        assert!(matches!(to_p1[1], ServerToClient::PeerDisconnected { .. }));

        let to_p2 = sink.messages_for(&p2).expect("p2 should have messages");
        assert_eq!(to_p2.len(), 1, "p2への送信が1件ある");
        assert!(matches!(to_p2[0], ServerToClient::PeerConnected { .. }));
    }
}
