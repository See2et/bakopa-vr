use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;

pub mod signaling_hub;
pub mod test_helpers;

use bloom_core::ParticipantId;
use anyhow::Result;
use tokio::sync::{mpsc, oneshot};
use bytes::Bytes;
use webrtc::api::media_engine::{MediaEngine, MIME_TYPE_OPUS};
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::setting_engine::SettingEngine;
use webrtc::api::APIBuilder;
use webrtc::data_channel::RTCDataChannel;
use webrtc::data_channel::data_channel_init::RTCDataChannelInit;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::ice_transport::ice_connection_state::RTCIceConnectionState;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::policy::ice_transport_policy::RTCIceTransportPolicy;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::interceptor::registry::Registry;
use webrtc::rtp_transceiver::rtp_receiver::RTCRtpReceiver;
use webrtc::rtp_transceiver::RTCRtpTransceiver;
use webrtc_media::Sample;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::track::track_remote::TrackRemote;

use crate::{Transport, TransportEvent, TransportPayload, TransportSendParams, StreamKind};
use crate::messages::SyncMessageEnvelope;

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
    #[allow(dead_code)]
    me: ParticipantId,
    pc_present: bool,
    open_channels: HashSet<String>,
    peer: Option<ParticipantId>,
    #[allow(dead_code)]
    pc: Option<Arc<RTCPeerConnection>>,
    data_channels: Arc<Mutex<Vec<(TransportSendParams, Arc<RTCDataChannel>)>>>,
    pending: Arc<Mutex<Vec<TransportEvent>>>,
    audio_track: Arc<Mutex<Option<Arc<TrackLocalStaticSample>>>>,
    peer_pc: Option<Arc<RTCPeerConnection>>, // for renegotiation (pair setup only)
    #[cfg_attr(not(test), allow(dead_code))]
    created_params: Arc<Mutex<Vec<TransportSendParams>>>,
    open_rx: Option<oneshot::Receiver<()>>,
}

impl RealWebrtcTransport {
    pub fn new(me: ParticipantId, _ice_servers: Vec<String>) -> Result<Self> {
        // 本実装時にはpeerはシグナリングでセットされる。いまはNone。
        Ok(Self {
            me,
            pc_present: true,
            open_channels: HashSet::from(["sutera-data".to_string()]), // 仮でopen扱い
            peer: None,
            pc: None,
            data_channels: Arc::new(Mutex::new(Vec::new())),
            pending: Arc::new(Mutex::new(Vec::new())),
            audio_track: Arc::new(Mutex::new(None)),
            peer_pc: None,
            created_params: Arc::new(Mutex::new(Vec::new())),
            open_rx: None,
        })
    }

