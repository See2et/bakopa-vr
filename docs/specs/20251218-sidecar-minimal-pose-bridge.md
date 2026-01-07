# Sidecar Minimal Pose Bridge 仕様書

## 概要
Bloom/Syncer と疎通するローカル Sidecar を用意し、外部クライアント（Unity なしでも可）が WebSocket/JSON 経由でルーム参加と頭・左右手の Transform 送受信を行える最小縦スライスを定義する。

## スコープ / 非スコープ
- スコープ
  - ローカル WebSocket エンドポイントを提供する Sidecar バイナリの追加。
  - Bloom WS への接続（CreateRoom/JoinRoom/Offer/Answer/IceCandidate のブリッジ）と Syncer への配線。
  - Pose（Head/HandL/HandR）メッセージの送受信と RateLimit/Error の伝達。
  - ユニットテスト／最小 E2E（2 クライアント相当）での送受信確認。
  - Client/Sidecar 間のシンプルな Token 認証（事前共有トークン、ローカル WS 接続の拒否/許可）。
- 非スコープ
  - 複数Clientの同時接続（Sidecarセッションの多重化）
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
### FR-000: Client/Sidecar Token 認証
- Sidecarは事前共有トークンを環境変数`SIDECAR_TOKEN`から取得する。未設定の場合、Sidecarは起動に失敗する。
- ClientがSidecarのWebSocket（ws://127.0.0.1:{port}/sidecar）に認証する際、Tokenによる認証を行う必要がある。
    - Client は WebSocket Upgrade リックエストに `Authorization: Bearer <token>` を付与する必要がある。
    - また、トークンの認証には定数時間比較を用いること。
- トークンが不一致/欠損の場合、HTTP 401/403を返す。
    - 401 Unauthorized: Authorizationヘッダが欠損、Bearerの形式が不正、トークンが不一致などの場合
    - 403 Forbidden: Originが非nullかつ拒否する場合（ブラウザからのアクセスは原則として拒否する）
- 認証失敗時、Sidecar は Bloom/Syncer へ一切接続せず、ローカル WS セッションのみを終了または継続待機する。

### FR-001: ローカル接続と Join
- Client は Sidecar の `ws://127.0.0.1:{port}/sidecar` に接続し、`Join` リクエストを送る。
    - もちろん、Tokenによる認証が済んでいることが前提
- `Join` は `room_id`（省略時は CreateRoom）、`bloom_ws_url`（必須）、`ice_servers` を含む。
    - `bloom_ws_url` が欠損している場合は `Error { kind="SignalingError", message=... }` を返す
    - `ice_servers` は空配列を許容する
- Sidecar は Bloom の WebSocket に接続し、room 参加に成功したら Syncer を初期化し、自身を登録する。
- 成功時に Client へ `SelfJoined { room_id, participant_id, participants }` を返す。
- また、ブラウザからのアクセスは原則として拒否する。

### FR-002: Pose 送信
- Client からの `SendPose { head, hand_l, hand_r }` を Syncer `Pose` メッセージ（Envelope v1, kind=pose）に変換し、DataChannel (unordered/unreliable) で送信する。
- レートリミットに抵触しない場合のみ送信し、抵触時は FR-004 に従う。

### FR-003: Pose 受信
- Syncer 経由で受信した `Pose` を `PoseReceived { from, pose }` として Client にプッシュする。
- 同期中の全参加者に対し、新規参加時は最新 Pose を受け取れるよう、Syncer の受信キューを即時ドレインする。

### FR-004: レートリミット/エラー伝達
- Syncer から `RateLimited { stream_kind }` を受け取った場合、Client に同名イベントを返す。
- Bloom/Syncer いずれかでの InvalidPayload/接続断など recoverable な失敗は `Error { kind, message }` として Client に通知し、Sidecar プロセスは継続する。
    - `Error.kind` は以下の粗い粒度に限定する
        - `InvalidPayload` / `NotJoined` / `SignalingError` / `TransportError`
    - Bloom WS の接続失敗・中継失敗は `SignalingError`
    - Syncer の `Error`/Transport 由来の失敗は `TransportError`

