# ディレクトリ構造

- `bloom/api` : WebSocket シグナリング用メッセージ型（ClientToServer / ServerToClient、RelaySdp/Ice、ErrorCode）。
- `bloom/core` : ルーム/参加者管理（最大 8 名）、UUID ベースの RoomId/ParticipantId、Join/Leave ロジック。
- `bloom/ws` : `/ws` エンドポイントを持つ実サーバ。rate limit 1 秒 20
  メッセージ/セッション、tracing で room_id/participant_id を付与。バイナリ
  `main.rs` が subscriber を初期化。
- `syncer` : P2P 同期ライブラリ。SyncMessage Envelope v1
  （`kind=pose/chat/voice/control.*/signaling.*`）、BasicSyncer（1
  リクエスト→複数イベント）、RateLimiter（1 秒 20 件/セッション）、
  WebRTC Transport（DataChannel label `sutera-data`、Pose
  unordered/unreliable、音声トラック/Opus 対応）。
- `docs/specs` : 仕様群（20251125-bloom-signaling, 20251203-syncer-minimal-p2p ほか）。

## SuteraVRを構成する要素（2025-12-18 現在）

### Bloom（シグナリング）

- 役割: ルーム作成/参加/離脱と WebRTC Offer/Answer/ICE の中継。メディア・位置データは中継しない。
- 実装状況: `bloom/ws` で WebSocket サーバが動作し、JSON メッセージを
  PascalCase `type` で扱う。メモリ内ルーム管理、RateLimit 1s/20、span に
  room_id/participant_id を付与するテスト済み。認証なし・永続化なし。

### Syncer（P2P 同期）

- 役割: Bloom シグナリングを受け取り WebRTC を確立、DataChannel/音声で Pose/Chat/Voice を配送。
- 実装状況: Envelope v1 と StreamKind で型安全にメッセージを扱う。
  BasicSyncer が Router/TransportInbox/RateLimiter を束ね、再接続時の
  transport 差し替えや PeerLeft の重複抑制を実装。WebRTC Transport は
  feature `webrtc` で実 WebRTC、デフォルトで有効。Pose/Chat/Voice
  の多数の統合テストが存在し、Opus トラックやチャネルパラメータ、
  失敗時クリーンアップを検証済み。

### Client

- 役割: VR/デスクトップ操作。
- 状況: まだリポジトリ内に実装なし。

## 現状の進捗スナップショット（2025-12-18）

- Bloom:
  シグナリング専用 MVP 実装済（/ws、Create/Join/Leave/Offer/Answer/Ice、
  RateLimit 20/s、PascalCase JSON、メモリ管理）。
- Syncer: Pose/Chat/Voice/Control/Signaling をカバーするファサードと
  WebRTC Transport 実装済。DataChannel は `sutera-data`、Pose は
  unordered/unreliable、Opus 音声トラックあり。レート制御と
  トレーシングのテストが多数通過。
- Sidecar/Client: 未実装。ローカル WS API と Bloom/Syncer ブリッジを次スライスで構築する。

## 短期的な目標

- Bloom はシグナリングに専念する（現行どおり）。
- Syncer で頭・両手（3 点）＋チャット/音声の P2P 同期を成立させる（基本達成）。
- Sidecar を追加し、Unity なしでも 3 点 Pose の送受信ができる
  最小経路を確立する（現在の着手ポイント）。
- 固定ワールド/アバターの PCVR デモは Sidecar 経由の疎通後に着手。

## 実装の優先度

1. Bloom によるシグナリング機能（実装済）
2. Syncer による P2P 接続と同期機能（実装済、継続的に堅牢化）
3. Sidecar 経由で Client が 3D 空間同期を行えるように
   （着手中：20251218 Sidecar Minimal Pose Bridge 仕様に従い実装）
