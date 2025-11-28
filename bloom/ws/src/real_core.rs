use bloom_api::{ErrorCode, RelayIce, RelaySdp};
use bloom_core::{CreateRoomResult, JoinRoomError, ParticipantId, RoomId, RoomManager};
use bloom_core::signaling;

use crate::core_api::CoreApi;

/// シンプルなインメモリ実装の CoreApi。WSサーバ用の最小版。
pub struct RealCore {
    rooms: RoomManager,
}

impl RealCore {
    pub fn new() -> Self {
        Self {
            rooms: RoomManager::new(),
        }
    }
}

impl Default for RealCore {
    fn default() -> Self {
        Self::new()
    }
}

impl CoreApi for RealCore {
    fn create_room(&mut self, room_owner: ParticipantId) -> CreateRoomResult {
        self.rooms.create_room(room_owner)
    }

    fn join_room(
        &mut self,
        room_id: &RoomId,
        participant: ParticipantId,
    ) -> Option<Result<Vec<ParticipantId>, JoinRoomError>> {
        self.rooms.join_room(room_id, participant)
    }

    fn leave_room(
        &mut self,
        room_id: &RoomId,
        participant: &ParticipantId,
    ) -> Option<Vec<ParticipantId>> {
        self.rooms.leave_room(room_id, participant)
    }

    fn relay_offer(
        &mut self,
        room_id: &RoomId,
        from: &ParticipantId,
        to: &ParticipantId,
        payload: RelaySdp,
    ) -> Result<(), ErrorCode> {
        let participants = self
            .rooms
            .join_room(room_id, from.clone())
            .and_then(Result::ok)
            .unwrap_or_default();
        signaling::relay_offer_checked(&mut signaling::MockDeliverySink::default(), &participants, from, to, payload)
    }

    fn relay_answer(
        &mut self,
        room_id: &RoomId,
        from: &ParticipantId,
        to: &ParticipantId,
        payload: RelaySdp,
    ) -> Result<(), ErrorCode> {
        let participants = self
            .rooms
            .join_room(room_id, from.clone())
            .and_then(Result::ok)
            .unwrap_or_default();
        signaling::relay_answer_checked(&mut signaling::MockDeliverySink::default(), &participants, from, to, payload)
    }

    fn relay_ice_candidate(
        &mut self,
        room_id: &RoomId,
        from: &ParticipantId,
        to: &ParticipantId,
        payload: RelayIce,
    ) -> Result<(), ErrorCode> {
        let participants = self
            .rooms
            .join_room(room_id, from.clone())
            .and_then(Result::ok)
            .unwrap_or_default();
        signaling::relay_ice_candidate_checked(
            &mut signaling::MockDeliverySink::default(),
            &participants,
            from,
            to,
            payload,
        )
    }
}
