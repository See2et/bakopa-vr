use std::collections::HashMap;

use bloom_api::{ErrorCode, RelayIce, RelaySdp, ServerToClient};

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
pub fn relay_offer(
    delivery: &mut impl DeliverySink,
    from: &ParticipantId,
    to: &ParticipantId,
    payload: RelaySdp,
) {
    let message = shape_offer_event(from, payload);
    delivery.send(to, message);
}

/// Answerを特定宛先へ1:1で配送する。
pub fn relay_answer(
    delivery: &mut impl DeliverySink,
    from: &ParticipantId,
    to: &ParticipantId,
    payload: RelaySdp,
) {
    let message = shape_answer_event(from, payload);
    delivery.send(to, message);
}

/// 受信者の存在確認付きでAnswerを配送する。
pub fn relay_answer_checked(
    delivery: &mut impl DeliverySink,
    participants: &[ParticipantId],
    from: &ParticipantId,
    to: &ParticipantId,
    payload: RelaySdp,
) -> Result<(), ErrorCode> {
    validate_membership(participants, from, to)?;
    relay_answer(delivery, from, to, payload);
    Ok(())
}

/// 受信者の存在確認付きでOfferを配送する。
/// 宛先が参加者リストに存在しない場合はParticipantNotFoundを返し、配送しない。
pub fn relay_offer_checked(
    delivery: &mut impl DeliverySink,
    participants: &[ParticipantId],
    from: &ParticipantId,
    to: &ParticipantId,
    payload: RelaySdp,
) -> Result<(), ErrorCode> {
    validate_membership(participants, from, to)?;
    relay_offer(delivery, from, to, payload);
    Ok(())
}

/// 送信者・宛先が参加者リストに含まれるか検証する。
fn validate_membership(
    participants: &[ParticipantId],
    from: &ParticipantId,
    to: &ParticipantId,
) -> Result<(), ErrorCode> {
    if participants.contains(from) && participants.contains(to) {
        Ok(())
    } else {
        Err(ErrorCode::ParticipantNotFound)
    }
}

/// Offerイベントの出力整形を一元化する。
/// 仕様: `from`フィールドを付与し、`to`は含めない。
pub fn shape_offer_event(from: &ParticipantId, payload: RelaySdp) -> ServerToClient {
    ServerToClient::Offer {
        from: from.to_string(),
        payload,
    }
}

/// Answerイベントの出力整形。
pub fn shape_answer_event(from: &ParticipantId, payload: RelaySdp) -> ServerToClient {
    ServerToClient::Answer {
        from: from.to_string(),
        payload,
    }
}

/// IceCandidateイベントの出力整形。
pub fn shape_ice_event(from: &ParticipantId, payload: RelayIce) -> ServerToClient {
    ServerToClient::IceCandidate {
        from: from.to_string(),
        payload,
    }
}

/// ICE candidate を特定宛先へ1:1で配送する。
pub fn relay_ice_candidate(
    delivery: &mut impl DeliverySink,
    from: &ParticipantId,
    to: &ParticipantId,
    payload: RelayIce,
) {
    let message = shape_ice_event(from, payload);
    delivery.send(to, message);
}

