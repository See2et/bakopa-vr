use std::cell::RefCell;
use std::rc::Rc;
use std::collections::HashSet;

mod mock_bus;
pub mod signaling_hub;

use bloom_core::ParticipantId;
use anyhow::Result;

use crate::{Transport, TransportEvent, TransportPayload, TransportSendParams};

#[derive(Default, Debug)]
struct WebrtcBus {
    messages: Vec<(ParticipantId, ParticipantId, TransportPayload)>, // (to, from, payload)
}

#[derive(Default, Debug)]
struct WebrtcState {
    sent_params: Vec<crate::TransportSendParams>,
    pending: Vec<crate::TransportEvent>,
    inject_failure_once: bool,
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

#[derive(Debug, Clone, Copy)]
pub struct WebrtcTransportOptions {
    pub inject_failure_once: bool,
}

impl Default for WebrtcTransportOptions {
    fn default() -> Self {
        Self {
            inject_failure_once: false,
        }
    }
}

/// 実WebRTC実装の土台となるアダプタ。現時点ではPCを保持するだけのスタブ。
pub struct RealWebrtcTransport {
    me: ParticipantId,
    pc_present: bool,
    open_channels: HashSet<String>,
    bus: mock_bus::MockBus,
    peer: Option<ParticipantId>,
}

impl RealWebrtcTransport {
    pub fn new(me: ParticipantId, _ice_servers: Vec<String>) -> Result<Self> {
        // 本実装時にはpeerはシグナリングでセットされる。いまはNone。
        let (bus, _peer_bus) = mock_bus::MockBus::new_shared();
        Ok(Self {
            me,
            pc_present: true,
            open_channels: HashSet::from(["sutera-data".to_string()]), // 仮でopen扱い
            bus,
            peer: None,
        })
    }

    pub fn pair_for_tests(a: ParticipantId, b: ParticipantId) -> (Self, Self) {
        let (bus_a, bus_b) = mock_bus::MockBus::new_shared();
        (
            Self {
                me: a.clone(),
                pc_present: true,
                open_channels: HashSet::from(["sutera-data".to_string()]),
                bus: bus_a,
                peer: Some(b.clone()),
            },
            Self {
                me: b,
                pc_present: true,
                open_channels: HashSet::from(["sutera-data".to_string()]),
                bus: bus_b,
                peer: Some(a),
            },
        )
    }

    pub fn has_peer_connection(&self) -> bool {
        self.pc_present
    }

    /// 仮実装: sutera-dataチャネルがopen済みかを返す（現状は即true）。
    pub fn has_data_channel_open(&self, label: &str) -> bool {
        self.open_channels.contains(label)
    }
}

impl Transport for RealWebrtcTransport {
    fn register_participant(&mut self, _participant: ParticipantId) {
        // TODO: 実装時にPCへの登録などを行う
    }

    fn send(&mut self, to: ParticipantId, payload: TransportPayload, _params: TransportSendParams) {
        if let Some(peer) = &self.peer {
            if &to == peer {
                self.bus.push(to, self.me.clone(), payload);
            }
        }
    }

    fn poll(&mut self) -> Vec<TransportEvent> {
        self.bus.drain_for(&self.me)
    }
}

impl WebrtcTransport {
    fn new(
        me: ParticipantId,
        peer: ParticipantId,
        bus: Rc<RefCell<WebrtcBus>>,
        opts: WebrtcTransportOptions,
    ) -> Self {
        Self {
            me,
            peer,
            bus,
            registered: false,
            state: Rc::new(RefCell::new(WebrtcState {
                sent_params: Vec::new(),
                pending: Vec::new(),
                inject_failure_once: opts.inject_failure_once,
            })),
        }
    }

    /// in-processで2ピア分のTransportを生成するためのヘルパー。
    /// 将来、ここを実WebRTC初期化に置き換える。
    pub fn pair(a: ParticipantId, b: ParticipantId) -> (Self, Self) {
        Self::pair_with_options(a, b, WebrtcTransportOptions::default(), WebrtcTransportOptions::default())
    }

    pub fn pair_with_options(
        a: ParticipantId,
        b: ParticipantId,
        opts_a: WebrtcTransportOptions,
        opts_b: WebrtcTransportOptions,
    ) -> (Self, Self) {
        let bus = Rc::new(RefCell::new(WebrtcBus::default()));
        (
            Self::new(a.clone(), b.clone(), bus.clone(), opts_a),
            Self::new(b, a, bus, opts_b),
        )
    }

    /// 送信時に使用されたチャネルパラメータの記録を取得（テスト用）。
    pub fn sent_params(&self) -> Vec<crate::TransportSendParams> {
        self.state.borrow().sent_params.clone()
    }
}

impl Transport for WebrtcTransport {
    fn register_participant(&mut self, participant: ParticipantId) {
        // 単純なフラグのみ。バス側には現状登録情報を残さない。
        if participant == self.me {
            self.registered = true;
        }
    }

    fn send(&mut self, _to: ParticipantId, payload: TransportPayload, params: TransportSendParams) {
        if !self.registered {
            return; // 登録前は送信しない（FilteringTransportと整合）
        }

        // 渡された送信パラメータを記録
        self.state.borrow_mut().sent_params.push(params);

        // 初回送信時に通信失敗をシミュレートし、自分宛にFailureイベントを積む。
        {
            let mut state = self.state.borrow_mut();
            if state.inject_failure_once {
                state.pending.push(crate::TransportEvent::Failure {
                    peer: self.peer.clone(),
                });
                state.inject_failure_once = false;
            }
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

        let mut out = {
            let mut state = self.state.borrow_mut();
            std::mem::take(&mut state.pending)
        };

        let mut bus = self.bus.borrow_mut();
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
