# Sidecar Minimal Pose Bridge 仕様書

## 概要
Bloom/Syncer と疎通するローカル Sidecar を用意し、外部クライアント（Unity なしでも可）が WebSocket/JSON 経由でルーム参加と頭・左右手の Transform 送受信を行える最小縦スライスを定義する。

## スコープ / 非スコープ
- スコープ
  - ローカル WebSocket エンドポイントを提供する Sidecar バイナリの追加。
  - Bloom WS への接続（CreateRoom/JoinRoom/Offer/Answer/IceCandidate のブリッジ）と Syncer への配線。
  - Pose（Head/HandL/HandR）メッセージの送受信と RateLimit/Error の伝達。
  - ユニットテスト／最小 E2E（2 クライアント相当）での送受信確認。
- 非スコープ
  - VR/Unity UI、音声・テキストチャット、認証・課金、TURN/帯域適応。
  - UGC 配信・アバター管理、モデレーション、永続化。

## 用語
- Sidecar: クライアント（Unity など）と Bloom/Syncer を橋渡しするローカルプロセス。
- Client: Sidecar にローカル WebSocket で接続する外部アプリ（本スライスでは CLI/テストダブル）。
- Bloom WS: `bloom/ws` が提供するシグナリング WebSocket (`/ws`)。
- Syncer: `syncer` クレートの BasicSyncer + WebRTC Transport。DataChannel label は `sutera-data`。
- PoseTransform: 頭・左右手の位置/姿勢をまとめた構造体（Syncer `PoseMessage` に準拠）。
- StreamKind: `pose` / `chat` / `voice` / `control.*` / `signaling.*` などのメッセージ種別。

## 前提
- Bloom WS は 20251125-bloom-signaling 仕様で動作し、`CreateRoom`/`JoinRoom`/`Offer`/`Answer`/`IceCandidate` を JSON でやりとりする。
- Syncer は 20251203-syncer-minimal-p2p 仕様を実装済み（DataChannel/音声、1 秒 20 メッセージのレートリミット、Envelope v1）。
- Sidecar と Client は同一ホスト上で TCP（WS）接続できることを前提とする。
- WebRTC STUN 設定は環境変数または設定ファイルで Sidecar → Syncer に渡す。TURN は未対応。

## 機能要件
### FR-001: ローカル接続と Join
- Client は Sidecar の `ws://127.0.0.1:{port}/sidecar` に接続し、`Join` リクエストを送る。
- `Join` は `room_id`（省略時は CreateRoom）、`bloom_ws_url`、`ice_servers` を含む。
- Sidecar は Bloom WS に接続し、room 参加に成功したら Syncer を初期化し、自身を登録する。
- 成功時に Client へ `SelfJoined { room_id, participant_id, participants }` を返す。

### FR-002: Pose 送信
- Client からの `SendPose { head, hand_l, hand_r }` を Syncer `Pose` メッセージ（Envelope v1, kind=pose）に変換し、DataChannel (unordered/unreliable) で送信する。
- レートリミットに抵触しない場合のみ送信し、抵触時は FR-004 に従う。

### FR-003: Pose 受信
- Syncer 経由で受信した `Pose` を `PoseReceived { from, pose }` として Client にプッシュする。
- 同期中の全参加者に対し、新規参加時は最新 Pose を受け取れるよう、Syncer の受信キューを即時ドレインする。

### FR-004: レートリミット/エラー伝達
- Syncer から `RateLimited { stream_kind }` を受け取った場合、Client に同名イベントを返す。
- Bloom/Syncer いずれかでの InvalidPayload/接続断など recoverable な失敗は `Error { kind, message }` として Client に通知し、Sidecar プロセスは継続する。

### FR-005: 切断・再接続
- Client が WS を正常終了した場合、Sidecar は Bloom へ Leave を送り、Syncer 内状態をクリアする。
- Client 再接続時は新規 session として扱い、既存 participant_id が残っていても重複しないようにする（Bloom 側で再発行された場合を優先）。

## 非機能要件
### NFR-001: ログ/トレース
- `tracing` を用い、Sidecar から出す span/log には `room_id`/`participant_id`/`stream_kind` を可能な限り付与する。Subscriber 初期化はバイナリ内 1 か所のみ。

### NFR-002: 安全なデフォルト
- デフォルト送信レートは Syncer の 1 秒 20 メッセージに従い、Sidecar 内で追加スロットリングは行わない（必要なら将来拡張）。
- バイナリは localhost バインドをデフォルトとし、外部公開には明示的設定を要する。

## ディレクトリ構造
- `docs/specs/20251218-sidecar-minimal-pose-bridge.md` （本書）
- `sidecar/` （新規 crate。bin + lib 構成）
  - `sidecar/src/lib.rs`（WS API, Bloom/Syncer ブリッジ）
  - `sidecar/src/bin/sidecar.rs`
  - `sidecar/tests/`（最小 E2E: 2 クライアント相当の Pose ラウンドトリップ）

## 未決事項 / オープンクエッション
- 座標系: 左手系/右手系、単位（m）と基準姿勢の定義をどうするか？
    - A. 本仕様では、暫定的にUnityが採用する座標系に従う
    - 左手座標系（Left-Handed, +X右, +Y上, +Z前）
    - 位置は メートル（m） を単位とする
    - 回転表現はQuaternion(x,y,z,w)を用います
    - Head/HandのTransformはプレイヤーのルート座標に対する相対的な値で表現します
- 送信レート: 1 秒あたりの Pose 更新頻度の推奨値・上限を Sidecar で設けるか?
    - Sidecar側では送信レートのリミットについて、判断を下さない。あくまでSyncer側の判断をClientに伝達するのみ。
    - Sidecarは最新のPoseのみを保持・送信する(coalescing)
- 認証: Bloom/Sidecar 間および Client/Sidecar 間でトークンを要求するか？導入タイミングは？
    - とりあえず今は実装しません
- エンドポイント設計: `ws://.../sidecar` のパス固定でよいか、ポート/パスの設定方法は？
    - `ws://127.0.0.1:{port}/sidecar`に固定。ポート設定のみ残します
- エラー列挙: Client への `Error.kind` をどの粒度で公開するか？（Bloom/Syncer の内部理由をどこまで露出させるか）
    - `Error.kind`は粗い粒度に保ち、`message`に人間向けの文章を差し込みましょう

## サンプルシナリオ (TDD入口)
- ハッピー:  
  Given Bloom WS が起動し、Room 未作成  
  When Client A が Sidecar に接続し `Join { bloom_ws_url, room_id: null }` を送信し、その後 `SendPose` を 1 度送る  
  Then Client A は `SelfJoined` を受け取り、`PoseReceived`（自分自身は含まない）が発火しないまま、Bloom/Syncer にエラーが記録されない  
  And Client B が別 Sidecar から同 room に Join して `PoseReceived`（from=A）が 1 件届く

- 代表的失敗:  
  Given Client が 1 秒間に 25 回 `SendPose` を送る  
  When Syncer が RateLimit し始める  
  Then Client は `RateLimited { stream_kind: pose }` を受け取り、Sidecar は該当期間の Pose を送信しない  
  And 1 秒経過後に再度送信すると Pose が配送される
