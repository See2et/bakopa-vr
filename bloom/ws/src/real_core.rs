use bloom_api::{ErrorCode, RelayIce, RelaySdp};
use bloom_core::signaling;
use bloom_core::{CreateRoomResult, JoinRoomError, ParticipantId, RoomId, RoomManager};

use crate::core_api::{CoreApi, RelayAction};

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

    fn participants(&self, room_id: &RoomId) -> Option<Vec<ParticipantId>> {
        self.rooms.participants(room_id)
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
    ) -> Result<RelayAction, ErrorCode> {
        let participants = self
            .rooms
            .participants(room_id)
            .ok_or(ErrorCode::ParticipantNotFound)?;
        if !participants.contains(from) || !participants.contains(to) {
            return Err(ErrorCode::ParticipantNotFound);
        }
        let message = signaling::shape_offer_event(from, payload);
        Ok(RelayAction {
            to: to.clone(),
            message,
        })
    }

    fn relay_answer(
        &mut self,
        room_id: &RoomId,
        from: &ParticipantId,
        to: &ParticipantId,
        payload: RelaySdp,
    ) -> Result<RelayAction, ErrorCode> {
        let participants = self
            .rooms
            .participants(room_id)
            .ok_or(ErrorCode::ParticipantNotFound)?;
        if !participants.contains(from) || !participants.contains(to) {
            return Err(ErrorCode::ParticipantNotFound);
        }
        let message = signaling::shape_answer_event(from, payload);
        Ok(RelayAction {
            to: to.clone(),
            message,
        })
    }

    fn relay_ice_candidate(
        &mut self,
        room_id: &RoomId,
        from: &ParticipantId,
        to: &ParticipantId,
        payload: RelayIce,
    ) -> Result<RelayAction, ErrorCode> {
        let participants = self
            .rooms
            .participants(room_id)
            .ok_or(ErrorCode::ParticipantNotFound)?;
        if !participants.contains(from) || !participants.contains(to) {
            return Err(ErrorCode::ParticipantNotFound);
        }
        let message = signaling::shape_ice_event(from, payload);
        Ok(RelayAction {
            to: to.clone(),
            message,
        })
    }
}