### FR-005: 切断・再接続
- Client が WebSocket 接続を正常終了した場合、Sidecar は Syncer を通じて Bloom へ Leave を送り、Syncer 内状態をクリアする。
- Client 再接続時は新規 session として扱い、既存 participant_id が残っていても重複しないようにする（Bloom 側で再発行された場合を優先）。
- Sidecarは同時に1つのClientに対してのみWebSocketセッションを保持する。
    - 既存セッションがある状態で新しい接続が来た場合、新しい方に接続する。
    - ただし、既存セッションの切断は、新規接続が認証に成功しWebSocket Upgradeが完了した直後にのみ行う。

### FR-006: シグナリング中継（Bloom互換）
- Client/Sidecar 間のシグナリングメッセージは Bloom WS と同一スキーマを用いる。
    - `Offer { to, sdp }` / `Answer { to, sdp }` / `IceCandidate { to, candidate }`
    - 受信側は `Offer { from, sdp }` / `Answer { from, sdp }` / `IceCandidate { from, candidate }`
- Sidecar は上記メッセージを Bloom WS へ中継し、Bloom からの受信を Client へ中継する（変換は行わない）。

## 非機能要件
### NFR-001: ログ/トレース
- `tracing` を用い、Sidecar から出す span/log には `room_id`/`participant_id`/`stream_kind` を可能な限り付与する。Subscriber 初期化はバイナリ内 1 か所のみ。
- ただし、SidecarのWebSocket接続認証時のTokenをログに出さないこと。

### NFR-002: 安全なデフォルト
- デフォルト送信レートは Syncer の 1 秒 20 メッセージに従い、Sidecar 内で追加スロットリングは行わない（必要なら将来拡張）。
- バイナリは localhost バインドをデフォルトとし、外部公開には明示的設定を要する。

## ディレクトリ構造
- `docs/specs/20251218-sidecar-minimal-pose-bridge.md` （本書）
- `sidecar/` （新規 crate。bin + lib 構成）
  - `sidecar/src/lib.rs`（WS API, Bloom/Syncer ブリッジ）
  - `sidecar/src/bin/sidecar.rs`
  - `sidecar/tests/`（最小 E2E: 2 クライアント相当の Pose ラウンドトリップ）

## 決定事項
- Client/Sidecar 間のシグナリングは Bloom WS と同一スキーマで中継する
- `Error.kind` は `InvalidPayload` / `NotJoined` / `SignalingError` / `TransportError` のみ
- 単一セッション切替は Upgrade 完了直後に既存セッションを切断する
- `Join` の `bloom_ws_url` は必須とする
- E2E はまず 1 Sidecar + 2 Client で実 WebRTC、次に 2 Sidecar を追加する

## 未決事項 / オープンクエッション
- 座標系: 左手系/右手系、単位（m）と基準姿勢の定義をどうするか？
    - A. 本仕様では、暫定的にUnityが採用する座標系に従う
    - 左手座標系（Left-Handed, +X右, +Y上, +Z前）
    - 位置は メートル（m） を単位とする
    - 回転表現はQuaternion(x,y,z,w)を用います
    - Head/HandのTransformはプレイヤーのルートに対する相対座標で表現します
- 送信レート: 1 秒あたりの Pose 更新頻度の推奨値・上限を Sidecar で設けるか?
    - Sidecar側では送信レートのリミットについて、判断を下さない。あくまでSyncer側の判断をClientに伝達するのみ。
    - Sidecarは最新のPoseのみを保持・送信する(coalescing)
- 認証: Bloom/Sidecar 間および Client/Sidecar 間でトークンを要求するか？導入タイミングは？
    - Client/Sidecar 間は事前共有 Token による簡易認証を実装する（本スライスで対応）。
    - Bloom/Sidecar 間はスコープ外（将来拡張）。
- エンドポイント設計: `ws://.../sidecar` のパス固定でよいか、ポート/パスの設定方法は？
    - `ws://127.0.0.1:{port}/sidecar`に固定。ポート設定のみ残します
- 現段階ではClientは未実装だが、テスト／デバッグの際にどうするのか？
    - `sidecar/tests`配下にFakeClient（テストヘルパ）を実装する
    - crate外への公開はせず、プロトコル契約の回帰防止とE2Eの再現性確保に目的を限定する