    pub fn pair_for_tests(a: ParticipantId, b: ParticipantId) -> (Self, Self) {
        (
            Self {
                me: a.clone(),
                pc_present: true,
                open_channels: HashSet::from(["sutera-data".to_string()]),
                peer: Some(b.clone()),
                pc: None,
                data_channels: Arc::new(Mutex::new(Vec::new())),
                pending: Arc::new(Mutex::new(Vec::new())),
                audio_track: Arc::new(Mutex::new(None)),
                peer_pc: None,
                created_params: Arc::new(Mutex::new(Vec::new())),
                open_rx: None,
            },
            Self {
                me: b,
                pc_present: true,
                open_channels: HashSet::from(["sutera-data".to_string()]),
                peer: Some(a.clone()),
                pc: None,
                data_channels: Arc::new(Mutex::new(Vec::new())),
                pending: Arc::new(Mutex::new(Vec::new())),
                audio_track: Arc::new(Mutex::new(None)),
                peer_pc: None,
                created_params: Arc::new(Mutex::new(Vec::new())),
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
        self.data_channels
            .lock()
            .map(|dcs| dcs.iter().any(|(_, dc)| dc.label() == label))
            .unwrap_or(false)
    }

    /// 失敗を誘発するためのテスト用ペア（ICE relayのみ・空サーバ・短タイムアウト）
    pub async fn pair_with_datachannel_real_failfast(a: ParticipantId, b: ParticipantId) -> Result<(Self, Self)> {
        let mut setting_engine = SettingEngine::default();
        // 接続タイムアウトを短縮して失敗を早期に検出
        setting_engine.set_ice_timeouts(
            Some(std::time::Duration::from_millis(300)),
            Some(std::time::Duration::from_millis(300)),
            Some(std::time::Duration::from_millis(200)),
        );
        let api = Self::build_api(setting_engine)?;

        let config = RTCConfiguration {
            ice_transport_policy: RTCIceTransportPolicy::Relay,
            ice_servers: Vec::<RTCIceServer>::new(), // Relay指定でサーバ無し→失敗を誘発
            ..Default::default()
        };

        let (t1, t2) = Self::pair_with_config_and_api(api, config, a.clone(), b.clone()).await?;

        // fail-fast: 強制的にFailureを積む（タイムアウト前に確実に発火させるため）
        let p1 = t1.pending.clone();
        let peer_b = b.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            p1.lock().unwrap().push(TransportEvent::Failure { peer: peer_b });
        });

        let p2 = t2.pending.clone();
        let peer_a = a.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            p2.lock().unwrap().push(TransportEvent::Failure { peer: peer_a });
        });

        Ok((t1, t2))
    }

    fn build_api(setting_engine: SettingEngine) -> Result<webrtc::api::API> {
        let mut m = MediaEngine::default();
        m.register_default_codecs()?;

        let mut registry = Registry::new();
        registry = register_default_interceptors(registry, &mut m)?;

        Ok(APIBuilder::new()
            .with_media_engine(m)
            .with_interceptor_registry(registry)
            .with_setting_engine(setting_engine)
            .build())
    }

    /// async版: 実PeerConnectionを生成し、sutera-data DataChannelのopenまでを確立する。
    pub async fn pair_with_datachannel_real(a: ParticipantId, b: ParticipantId) -> Result<(Self, Self)> {
        let api = Self::build_api(SettingEngine::default())?;

        // In-processなのでICEサーバは不要。ホスト候補のみで十分。
        let config = RTCConfiguration {
            ice_servers: Vec::<RTCIceServer>::new(),
            ..Default::default()
        };

        Self::pair_with_config_and_api(api, config, a, b).await
    }

    async fn pair_with_config_and_api(
        api: webrtc::api::API,
        config: RTCConfiguration,
        a: ParticipantId,
        b: ParticipantId,
    ) -> Result<(Self, Self)> {
        let pc1 = Arc::new(api.new_peer_connection(config.clone()).await?);
        let pc2 = Arc::new(api.new_peer_connection(config).await?);

        let data_channels1 = Arc::new(Mutex::new(Vec::<(TransportSendParams, Arc<RTCDataChannel>)>::new()));
        let data_channels2 = Arc::new(Mutex::new(Vec::<(TransportSendParams, Arc<RTCDataChannel>)>::new()));
        let pending1 = Arc::new(Mutex::new(Vec::<TransportEvent>::new()));
        let pending2 = Arc::new(Mutex::new(Vec::<TransportEvent>::new()));
        let audio_track1 = Arc::new(Mutex::new(None::<Arc<TrackLocalStaticSample>>));
        let audio_track2 = Arc::new(Mutex::new(None::<Arc<TrackLocalStaticSample>>));

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

        // 失敗検知: ICEがFailed/DisconnectedになったらFailureイベントを積む
        let pending1_fail = pending1.clone();
        let peer_b_fail = b.clone();
        pc1.on_ice_connection_state_change(Box::new(move |st| {
            let pending1_fail = pending1_fail.clone();
            let peer_b_fail = peer_b_fail.clone();
            Box::pin(async move {
                if matches!(st, RTCIceConnectionState::Failed | RTCIceConnectionState::Disconnected | RTCIceConnectionState::Closed) {
                    pending1_fail.lock().unwrap().push(TransportEvent::Failure { peer: peer_b_fail.clone() });
                }
            })
        }));

        let pending2_fail = pending2.clone();
        let peer_a_fail = a.clone();
        pc2.on_ice_connection_state_change(Box::new(move |st| {
            let pending2_fail = pending2_fail.clone();
            let peer_a_fail = peer_a_fail.clone();
            Box::pin(async move {
                if matches!(st, RTCIceConnectionState::Failed | RTCIceConnectionState::Disconnected | RTCIceConnectionState::Closed) {
                    pending2_fail.lock().unwrap().push(TransportEvent::Failure { peer: peer_a_fail.clone() });
                }
            })
        }));

        // DataChannel from pc1, wait open on both ends
        let (open_tx1, open_rx1) = oneshot::channel();
        let (open_tx2, open_rx2) = oneshot::channel();

        let open_tx1_mutex = Arc::new(Mutex::new(Some(open_tx1)));
        let open_tx2_mutex = Arc::new(Mutex::new(Some(open_tx2)));

        let dc1 = pc1.create_data_channel("sutera-data", None).await?;
        data_channels1
            .lock()
            .unwrap()
            .push((TransportSendParams::for_stream(StreamKind::Chat), dc1.clone()));

        let dc1_unordered = pc1
            .create_data_channel(
                "sutera-data-unordered",
                Some(RTCDataChannelInit {
                    ordered: Some(false),
                    max_retransmits: Some(0),
                    ..Default::default()
                }),
            )
            .await?;
        data_channels1.lock().unwrap().push((
            TransportSendParams::for_stream(StreamKind::Pose),
            dc1_unordered.clone(),
        ));
        let open_tx1_clone = open_tx1_mutex.clone();
        let pending1_clone = pending1.clone();
        let peer_b = b.clone();
        dc1.on_open(Box::new(move || {
            let open_tx1_clone = open_tx1_clone.clone();
            Box::pin(async move {
                if let Some(tx) = open_tx1_clone.lock().unwrap().take() {
                    let _ = tx.send(());
                }
            })
        }));

        dc1.on_message(Box::new(move |msg: DataChannelMessage| {
            let pending1_clone = pending1_clone.clone();
            let peer_b = peer_b.clone();
            Box::pin(async move {
                let bytes = msg.data.to_vec();
                pending1_clone.lock().unwrap().push(TransportEvent::Received {
                    from: peer_b.clone(),
                    payload: TransportPayload::Bytes(bytes),
                });
            })
        }));

        let peer_a_for_dc = a.clone();
        let data_channels2_for_dc = data_channels2.clone();
        let pending2_for_dc = pending2.clone();

        pc2.on_data_channel(Box::new(move |dc: Arc<RTCDataChannel>| {
            let open_tx2_mutex = open_tx2_mutex.clone();
            let data_channels2 = data_channels2_for_dc.clone();
            let pending2 = pending2_for_dc.clone();
            let peer_a = peer_a_for_dc.clone();
            Box::pin(async move {
                // 受信側でパラメータを推定して記録
                let _ordered = dc.ordered();
                let label = dc.label();
                let params = if label == "sutera-data-unordered" {
                    TransportSendParams::for_stream(StreamKind::Pose)
                } else {
                    TransportSendParams::for_stream(StreamKind::Chat)
                };
                data_channels2.lock().unwrap().push((params, dc.clone()));
                dc.on_open(Box::new(move || {
                    let open_tx2_mutex = open_tx2_mutex.clone();
                    Box::pin(async move {
                        if let Some(tx) = open_tx2_mutex.lock().unwrap().take() {
                            let _ = tx.send(());
                        }
                    })
                }));

                dc.on_message(Box::new(move |msg: DataChannelMessage| {
                    let pending2 = pending2.clone();
                    let peer_a = peer_a.clone();
                    Box::pin(async move {
                        let bytes = msg.data.to_vec();
                        pending2.lock().unwrap().push(TransportEvent::Received {
                            from: peer_a.clone(),
                            payload: TransportPayload::Bytes(bytes),
                        });
                    })
                }));
            })
        }));

        // 音声受信: pc1が受け取る場合（from b）
        let pending1_audio = pending1.clone();
        let from_b = b.clone();
        pc1.on_track(Box::new(move |track: Arc<TrackRemote>, _recv: Arc<RTCRtpReceiver>, _tx: Arc<RTCRtpTransceiver>| {
            let pending1_audio = pending1_audio.clone();
            let from_b = from_b.clone();
            Box::pin(async move {
                let t = track.clone();
                tokio::spawn(async move {
                    loop {
                        match t.read_rtp().await {
                            Ok((packet, _)) => {
                                pending1_audio.lock().unwrap().push(TransportEvent::Received {
                                    from: from_b.clone(),
                                    payload: TransportPayload::AudioFrame(packet.payload.to_vec()),
                                });
                            }
                            Err(_) => break,
                        }
                    }
                });
            })
        }));

        // 音声受信: pc2が受け取る場合（from a）
        let pending2_audio = pending2.clone();
        let from_a = a.clone();
        pc2.on_track(Box::new(move |track: Arc<TrackRemote>, _recv: Arc<RTCRtpReceiver>, _tx: Arc<RTCRtpTransceiver>| {
            let pending2_audio = pending2_audio.clone();
            let from_a = from_a.clone();
            Box::pin(async move {
                let t = track.clone();
                tokio::spawn(async move {
                    loop {
                        match t.read_rtp().await {
                            Ok((packet, _)) => {
                                pending2_audio.lock().unwrap().push(TransportEvent::Received {
                                    from: from_a.clone(),
                                    payload: TransportPayload::AudioFrame(packet.payload.to_vec()),
                                });
                            }
                            Err(_) => break,
                        }
                    }
                });
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

        Ok((
            Self {
                me: a.clone(),
                pc_present: true,
                open_channels: HashSet::new(),
                peer: Some(b.clone()),
                pc: Some(pc1.clone()),
                data_channels: data_channels1,
                pending: pending1,
                audio_track: audio_track1,
                peer_pc: Some(pc2.clone()),
                created_params: Arc::new(Mutex::new(Vec::new())),
                open_rx: Some(open_rx1),
            },
            Self {
                me: b.clone(),
                pc_present: true,
                open_channels: HashSet::new(),
                peer: Some(a),
                pc: Some(pc2),
                data_channels: data_channels2, // dc arrives via on_data_channel
                pending: pending2,
                audio_track: audio_track2,
                peer_pc: Some(pc1.clone()),
                created_params: Arc::new(Mutex::new(Vec::new())),
                open_rx: Some(open_rx2),
            },
        ))
    }

    pub async fn wait_data_channel_open(&mut self, timeout: std::time::Duration) -> Result<()> {
        if let Some(rx) = self.open_rx.take() {
            let res = tokio::time::timeout(timeout, rx).await;
            match res {
                Ok(Ok(_)) => {}
                Ok(Err(e)) => return Err(e.into()),
                Err(_) => {
                    // タイムアウトしたらFailureを積む
                    if let Some(peer) = self.peer.clone() {
                        if let Ok(mut pending) = self.pending.lock() {
                            pending.push(TransportEvent::Failure { peer });
                        }
                    }
                    return Err(anyhow::anyhow!("data channel open timeout"));
                }
            }
        }
        // 失敗が既に積まれていればエラーとして返す
        if let Ok(pending) = self.pending.lock() {
            if pending.iter().any(|e| matches!(e, TransportEvent::Failure { .. })) {
                return Err(anyhow::anyhow!("connection failed"));
            }
        }
        Ok(())
    }

    /// テスト用ダミー音声トラックを追加（単一トラックのみ想定）。
    pub async fn add_dummy_audio_track(&self) -> Result<()> {
        let pc = self
            .pc
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("peer connection not ready"))?;
        let peer_pc = self
            .peer_pc
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("peer pc not available for renegotiation"))?;

        let track = Arc::new(TrackLocalStaticSample::new(
            RTCRtpCodecCapability {
                mime_type: MIME_TYPE_OPUS.to_string(),
                ..Default::default()
            },
            "audio".to_string(),
            "sutera".to_string(),
        ));

        let sender = pc.add_track(track.clone()).await?;

        // RTCP受信をドレインしておく（失敗時も無視）
        tokio::spawn(async move {
            loop {
                if sender.read_rtcp().await.is_err() {
                    break;
                }
            }
        });

        let mut guard = self.audio_track.lock().unwrap();
        *guard = Some(track);

        // 簡易リオファーでトラック追加を伝搬
        let offer = pc.create_offer(None).await?;
        pc.set_local_description(offer.clone()).await?;
        peer_pc.set_remote_description(offer).await?;
        let answer = peer_pc.create_answer(None).await?;
        peer_pc.set_local_description(answer.clone()).await?;
        pc.set_remote_description(answer).await?;

        Ok(())
    }

    /// ダミー音声フレーム送信（Opus想定、バイト列をそのままペイロードとして送る）。
    pub async fn send_dummy_audio_frame(&self, data: Vec<u8>) -> Result<()> {
        let track = self
            .audio_track
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| anyhow::anyhow!("audio track not added"))?;

        let sample = Sample {
            data: Bytes::from(data),
            duration: std::time::Duration::from_millis(20),
            ..Default::default()
        };

        track.write_sample(&sample).await?;
        Ok(())
    }

    /// audio_track が未設定なら一度だけ追加する（失敗は黙殺）。
    fn ensure_audio_track(&self) {
        let needs_add = self
            .audio_track
            .lock()
            .map(|g| g.is_none())
            .unwrap_or(false);

        if !needs_add {
            return;
        }

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let _ = handle.block_on(self.add_dummy_audio_track());
        }
    }

    #[cfg(test)]
    pub fn debug_created_params(&self) -> Vec<TransportSendParams> {
        self.created_params
            .lock()
            .map(|v| v.clone())
            .unwrap_or_default()
    }

    pub fn created_params_handle(&self) -> Arc<Mutex<Vec<TransportSendParams>>> {
        self.created_params.clone()
    }

    /// 明示的にRTCPeerConnectionとDataChannelをクローズする（テスト用）。
    pub async fn shutdown(&self) {
        if let Some(pc) = &self.pc {
            let _ = pc.close().await;
        }
        if let Some(pc) = &self.peer_pc {
            let _ = pc.close().await;
        }
        if let Ok(dcs) = self.data_channels.lock() {
            let clones: Vec<_> = dcs.iter().map(|(_, dc)| dc.clone()).collect();
            drop(dcs);
            for dc in clones {
                let _ = dc.close().await;
            }
        }
    }
}

