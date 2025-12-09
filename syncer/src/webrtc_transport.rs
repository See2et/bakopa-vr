use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;

mod mock_bus;
pub mod signaling_hub;

use bloom_core::ParticipantId;
use anyhow::Result;
use tokio::sync::{mpsc, oneshot};
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::setting_engine::SettingEngine;
use webrtc::api::APIBuilder;
use webrtc::data_channel::RTCDataChannel;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::interceptor::registry::Registry;

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
    #[allow(dead_code)]
    pc: Option<Arc<RTCPeerConnection>>,
    data_channels: Vec<Arc<RTCDataChannel>>,
    open_rx: Option<oneshot::Receiver<()>>,
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
            pc: None,
            data_channels: Vec::new(),
            open_rx: None,
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
                pc: None,
                data_channels: Vec::new(),
                open_rx: None,
            },
            Self {
                me: b,
                pc_present: true,
                open_channels: HashSet::from(["sutera-data".to_string()]),
                bus: bus_b,
                peer: Some(a.clone()),
                pc: None,
                data_channels: Vec::new(),
                open_rx: None,
            },
        )
    }

    pub fn has_peer_connection(&self) -> bool {
        self.pc_present
    }

    /// 仮実装: sutera-dataチャネルがopen済みかを返す（現状は即true）。
    pub fn has_data_channel_open(&self, label: &str) -> bool {
        if self.open_channels.contains(label) {
            return true;
        }
        self.data_channels.iter().any(|dc| dc.label() == label)
    }

    /// async版: 実PeerConnectionを生成し、sutera-data DataChannelのopenまでを確立する。
    pub async fn pair_with_datachannel_real(a: ParticipantId, b: ParticipantId) -> Result<(Self, Self)> {
        // MediaEngine/Codecs setup
        let mut m = MediaEngine::default();
        m.register_default_codecs()?;

        let mut registry = Registry::new();
        registry = register_default_interceptors(registry, &mut m)?;

        let setting_engine = SettingEngine::default();

        let api = APIBuilder::new()
            .with_media_engine(m)
            .with_interceptor_registry(registry)
            .with_setting_engine(setting_engine)
            .build();

        // In-processなのでICEサーバは不要。ホスト候補のみで十分。
        let config = RTCConfiguration {
            ice_servers: Vec::<RTCIceServer>::new(),
            ..Default::default()
        };

        let pc1 = Arc::new(api.new_peer_connection(config.clone()).await?);
        let pc2 = Arc::new(api.new_peer_connection(config).await?);

        // ICE candidate exchange via local channels (in-process signaling)
        let (to_pc2_tx, mut to_pc2_rx) = mpsc::unbounded_channel::<RTCIceCandidateInit>();
        let (to_pc1_tx, mut to_pc1_rx) = mpsc::unbounded_channel::<RTCIceCandidateInit>();

        pc1.on_ice_candidate(Box::new(move |cand| {
            let tx = to_pc2_tx.clone();
            Box::pin(async move {
                if let Some(c) = cand {
                    if let Ok(json) = c.to_json() {
                        let _ = tx.send(json);
                    }
                }
            })
        }));

        pc2.on_ice_candidate(Box::new(move |cand| {
            let tx = to_pc1_tx.clone();
            Box::pin(async move {
                if let Some(c) = cand {
                    if let Ok(json) = c.to_json() {
                        let _ = tx.send(json);
                    }
                }
            })
        }));

        // DataChannel from pc1, wait open on both ends
        let (open_tx1, open_rx1) = oneshot::channel();
        let (open_tx2, open_rx2) = oneshot::channel();

        let open_tx1_mutex = Arc::new(Mutex::new(Some(open_tx1)));
        let open_tx2_mutex = Arc::new(Mutex::new(Some(open_tx2)));

        let dc1 = pc1.create_data_channel("sutera-data", None).await?;
        let open_tx1_clone = open_tx1_mutex.clone();
        dc1.on_open(Box::new(move || {
            let open_tx1_clone = open_tx1_clone.clone();
            Box::pin(async move {
                if let Some(tx) = open_tx1_clone.lock().unwrap().take() {
                    let _ = tx.send(());
                }
            })
        }));

        pc2.on_data_channel(Box::new(move |dc: Arc<RTCDataChannel>| {
            let open_tx2_mutex = open_tx2_mutex.clone();
            Box::pin(async move {
                dc.on_open(Box::new(move || {
                    let open_tx2_mutex = open_tx2_mutex.clone();
                    Box::pin(async move {
                        if let Some(tx) = open_tx2_mutex.lock().unwrap().take() {
                            let _ = tx.send(());
                        }
                    })
                }));
            })
        }));

        // Offer/Answer exchange
        let offer = pc1.create_offer(None).await?;
        pc1.set_local_description(offer.clone()).await?;
        let mut gather_complete = pc1.gathering_complete_promise().await;
        // wait for ICE gathering to complete to include host candidates
        let _ = gather_complete.recv().await;

        pc2.set_remote_description(offer).await?;
        // start forwarding pc1 -> pc2 ICE after remote desc is set
        let pc2_for_task = pc2.clone();
        tokio::spawn(async move {
            while let Some(c) = to_pc2_rx.recv().await {
                let _ = pc2_for_task.add_ice_candidate(c).await;
            }
        });

        let answer = pc2.create_answer(None).await?;
        pc2.set_local_description(answer.clone()).await?;
        let mut gather_complete2 = pc2.gathering_complete_promise().await;
        let _ = gather_complete2.recv().await;

        pc1.set_remote_description(answer).await?;
        // start forwarding pc2 -> pc1 ICE after remote desc is set
        let pc1_for_task = pc1.clone();
        tokio::spawn(async move {
            while let Some(c) = to_pc1_rx.recv().await {
                let _ = pc1_for_task.add_ice_candidate(c).await;
            }
        });

        let (bus1, bus2) = mock_bus::MockBus::new_shared();

        Ok((
            Self {
                me: a.clone(),
                pc_present: true,
                open_channels: HashSet::new(),
                bus: bus1,
                peer: Some(b.clone()),
                pc: Some(pc1),
                data_channels: vec![dc1],
                open_rx: Some(open_rx1),
            },
            Self {
                me: b.clone(),
                pc_present: true,
                open_channels: HashSet::new(),
                bus: bus2,
                peer: Some(a),
                pc: Some(pc2),
                data_channels: Vec::new(), // dc arrives via on_data_channel
                open_rx: Some(open_rx2),
            },
        ))
    }

    pub async fn wait_data_channel_open(&mut self, timeout: std::time::Duration) -> Result<()> {
        if let Some(rx) = self.open_rx.take() {
            tokio::time::timeout(timeout, rx).await??;
        }
        Ok(())
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
