use bloom_api::{ErrorCode, RelayIce, RelaySdp};
use bloom_core::{CreateRoomResult, JoinRoomError, ParticipantId, RoomId};

use crate::core_api::CoreApi;

/// Test helper core that returns predetermined values.
#[derive(Clone, Debug)]
pub struct MockCore {
    pub create_room_result: CreateRoomResult,
    pub create_room_calls: Vec<ParticipantId>,
    pub join_room_result: Option<Result<Vec<ParticipantId>, JoinRoomError>>,
    pub join_room_calls: Vec<(RoomId, ParticipantId)>,
    pub leave_room_result: Option<Vec<ParticipantId>>,
    pub leave_room_calls: Vec<(RoomId, ParticipantId)>,
    pub relay_offer_calls: Vec<(RoomId, ParticipantId, ParticipantId, RelaySdp)>,
    pub relay_offer_result: Result<(), ErrorCode>,
    pub relay_answer_calls: Vec<(RoomId, ParticipantId, ParticipantId, RelaySdp)>,
    pub relay_answer_result: Result<(), ErrorCode>,
    pub relay_ice_calls: Vec<(RoomId, ParticipantId, ParticipantId, RelayIce)>,
    pub relay_ice_result: Result<(), ErrorCode>,
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
            relay_offer_result: Ok(()),
            relay_answer_calls: Vec::new(),
            relay_answer_result: Ok(()),
            relay_ice_calls: Vec::new(),
            relay_ice_result: Ok(()),
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

    pub fn with_relay_offer_result(mut self, result: Result<(), ErrorCode>) -> Self {
        self.relay_offer_result = result;
        self
    }

    pub fn with_relay_answer_result(mut self, result: Result<(), ErrorCode>) -> Self {
        self.relay_answer_result = result;
        self
    }

    pub fn with_relay_ice_result(mut self, result: Result<(), ErrorCode>) -> Self {
        self.relay_ice_result = result;
        self
    }
}

impl CoreApi for MockCore {
    fn create_room(&mut self, room_owner: ParticipantId) -> CreateRoomResult {
        self.create_room_calls.push(room_owner.clone());
        let mut res = self.create_room_result.clone();
        // 上書きしてownerをself_idにする（テストでparticipant_idと揃えるため）
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
        self.join_room_calls.push((room_id.clone(), participant.clone()));
        match self.join_room_result.clone() {
            Some(Ok(mut v)) if v.is_empty() => {
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

    fn relay_offer(
        &mut self,
        room_id: &RoomId,
        from: &ParticipantId,
        to: &ParticipantId,
        payload: RelaySdp,
    ) -> Result<(), ErrorCode> {
        self.relay_offer_calls
            .push((room_id.clone(), from.clone(), to.clone(), payload));
        self.relay_offer_result.clone()
    }

    fn relay_answer(
        &mut self,
        room_id: &RoomId,
        from: &ParticipantId,
        to: &ParticipantId,
        payload: RelaySdp,
    ) -> Result<(), ErrorCode> {
        self.relay_answer_calls
            .push((room_id.clone(), from.clone(), to.clone(), payload));
        self.relay_answer_result.clone()
    }

    fn relay_ice_candidate(
        &mut self,
        room_id: &RoomId,
        from: &ParticipantId,
        to: &ParticipantId,
        payload: RelayIce,
    ) -> Result<(), ErrorCode> {
        self.relay_ice_calls
            .push((room_id.clone(), from.clone(), to.clone(), payload));
        self.relay_ice_result.clone()
    }
}
