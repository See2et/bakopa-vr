use bloom_core::ParticipantId;

/// Smoke: RealWebrtcTransport が生成され、内部に RTCPeerConnection を保持することを期待。
/// 現状未実装のためコンパイルエラーでREDになる想定。
#[test]
fn real_webrtc_transport_initializes_peer_connection() {
    let me = ParticipantId::new();

    // 未実装: 実装時には STUN サーバ一覧などの設定を渡す。
    let transport = syncer::webrtc_transport::RealWebrtcTransport::new(me.clone(), vec![])
        .expect("should create real webrtc transport");

    // RTCPeerConnectionを持っていることを確認（インターフェースは実装時に定義）。
    assert!(transport.has_peer_connection());
}
