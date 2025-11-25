use std::collections::HashMap;

use bloom_api::{RelayIce, RelaySdp, ServerToClient};

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

/// Offerを特定宛先へ1:1で配送する。
/// 現在はフェーズ1（Red）用に中身は未実装。
pub fn relay_offer(
    delivery: &mut impl DeliverySink,
    from: &ParticipantId,
    to: &ParticipantId,
    payload: RelaySdp,
) {
    let message = ServerToClient::Offer {
        from: from.to_string(),
        payload,
    };
    delivery.send(to, message);
}

/// Answerを特定宛先へ1:1で配送する。
/// 現在はフェーズ1（Red）用に中身は未実装。
pub fn relay_answer(
    delivery: &mut impl DeliverySink,
    from: &ParticipantId,
    to: &ParticipantId,
    payload: RelaySdp,
) {
    let message = ServerToClient::Answer {
        from: from.to_string(),
        payload,
    };
    delivery.send(to, message);
}

/// ICE candidate を特定宛先へ1:1で配送する。
/// 現在はフェーズ1（Red）用に中身は未実装。
pub fn relay_ice_candidate(
    delivery: &mut impl DeliverySink,
    from: &ParticipantId,
    to: &ParticipantId,
    payload: RelayIce,
) {
    let message = ServerToClient::IceCandidate {
        from: from.to_string(),
        payload,
    };
    delivery.send(to, message);
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

    #[test]
    fn relay_offer_sends_only_to_target_peer() {
        let mut sink = MockDeliverySink::new();
        let sender = ParticipantId::new();
        let receiver = ParticipantId::new();
        let payload = RelaySdp {
            sdp: "v=0 offer".into(),
        };

        relay_offer(&mut sink, &sender, &receiver, payload.clone());

        // 宛先BにはOfferが1件届く
        let to_receiver = sink
            .messages_for(&receiver)
            .expect("receiver should get message");
        assert_eq!(to_receiver.len(), 1, "Bには1件だけ届く");
        assert_eq!(
            to_receiver[0],
            ServerToClient::Offer {
                from: sender.to_string(),
                payload
            }
        );

        // 送信者Aには配送されない
        assert!(
            sink.messages_for(&sender).is_none(),
            "送信者自身にメッセージが返らないこと"
        );
    }

    #[test]
    fn relay_answer_sends_only_to_target_peer() {
        let mut sink = MockDeliverySink::new();
        let sender = ParticipantId::new();
        let receiver = ParticipantId::new();
        let payload = RelaySdp {
            sdp: "v=0 answer".into(),
        };

        relay_answer(&mut sink, &sender, &receiver, payload.clone());

        // 宛先BにはAnswerが1件届く
        let to_receiver = sink
            .messages_for(&receiver)
            .expect("receiver should get message");
        assert_eq!(to_receiver.len(), 1, "Bには1件だけ届く");
        assert_eq!(
            to_receiver[0],
            ServerToClient::Answer {
                from: sender.to_string(),
                payload
            }
        );

        // 送信者Aには配送されない
        assert!(
            sink.messages_for(&sender).is_none(),
            "送信者自身にメッセージが返らないこと"
        );
    }

    #[test]
    fn relay_ice_candidate_sends_only_to_target_peer() {
        let mut sink = MockDeliverySink::new();
        let sender = ParticipantId::new();
        let receiver = ParticipantId::new();
        let payload = RelayIce {
            candidate: "cand1".into(),
        };

        relay_ice_candidate(&mut sink, &sender, &receiver, payload.clone());

        // 宛先BにはIceCandidateが1件届く
        let to_receiver = sink
            .messages_for(&receiver)
            .expect("receiver should get message");
        assert_eq!(to_receiver.len(), 1, "Bには1件だけ届く");
        assert_eq!(
            to_receiver[0],
            ServerToClient::IceCandidate {
                from: sender.to_string(),
                payload
            }
        );

        // 送信者Aには配送されない
        assert!(
            sink.messages_for(&sender).is_none(),
            "送信者自身にメッセージが返らないこと"
        );
    }
}