## テスト戦略
- Unit: JSONシリアライズ/デシリアライズ、Envelope v1 への変換、RateLimitイベント伝達、接続状態管理（再接続時の participant_id リセット）、エラー分類。
- Integration: Bloom WS とのシグナリング往復（Create/Join/Offer/Answer/Ice）、Syncer との Pose 送受信（実 WebRTC を含む）、RateLimiter 1 秒 20 件の境界。
- E2E（最小）: 1 Sidecar + 2 Client で Join → シグナリング → DataChannel 確立 → Pose 片方向配送（実 WebRTC）。
- E2E（拡張）: 2 Sidecar で同 room Join → シグナリング → Pose 片方向配送（実 WebRTC）。
- 依存の扱い: Clock をテストダブル化してレートリミットの境界を再現、Bloom/Syncer はローカルで立ち上げるかモックに差し替え。ネットワークポートは Ephemeral を使い衝突回避。

## テストケース一覧

### TC-000: 認証失敗で WS 接続が拒否される
#### TC-000-a
- 対応要件: FR-000
- 種別: Failure
- テスト層: Unit/Integration
- Given: Sidecarが期待するTokenを`CORRECT_TOKEN_ABC`に設定して起動している
- When: `Authorization: Bearer WRONG_TOKEN_XYZ`が渡され、token が不一致（必ず定数時間比較を行う）
- Then: 
    - Upgrade が拒否される（HTTP 401）

#### TC-000-b
- 対応要件: FR-000
- 種別: Failure
- テスト層: Unit/Integration
- Given: Sidecarが期待するTokenを`CORRECT_TOKEN_ABC`に設定して起動している
- When: Authorization ヘッダ無しで WebSocket Upgradeを試す
- Then: 
    - Upgrade が拒否される（HTTP 401）

#### TC-000-c
- 対応要件: FR-000
- 種別: Failure
- テスト層: Unit/Integration
- Given: Sidecarが期待するTokenを`CORRECT_TOKEN_ABC`に設定して起動している
- When: 不正な形式のヘッダが渡される
- Then: 
    - Upgrade が拒否される（HTTP 401）

#### TC-000-d
- 対応要件: FR-000
- 種別: Failure
- テスト層: Unit/Integration
- Given: Sidecarが期待するTokenを`CORRECT_TOKEN_ABC`に設定して起動している
- When: `Authorization: Bearer CORRECT_TOKEN_ABC`が渡される
- Then: 
    - Upgrade が成功し、Client/Sidecar間の WebSocket 接続が確立する

#### TC-000-e
- 対応要件: FR-000
- Given: 正しいtokenによってSidecarが起動
- When: WebSocket UpgradeリクエストのOriginが空、あるいはnullのとき
- Then: Upgrade が成功する

#### TC-000-f
- 対応要件: FR-000
- Given: 正しいtokenによってSidecar が起動
- When: WebSocket Upgrade リクエストに Origin: https://evil.example（任意の非null）を付与
- Then: Upgrade が拒否される（HTTP 403）

### TC-001: 新規ルーム Join 成功
- 対応要件: FR-001
- 種別: Happy
- テスト層: Integration
- Given Bloom WS が起動し、ルーム未作成、Sidecar がデフォルト設定で待機
- When Client が `/sidecar` に接続し `Join { room_id: null, bloom_ws_url, ice_servers }` を送る
- Then Client は `SelfJoined { room_id!=null, participant_id!=null, participants=[self] }` を受け取り、Bloom にルームが生成される

### TC-002: 既存ルーム Join で参加者リストを受信
- 対応要件: FR-001
- 種別: Happy
- テスト層: Integration
- Given Room が Bloom に存在し participant_x が登録済み
- When Client Y が room_id を指定して Join する
- Then Client Y は `SelfJoined` で participants に participant_x を含み、Bloom は参加者数を 2 に更新する

### TC-003: Pose 送信が DataChannel に載る
- 対応要件: FR-002
- 種別: Happy
- テスト層: Unit（Transport ダブル）
- Given Sidecar が Syncer に接続済みで、Transport ダブルが送信ペイロードを観測できる
- When Client から `SendPose { head, hand_l, hand_r }` を送信
- Then Transport へ kind=pose, Envelope v1, unordered/unreliable 指定で1件送信される

### TC-004: Pose 受信を Client へ中継
- 対応要件: FR-003
- 種別: Happy
- テスト層: Integration
- Given 2 クライアントが同 room に参加し、B 側の Syncer へ A からの Pose が到着する
- When Sidecar B が Transport 受信イベントを処理
- Then Client B は `PoseReceived { from=A, pose=... }` を受け取る（自分自身の Pose は配信しない）