/// 受信者の存在確認付きでICE candidateを配送する。
pub fn relay_ice_candidate_checked(
    delivery: &mut impl DeliverySink,
    participants: &[ParticipantId],
    from: &ParticipantId,
    to: &ParticipantId,
    payload: RelayIce,
) -> Result<(), ErrorCode> {
    validate_membership(participants, from, to)?;
    relay_ice_candidate(delivery, from, to, payload);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bloom_api::ServerToClient;

    fn three_participants() -> (ParticipantId, ParticipantId, ParticipantId) {
        (
            ParticipantId::new(),
            ParticipantId::new(),
            ParticipantId::new(),
        )
    }

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
    fn shaped_offer_event_has_from_and_no_to() {
        let from_id = ParticipantId::new();
        let payload = RelaySdp {
            sdp: "v=0 offer".into(),
        };

        let shaped = shape_offer_event(&from_id, payload.clone());

        // fromが付与されていることを確認
        match shaped {
            ServerToClient::Offer {
                ref from,
                payload: ref p,
            } => {
                assert_eq!(from, &from_id.to_string());
                assert_eq!(p, &payload);
            }
            other => panic!("Offerイベントが生成されるはずが {:?} だった", other),
        }

        // JSONにシリアライズしても`to`フィールドが含まれないことを確認
        let json = serde_json::to_string(&shaped).expect("serialize shaped offer");
        assert!(!json.contains("\"to\""), "toフィールドは除去されているべき");
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
    fn relay_answer_checked_errors_when_recipient_missing() {
        let mut sink = MockDeliverySink::new();
        let (sender, receiver, missing) = three_participants();
        let participants = vec![sender.clone(), receiver.clone()];
        let payload = RelaySdp {
            sdp: "v=0 answer".into(),
        };

        let result =
            relay_answer_checked(&mut sink, &participants, &sender, &missing, payload.clone());

        assert_eq!(result, Err(ErrorCode::ParticipantNotFound));
        assert!(sink.messages_for(&receiver).is_none());
        assert!(sink.messages_for(&missing).is_none());
    }

    #[test]
    fn relay_answer_checked_errors_when_sender_missing() {
        let mut sink = MockDeliverySink::new();
        let (sender_missing, receiver, extra) = three_participants();
        let participants = vec![receiver.clone(), extra];
        let payload = RelaySdp {
            sdp: "v=0 answer".into(),
        };

        let result = relay_answer_checked(
            &mut sink,
            &participants,
            &sender_missing,
            &receiver,
            payload,
        );

        assert_eq!(result, Err(ErrorCode::ParticipantNotFound));
        assert!(sink.messages_for(&receiver).is_none());
        assert!(sink.messages_for(&sender_missing).is_none());
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

    #[test]
    fn relay_ice_candidate_checked_errors_when_recipient_missing() {
        let mut sink = MockDeliverySink::new();
        let (sender, receiver, missing) = three_participants();
        let participants = vec![sender.clone(), receiver.clone()];
        let payload = RelayIce {
            candidate: "cand1".into(),
        };

        let result = relay_ice_candidate_checked(
            &mut sink,
            &participants,
            &sender,
            &missing,
            payload.clone(),
        );

        assert_eq!(result, Err(ErrorCode::ParticipantNotFound));
        assert!(sink.messages_for(&receiver).is_none());
        assert!(sink.messages_for(&missing).is_none());
    }

    #[test]
    fn relay_ice_candidate_checked_errors_when_sender_missing() {
        let mut sink = MockDeliverySink::new();
        let (sender_missing, receiver, extra) = three_participants();
        let participants = vec![receiver.clone(), extra];
        let payload = RelayIce {
            candidate: "cand1".into(),
        };

        let result = relay_ice_candidate_checked(
            &mut sink,
            &participants,
            &sender_missing,
            &receiver,
            payload,
        );

        assert_eq!(result, Err(ErrorCode::ParticipantNotFound));
        assert!(sink.messages_for(&receiver).is_none());
        assert!(sink.messages_for(&sender_missing).is_none());
    }

    #[test]
    fn relay_offer_checked_returns_error_and_delivers_to_no_one_when_recipient_missing() {
        let mut sink = MockDeliverySink::new();
        let sender = ParticipantId::new();
        let existing = ParticipantId::new();
        let missing = ParticipantId::new();
        let participants = vec![sender.clone(), existing];
        let payload = RelaySdp {
            sdp: "v=0 offer".into(),
        };

        let result =
            relay_offer_checked(&mut sink, &participants, &sender, &missing, payload.clone());

        assert_eq!(
            result,
            Err(ErrorCode::ParticipantNotFound),
            "宛先不在ならParticipantNotFoundを返す"
        );

        // 宛先にも送信者にも配送されない
        assert!(
            sink.messages_for(&missing).is_none(),
            "不在宛先には何も届かない"
        );
        assert!(
            sink.messages_for(&sender).is_none(),
            "送信者にもループバックしない"
        );
    }

    #[test]
    fn relay_offer_checked_returns_error_when_sender_not_in_room() {
        let mut sink = MockDeliverySink::new();
        let sender_not_in_room = ParticipantId::new();
        let receiver_in_room = ParticipantId::new();
        let participants = vec![receiver_in_room.clone()];
        let payload = RelaySdp {
            sdp: "v=0 offer".into(),
        };

        let result = relay_offer_checked(
            &mut sink,
            &participants,
            &sender_not_in_room,
            &receiver_in_room,
            payload,
        );

        assert_eq!(
            result,
            Err(ErrorCode::ParticipantNotFound),
            "未参加送信者ならParticipantNotFoundを返す（専用コード未定のため流用）"
        );
        assert!(
            sink.messages_for(&receiver_in_room).is_none(),
            "宛先にも配送されない"
        );
        assert!(
            sink.messages_for(&sender_not_in_room).is_none(),
            "送信者にも配送されない"
        );
    }

    #[test]
    fn relay_offer_does_not_leak_to_other_participants() {
        let mut sink = MockDeliverySink::new();
        let (sender, receiver, bystander) = three_participants();

        // AとBがRoom参加者、Cは別の参加者として用意（配送先には指定しない）
        let participants = vec![sender.clone(), receiver.clone(), bystander.clone()];
        let payload = RelaySdp {
            sdp: "v=0 offer".into(),
        };

        // 宛先チェック付きでA→Bへ送る
        relay_offer_checked(
            &mut sink,
            &participants,
            &sender,
            &receiver,
            payload.clone(),
        )
        .expect("should deliver");

        // Bには届く
        let to_receiver = sink
            .messages_for(&receiver)
            .expect("receiver should get message");
        assert_eq!(to_receiver.len(), 1);
        assert!(matches!(to_receiver[0], ServerToClient::Offer { .. }));

        // Cには届かない
        assert!(
            sink.messages_for(&bystander).is_none(),
            "宛先以外の参加者に漏洩しない"
        );
    }
}
