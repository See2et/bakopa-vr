use bloom_api::{RelayIce, RelaySdp};
use bloom_core::{CreateRoomResult, JoinRoomError, ParticipantId, RoomId};

/// Core domain API that the WebSocket layer depends on.
pub trait CoreApi {
    fn create_room(&mut self, room_owner: ParticipantId) -> CreateRoomResult;
    fn join_room(
        &mut self,
        room_id: &RoomId,
        participant: ParticipantId,
    ) -> Option<Result<Vec<ParticipantId>, JoinRoomError>>;
    fn leave_room(
        &mut self,
        room_id: &RoomId,
        participant: &ParticipantId,
    ) -> Option<Vec<ParticipantId>>;

    fn relay_offer(
        &mut self,
        room_id: &RoomId,
        from: &ParticipantId,
        to: &ParticipantId,
        payload: RelaySdp,
    ) -> Result<(), bloom_api::ErrorCode>;
    fn relay_answer(
        &mut self,
        room_id: &RoomId,
        from: &ParticipantId,
        to: &ParticipantId,
        payload: RelaySdp,
    ) -> Result<(), bloom_api::ErrorCode>;
    fn relay_ice_candidate(
        &mut self,
        room_id: &RoomId,
        from: &ParticipantId,
        to: &ParticipantId,
        payload: RelayIce,
    ) -> Result<(), bloom_api::ErrorCode>;
}

/// Coreからハンドラへ流れてくるイベントを受け取るためのフック。
#[allow(dead_code)]
pub trait CoreEventReceiver {
    fn on_peer_connected(&mut self, participants: &[ParticipantId], joined: &ParticipantId);
    fn on_peer_disconnected(&mut self, participants: &[ParticipantId], left: &ParticipantId);
    fn on_participants_updated(&mut self, room_id: &RoomId, participants: &[ParticipantId]);
}