### TC-005: レートリミット発火時の RateLimited イベント
- 対応要件: FR-004
- 種別: Failure/Boundary
- テスト層: Integration
- Given Clock ダブルで 1 秒間に 21 回の `SendPose` を発行
- When Syncer の RateLimiter が上限を超える
- Then Client は `RateLimited { stream_kind: pose }` を受け取り、超過分の Pose は送信されない

### TC-006: レートリミット回復後の配送再開
- 対応要件: FR-002, FR-004
- 種別: Boundary
- テスト層: Integration
- Given TC-005 直後で 1 秒経過するまで待機
- When その後に `SendPose` を 1 回送る
- Then Pose が再び送信され、RateLimited は発火しない

### TC-007: InvalidPayload を Error として転送
- 対応要件: FR-004
- 種別: Failure
- テスト層: Unit
- Given Client から body が欠損した `SendPose` または未知 kind を送信
- When Sidecar が検証し Syncer から InvalidPayload 相当のエラーを受け取る
- Then Client は `Error { kind=\"InvalidPayload\" }` を受け取り、Sidecar プロセスは継続する

### TC-008: WS 切断時の Leave/状態クリア
- 対応要件: FR-005
- 種別: Invariant
- テスト層: Integration
- Given Client が Join 済みで participants>0
- When Client が WS を正常 Close する
- Then Sidecar は Bloom に Leave を送り、Syncer の participant テーブルが空になり、後続の `PoseReceived` は発火しない

### TC-009: 再接続で participant_id が衝突しない
- 対応要件: FR-005
- 種別: Boundary
- テスト層: Integration
- Given Client が切断後すぐ再接続
- When 新規 `Join` を送信
- Then 新しい participant_id が払い出され、`SelfJoined` participants に重複がないことを確認する

### TC-010: トレースフィールド付与
- 対応要件: NFR-001
- 種別: Non-functional
- テスト層: Unit
- Given RecordingSubscriber をセットした Sidecar で Join→SendPose を 1 回実行
- When ログ/Span を収集
- Then いずれかの span に `room_id` `participant_id` `stream_kind` がフィールドとして含まれる

### TC-011: デフォルトバインドと設定
- 対応要件: NFR-002
- 種別: Non-functional / Boundary
- テスト層: Unit
- Given Sidecar を設定なしで起動
- Then リッスンアドレスが `127.0.0.1:{port}` になる  
- When 環境変数または設定でポートを指定  
- Then 指定ポートで起動し、パス `/sidecar` 以外への接続は 404 で拒否する

### TC-012: 新接続が未認証なら既存セッションを落とせない
- 対応要件: FR-000, FR-005
- 種別: Failure
- テスト層: Integration
- Given: 
    - Sidecar が期待する Token を CORRECT_TOKEN_ABC に設定して起動している
    - Client A が `Authorization: Bearer CORRECT_TOKEN_ABC` で /sidecar に接続済みで、WebSocket セッションが確立している
- When:
    - Client B が `Authorization: Bearer WRONG_TOKEN_XYZ` で /sidecar へ WebSocket Upgrade を試みる
- Then:
    - Client B の Upgrade は拒否される
    - Client A の WebSocket セッションは切断されない
    - Sidecar は Bloom/Syncer の状態を更新しない（Leaveや再接続が発生しない）

### TC-013: Join 前の SendPose は NotJoined として拒否される

- 対応要件: FR-001, FR-002, FR-004
- 種別: Failure / Invariant
- テスト層: Unit/Integration
- Given:
  - Sidecar が期待する Token を `CORRECT_TOKEN_ABC` に設定して起動している
  - Client が `Authorization: Bearer CORRECT_TOKEN_ABC` で `/sidecar` に接続済みで、WebSocket セッションが確立している
  - ただし、Client はまだ `Join` リクエストを送っておらず、Sidecar は Bloom/Syncer へ未接続である
- When:
  - Client が `SendPose { head, hand_l, hand_r }` を送信する
- Then:
  - Sidecar は Bloom/Syncer へ接続しない（CreateRoom/JoinRoom 等の呼び出しが発生しない）
  - Sidecar は当該 Pose を送信せず破棄する
  - Client は `Error { kind="NotJoined", message=... }` を受け取る
  - Sidecar プロセスは継続し、後続の `Join` を受け付けられる状態を保つ
  