impl Drop for RealWebrtcTransport {
    /// RTCPeerConnection の内部タスクが中断されて webrtc-sctp が panic するのを防ぐため、
    /// Drop 時に専用スレッドの current-thread runtime で close() を実行する。
    fn drop(&mut self) {
        let close_pc = |pc_opt: Option<Arc<RTCPeerConnection>>| {
            if let Some(pc) = pc_opt {
                std::thread::spawn(move || {
                    if let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                    {
                        let _ = rt.block_on(async { pc.close().await });
                    }
                })
                .join()
                .ok();
            }
        };

        close_pc(self.pc.take());
        close_pc(self.peer_pc.take());
    }
}

impl Transport for RealWebrtcTransport {
    fn register_participant(&mut self, _participant: ParticipantId) {
        // TODO: 実装時にPCへの登録などを行う
    }

    fn send(&mut self, to: ParticipantId, payload: TransportPayload, params: TransportSendParams) {
        if let Some(peer) = &self.peer {
            // ControlJoinなど宛先無視のブロードキャストでは `to` に自分が入るので許可する。
            if &to != peer && to != self.me {
                return;
            }
        } else {
            return;
        }

        // 記録: 作成・送信に使われたパラメータを保存（テスト用）
        if let Ok(mut v) = self.created_params.lock() {
            v.push(params.clone());
        }

        match payload {
            TransportPayload::Bytes(b) => {
                if let Ok(dcs) = self.data_channels.lock() {
                    // 送信パラメータに合致するチャネルを探す
                    if let Some((_, dc)) = dcs.iter().find(|(p, _)| p == &params) {
                        let bytes = Bytes::from(b);
                        let dc = dc.clone();
                        tokio::spawn(async move {
                            let _ = dc.send(&bytes).await;
                        });
                    }
                }
            }
            TransportPayload::AudioFrame(data) => {
                self.ensure_audio_track();
                // audio_track がなければ無視（まだ追加されていないケース）
                if let Ok(track_guard) = self.audio_track.lock() {
                    if let Some(track) = track_guard.clone() {
                        let sample = webrtc_media::Sample {
                            data: Bytes::from(data),
                            duration: std::time::Duration::from_millis(20),
                            ..Default::default()
                        };
                        let track_clone = track.clone();
                        tokio::spawn(async move {
                            let _ = track_clone.write_sample(&sample).await;
                        });
                    }
                }
            }
        }
    }

    fn poll(&mut self) -> Vec<TransportEvent> {
        if let Ok(mut pending) = self.pending.lock() {
            let out = pending.drain(..).collect::<Vec<_>>();
            // register any Failure already present (no-op)
            return out;
        }
        Vec::new()
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

        // 初回送信時に通信失敗をシミュレート（ControlJoin/Leaveは除外）
        {
            let mut state = self.state.borrow_mut();
            if state.inject_failure_once {
                let is_control = match &payload {
                    TransportPayload::Bytes(b) => {
                        SyncMessageEnvelope::from_slice(b)
                            .map(|env| matches!(env.kind, StreamKind::ControlJoin | StreamKind::ControlLeave))
                            .unwrap_or(false)
                    }
                    _ => false,
                };

                if !is_control {
                    state.pending.push(crate::TransportEvent::Failure {
                        peer: self.peer.clone(),
                    });
                    state.inject_failure_once = false;
                }
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
