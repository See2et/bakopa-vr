use std::collections::HashMap;

use crate::id::{ParticipantId, RoomId};

pub type ParticipantList = Vec<ParticipantId>;

const MAX_PARTICIPANTS: usize = 8;

#[derive(Clone, Debug)]
struct RoomState {
    /// 参加順を保持するためにVecを使用（仕様で順序が意味を持つ）。
    participants: ParticipantList,
}

#[derive(Default)]
pub struct RoomManager {
    rooms: HashMap<RoomId, RoomState>,
}

impl RoomManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// 新規Roomを作成し、作成者自身を最初の参加者として登録する。
    pub fn create_room(&mut self, room_owner: ParticipantId) -> CreateRoomResult {
        let room_id = RoomId::new();
        let self_id = room_owner;
        let participants = vec![self_id.clone()];

        let state = RoomState {
            participants: participants.clone(),
        };
        self.rooms.insert(room_id.clone(), state);

        CreateRoomResult {
            room_id,
            self_id,
            participants,
        }
    }

    /// 既存Roomに参加者を追加し、最新の参加者リストを返す。
    pub fn join_room(
        &mut self,
        room_id: &RoomId,
        participant: ParticipantId,
    ) -> Option<Result<ParticipantList, JoinRoomError>> {
        if let Some(room) = self.rooms.get_mut(room_id) {
            if room.participants.len() >= MAX_PARTICIPANTS
                && !room.participants.contains(&participant)
            {
                return Some(Err(JoinRoomError::RoomFull));
            }
            if !room.participants.contains(&participant) {
                room.participants.push(participant);
            }
            Some(Ok(room.participants.clone()))
        } else {
            None
        }
    }

    /// 指定参加者をRoomから離脱させ、最新の参加者リストを返す。
    ///
    /// 参加者が全員いなくなった場合はRoomを削除する。
    pub fn leave_room(
        &mut self,
        room_id: &RoomId,
        participant: &ParticipantId,
    ) -> Option<ParticipantList> {
        if let Some(room) = self.rooms.get_mut(room_id) {
            room.participants.retain(|p| p != participant);
            if room.participants.is_empty() {
                self.rooms.remove(room_id);
                return Some(vec![]);
            }
            return Some(room.participants.clone());
        }
        None
    }

    /// Roomの参加者一覧を取得する（存在しない場合None）。
    pub fn participants(&self, room_id: &RoomId) -> Option<ParticipantList> {
        self.rooms.get(room_id).map(|r| r.participants.clone())
    }
}

/// Room作成時の戻り値。
#[derive(Clone, Debug)]
pub struct CreateRoomResult {
    pub room_id: RoomId,
    pub self_id: ParticipantId,
    pub participants: ParticipantList,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JoinRoomError {
    RoomFull,
}
