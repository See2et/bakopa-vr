use std::cell::RefCell;
use std::rc::Rc;

use bloom_core::ParticipantId;

use crate::{
    messages::SyncMessage,
    Transport, TransportEvent, TransportPayload, TransportSendParams, StreamKind,
};

#[derive(Default, Debug)]
struct WebrtcBus {
    messages: Vec<(ParticipantId, ParticipantId, TransportPayload)>, // (to, from, payload)
}

#[derive(Default, Debug)]
struct WebrtcState {
    sent_params: Vec<crate::TransportSendParams>,
}

/// 最小動作のためのin-process WebRTC風Transport。
/// 現段階ではSignal/ICEなしで、ペア内の相互配送のみを提供する。
#[derive(Clone, Debug)]
pub struct WebrtcTransport {
    me: ParticipantId,
    peer: ParticipantId,
    bus: Rc<RefCell<WebrtcBus>>, // シェアされたメモリバス
    registered: bool,
    state: Rc<RefCell<WebrtcState>>, // テスト用の観測ポイント
}

impl WebrtcTransport {
    fn new(me: ParticipantId, peer: ParticipantId, bus: Rc<RefCell<WebrtcBus>>) -> Self {
        Self {
            me,
            peer,
            bus,
            registered: false,
            state: Rc::new(RefCell::new(WebrtcState::default())),
        }
    }

    /// in-processで2ピア分のTransportを生成するためのヘルパー。
    /// 将来、ここを実WebRTC初期化に置き換える。
    pub fn pair(a: ParticipantId, b: ParticipantId) -> (Self, Self) {
        let bus = Rc::new(RefCell::new(WebrtcBus::default()));
        let ta_state = Rc::new(RefCell::new(WebrtcState::default()));
        let tb_state = Rc::new(RefCell::new(WebrtcState::default()));

        (
            Self {
                me: a.clone(),
                peer: b.clone(),
                bus: bus.clone(),
                registered: false,
                state: ta_state,
            },
            Self {
                me: b,
                peer: a,
                bus,
                registered: false,
                state: tb_state,
            },
        )
    }

    /// 送信時に使用されたチャネルパラメータの記録を取得（テスト用）。
    pub fn sent_params(&self) -> Vec<crate::TransportSendParams> {
        self.state.borrow().sent_params.clone()
    }
}

fn stream_kind_from_payload(payload: &TransportPayload) -> Option<StreamKind> {
    match payload {
        TransportPayload::AudioFrame(_) => Some(StreamKind::Voice),
        TransportPayload::Bytes(_) => match payload.parse_sync_message() {
            Ok(SyncMessage::Pose(_)) => Some(StreamKind::Pose),
            Ok(SyncMessage::Chat(_)) => Some(StreamKind::Chat),
            Ok(SyncMessage::Control(control)) => Some(control.kind_stream()),
            Ok(SyncMessage::Signaling(sig)) => Some(sig.kind_stream()),
            Err(_) => None,
        },
    }
}

impl Transport for WebrtcTransport {
    fn register_participant(&mut self, participant: ParticipantId) {
        // 単純なフラグのみ。バス側には現状登録情報を残さない。
        if participant == self.me {
            self.registered = true;
        }
    }

    fn send(&mut self, _to: ParticipantId, payload: TransportPayload) {
        if !self.registered {
            return; // 登録前は送信しない（FilteringTransportと整合）
        }

        // ペイロードから StreamKind を推定し、送信用パラメータを記録する。
        if let Some(kind) = stream_kind_from_payload(&payload) {
            let params = TransportSendParams::for_stream(kind);
            self.state.borrow_mut().sent_params.push(params);
        }

        // 相手ピアに無条件で配送する（現段階では単一ピアのみサポート）。
        let mut bus = self.bus.borrow_mut();
        bus.messages
            .push((self.peer.clone(), self.me.clone(), payload));
    }

    fn poll(&mut self) -> Vec<TransportEvent> {
        if !self.registered {
            return Vec::new();
        }

        let mut bus = self.bus.borrow_mut();
        let mut out = Vec::new();
        let mut i = 0;
        while i < bus.messages.len() {
            if bus.messages[i].0 == self.me {
                let (_to, from, payload) = bus.messages.remove(i);
                out.push(TransportEvent::Received { from, payload });
            } else {
                i += 1;
            }
        }
        out
    }
}