### TC-014: Pose に NaN/Inf が含まれると InvalidPayload
- 対応要件: FR-004（および FR-002 の入力検証）
- 種別: Failure / Validation
- テスト層: Unit（Sidecar の入力検証）
- Given
    - Sidecar が起動しており、Client/Sidecar 間の WebSocket 接続が確立している（認証成功）
    - Client は `Join` 済みで `SelfJoined` を受信している（＝Sidecar は Pose を受理可能な状態）
- When
    - Client が `SendPose { head, hand_l, hand_r }` を送信するが、以下のいずれかを満たすペイロードを含む
    - `*.position` または `*.rotation` のいずれかの成分に **NaN/Inf/-Inf** が含まれる
    - 例: `head.position.x = NaN
- Then
    - Sidecar は当該 `SendPose` を **InvalidPayload** として扱い、Client に次を返す
    - `Error { kind="InvalidPayload", message=... }`

### TC-015: Offer/Answer/IceCandidate が Bloom へ中継される
- 対応要件: FR-006, FR-001
- 種別: Happy
- テスト層: Integration
- Given:
    - Sidecar が Bloom WS に接続できる状態で起動している
    - Client が `Join` 済みで、Bloom から `participants` が取得できている
- When:
    - Client が `Offer { to, sdp }` / `Answer { to, sdp }` / `IceCandidate { to, candidate }` を送信する
- Then:
    - Bloom WS は同一 payload を受け取る（type と fields が一致する）

### TC-016: Bloom からの Offer/Answer/IceCandidate を Client へ中継
- 対応要件: FR-006
- 種別: Happy
- テスト層: Integration
- Given:
    - Sidecar が Bloom WS に接続し、Client が `Join` 済み
- When:
    - Bloom WS から `Offer { from, sdp }` / `Answer { from, sdp }` / `IceCandidate { from, candidate }` が到着
- Then:
    - Client は同一 payload を受け取る（type と fields が一致する）

### TC-017: Join に bloom_ws_url が無い場合は SignalingError
- 対応要件: FR-001, FR-004
- 種別: Failure
- テスト層: Unit/Integration
- Given:
    - Client が認証済みで /sidecar に接続している
- When:
    - `Join { room_id, bloom_ws_url: null, ice_servers: [] }` を送信する
- Then:
    - Client は `Error { kind="SignalingError", message=... }` を受け取る
    - Sidecar は Bloom/Syncer に接続しない

### TC-018: E2E 最小（1 Sidecar + 2 Client, 実 WebRTC）
- 対応要件: FR-001, FR-002, FR-003, FR-006
- 種別: Happy / E2E
- テスト層: E2E
- Given:
    - Bloom WS が起動している
    - Sidecar が実 WebRTC で起動できる
- When:
    - Client A/B が同 room に Join し、Offer/Answer/IceCandidate を通じて接続する
    - Client A が `SendPose` を送る
- Then:
    - Client B は `PoseReceived` を受け取る

### TC-019: E2E 拡張（2 Sidecar, 実 WebRTC）
- 対応要件: FR-001, FR-002, FR-003, FR-006
- 種別: Happy / E2E
- テスト層: E2E
- Given:
    - Bloom WS が起動している
    - Sidecar A/B が実 WebRTC で起動できる
- When:
    - Client A が Sidecar A に、Client B が Sidecar B に接続し同 room に Join
    - Offer/Answer/IceCandidate を通じて接続する
    - Client A が `SendPose` を送る
- Then:
    - Client B は `PoseReceived` を受け取る

## カバレッジ確認チェックリスト
- [x] Join 成功/既存ルーム/参加者リスト
- [x] Pose 送信/受信（unordered/unreliable）と Envelope v1
- [x] レートリミット発火と回復（1 秒 20 件）
- [x] InvalidPayload/未知 kind のハンドリング
- [x] 切断・再接続・状態クリア
- [x] シグナリング中継（Offer/Answer/IceCandidate）と Bloom 互換性
- [x] `bloom_ws_url` 必須の検証
- [x] 単一セッション切替（Upgrade 完了直後の切断）
- [x] 実 WebRTC を使った最小 E2E
- [x] トレーシングフィールド付与
- [x] デフォルトバインド/設定パス
- [x] 座標系の定義に基づく値検証（未決: 左手/右手系と単位の厳密テスト）
- [x] 認証/トークン有無の分岐（未決: 認証方針）
