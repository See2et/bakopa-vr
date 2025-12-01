use bloom_api::{ErrorCode, RelayIce, RelaySdp};
use bloom_core::{CreateRoomResult, JoinRoomError, ParticipantId, RoomId};

use crate::core_api::{CoreApi, RelayAction};

/// Test helper core that returns predetermined values.
/// テストで参加者IDを動的に扱えるよう、create_room/join_room時に呼び出しごとのIDを反映する。
#[derive(Clone, Debug)]
pub struct MockCore {
    pub create_room_result: CreateRoomResult,
    pub create_room_calls: Vec<ParticipantId>,
    pub join_room_result: Option<Result<Vec<ParticipantId>, JoinRoomError>>,
    pub join_room_calls: Vec<(RoomId, ParticipantId)>,
    pub leave_room_result: Option<Vec<ParticipantId>>,
    pub leave_room_calls: Vec<(RoomId, ParticipantId)>,
    pub relay_offer_calls: Vec<(RoomId, ParticipantId, ParticipantId, RelaySdp)>,
    pub relay_offer_result: Option<Result<RelayAction, ErrorCode>>,
    pub relay_answer_calls: Vec<(RoomId, ParticipantId, ParticipantId, RelaySdp)>,
    pub relay_answer_result: Option<Result<RelayAction, ErrorCode>>,
    pub relay_ice_calls: Vec<(RoomId, ParticipantId, ParticipantId, RelayIce)>,
    pub relay_ice_result: Option<Result<RelayAction, ErrorCode>>,
    pub participants_map: std::collections::HashMap<RoomId, Vec<ParticipantId>>,
}

impl MockCore {
    pub fn new(create_room_result: CreateRoomResult) -> Self {
        Self {
            create_room_result,
            create_room_calls: Vec::new(),
            join_room_result: None,
            join_room_calls: Vec::new(),
            leave_room_result: None,
            leave_room_calls: Vec::new(),
            relay_offer_calls: Vec::new(),
            relay_offer_result: None,
            relay_answer_calls: Vec::new(),
            relay_answer_result: None,
            relay_ice_calls: Vec::new(),
            relay_ice_result: None,
            participants_map: std::collections::HashMap::new(),
        }
    }

    pub fn with_join_result(
        mut self,
        result: Option<Result<Vec<ParticipantId>, JoinRoomError>>,
    ) -> Self {
        self.join_room_result = result;
        self
    }

    pub fn with_leave_result(mut self, result: Option<Vec<ParticipantId>>) -> Self {
        self.leave_room_result = result;
        self
    }

    pub fn with_relay_offer_result(mut self, result: Result<RelayAction, ErrorCode>) -> Self {
        self.relay_offer_result = Some(result);
        self
    }

    pub fn with_relay_answer_result(mut self, result: Result<RelayAction, ErrorCode>) -> Self {
        self.relay_answer_result = Some(result);
        self
    }

    pub fn with_relay_ice_result(mut self, result: Result<RelayAction, ErrorCode>) -> Self {
        self.relay_ice_result = Some(result);
        self
    }

    pub fn with_participants(mut self, room_id: RoomId, participants: Vec<ParticipantId>) -> Self {
        self.participants_map.insert(room_id, participants);
        self
    }
}

impl CoreApi for MockCore {
    fn create_room(&mut self, room_owner: ParticipantId) -> CreateRoomResult {
        self.create_room_calls.push(room_owner.clone());
        let mut res = self.create_room_result.clone();
        // owner を self_id に反映し、participants にも含める
        res.self_id = room_owner.clone();
        if res.participants.is_empty() {
            res.participants.push(room_owner);
        }
        // 次回以降も同じ内容を返すように更新
        self.create_room_result = res.clone();
        res
    }

    fn join_room(
        &mut self,
        room_id: &RoomId,
        participant: ParticipantId,
    ) -> Option<Result<Vec<ParticipantId>, JoinRoomError>> {
        self.join_room_calls
            .push((room_id.clone(), participant.clone()));
        match self.join_room_result.clone() {
            Some(Ok(v)) if v.is_empty() => {
                // デフォルト: 既存self_idと参加者を返す
                let mut participants = self.create_room_result.participants.clone();
                if participants.is_empty() {
                    participants.push(self.create_room_result.self_id.clone());
                }
                participants.push(participant);
                Some(Ok(participants))
            }
            other => other,
        }
    }

    fn leave_room(
        &mut self,
        room_id: &RoomId,
        participant: &ParticipantId,
    ) -> Option<Vec<ParticipantId>> {
        self.leave_room_calls
            .push((room_id.clone(), participant.clone()));
        self.leave_room_result.clone()
    }

    fn participants(&self, room_id: &RoomId) -> Option<Vec<ParticipantId>> {
        self.participants_map.get(room_id).cloned()
    }

    fn relay_offer(
        &mut self,
        room_id: &RoomId,
        from: &ParticipantId,
        to: &ParticipantId,
        payload: RelaySdp,
    ) -> Result<RelayAction, ErrorCode> {
        let payload_clone = payload.clone();
        self.relay_offer_calls
            .push((room_id.clone(), from.clone(), to.clone(), payload));
        if let Some(result) = self.relay_offer_result.clone() {
            result
        } else {
            Ok(RelayAction {
                to: to.clone(),
                message: bloom_api::ServerToClient::Offer {
                    from: from.to_string(),
                    payload: payload_clone,
                },
            })
        }
    }

    fn relay_answer(
        &mut self,
        room_id: &RoomId,
        from: &ParticipantId,
        to: &ParticipantId,
        payload: RelaySdp,
    ) -> Result<RelayAction, ErrorCode> {
        let payload_clone = payload.clone();
        self.relay_answer_calls
            .push((room_id.clone(), from.clone(), to.clone(), payload));
        if let Some(result) = self.relay_answer_result.clone() {
            result
        } else {
            Ok(RelayAction {
                to: to.clone(),
                message: bloom_api::ServerToClient::Answer {
                    from: from.to_string(),
                    payload: payload_clone,
                },
            })
        }
    }

    fn relay_ice_candidate(
        &mut self,
        room_id: &RoomId,
        from: &ParticipantId,
        to: &ParticipantId,
        payload: RelayIce,
    ) -> Result<RelayAction, ErrorCode> {
        let payload_clone = payload.clone();
        self.relay_ice_calls
            .push((room_id.clone(), from.clone(), to.clone(), payload));
        if let Some(result) = self.relay_ice_result.clone() {
            result
        } else {
            Ok(RelayAction {
                to: to.clone(),
                message: bloom_api::ServerToClient::IceCandidate {
                    from: from.to_string(),
                    payload: payload_clone,
                },
            })
        }
    }
}
