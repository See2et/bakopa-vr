# Syncer-Minimal-P2P 仕様書

作成日: 2025-12-03

## 概要
SyncerはBloomが提供するシグナリング結果を用い、クライアント間のリアルタイム同期（姿勢・テキスト・音声）をP2Pで成立させる。トランスポートはWebRTCを採用し、MVPでは「Syncer単体の最小E2E」を1本のPRで実装する。Sidecar/Clientは後続実装としつつ、契約インターフェースは本仕様で固定する。

## スコープ / 非スコープ
- スコープ
  - WebRTCによるP2P確立（STUNのみ。TURNは未対応）。
  - 姿勢(Head/HandL/HandR)、テキストチャット、音声(Opus 48kHz/20ms)の配送。
  - 参加/離脱の通知、再接続時のセッション入れ替え、レート制御（1秒20メッセージ）。
  - tracingフィールド: `room_id`, `participant_id`, `stream_kind` の付与。
  - Sidecar/ClientとのIPC契約（ドキュメントとスタブまで）。
- 非スコープ
  - TURN/ICEリレー、帯域適応、NAT失敗時のフォールバック。
  - Sidecar/Client本体の実装、Unity統合。
  - 永続化、録音、モデレーション、暗号鍵管理。

## 用語
- room_id / participant_id: Bloomが払い出す接続・ルーム識別子。Syncerはそのままピア識別に用いる。
- peer_id: Syncer内部でのピアキー。participant_idと同一。
- Sidecar: Unity等クライアントとのローカルIPCブリッジ。今回未実装。
- stream_kind: pose / chat / voice / signaling を示すログ用フィールド。

## 前提
- シグナリングは Bloom WebSocket 仕様（20251125-bloom-signaling.md）に準拠。
- トランスポートはWebRTC（DataChannel + Audio Track）。STUNサーバ一覧は設定経由で渡される。
- 音声はOpus mono 48kHz、ptime 20ms、VAD/DTXオフ（シンプル優先）。
- DataChannelは原則としてordered/reliable（Poseについてはunordered/unreliable）、label="sutera-data"、JSONシリアライズ（`v`フィールド必須）。
- レート制御は1接続あたり直近1秒20件超過でドロップ＋`RateLimited`返却。

## 機能要件
1) 接続・シグナリング
- SyncerはBloomへOffer/Answer/ICEを送信し、Bloomからのシグナリングを処理してPeerConnectionを確立する。
- participant_idが重複接続で再到来した場合は旧セッションを破棄し、新セッションを有効化する。

2) データ配送（DataChannel）
- メッセージ種: Pose, Chat, Control(join/leave通知)。
- Poseは最新優先で順序保証不要、Chat/Controlは順序保証。

3) 音声配送
- 音声トラックでOpusフレームを送受信。片方向でも到達すればMVP成立。

4) 参加/離脱通知
- peer切断検知時に`PeerLeft`を一度だけ発火し、状態をクリーンアップ。
- Bloomからのjoin/leave通知を受けて内部ピア表を更新。

5) レート制御
- Syncer内のIPCセッション単位で1秒20メッセージ上限。超過期間は`RateLimited`のみ返す。

## 非機能要件
- ロギング: すべての主要spanに`room_id`,`participant_id`（可能なら`remote_participant_id`）と`stream_kind`を付与。機密データはログ禁止。
- エラーハンドリング: パース失敗/必須欠損はInvalidPayloadとして無視、ICE/DTLS失敗時はPeerLeftを発火しクリーンアップ。
- テレメトリ: レート制御発火回数、接続寿命、往復遅延をメトリクス化できるフックを用意（実装は後続で可）。

## ディレクトリ構造
- `syncer/`（予定）: Syncer本体。WebRTC/IPC/シグナリングアダプタを配置。
- `bloom/` : 既存Bloomコード。今回主に `bloom/ws` 側と連携。
- `docs/specs/20251203-syncer-minimal-p2p.md` : 本仕様。

## テスト項目
1. メッセージ型JSONラウンドトリップ（Pose/Chat/Control/Offer/Answer/Ice）。
2. 2ピア間のPose/Chat配送（DataChannel）。
3. 音声Opusフレームが片方向で届く。
4. レート制御: 1秒21件でRateLimitedとなり、1秒後に回復。
5. 再接続: 同一participant_idで再Offerした場合、旧接続破棄→新接続有効。
6. 切断検知: DataChannel/音声クローズ時にPeerLeftが一度だけ通知され、状態が空になる。

## Syncer API 最小仕様
- 本節は 1PR/1Spec 運用のための最小API仕様。基礎契約とする。

### リクエスト/イベント列挙（案）
- `SyncerRequest`
  - `Join { room_id, participant_id, ice_servers, auth_token }`
  - `SendPose { pose }`
  - `SendChat { message }`
  - （将来拡張用のプレースホルダ: Control系, Signaling系）
- `SyncerEvent`
  - `SelfJoined { room_id, participant_id }`
  - `PeerJoined { participant_id }`
  - `PoseReceived { from, pose }`
  - `ChatReceived { from, message }`
  - `PeerLeft { participant_id }`
  - `RateLimited { stream_kind }`
  - `Error { kind }` （InvalidPayload等を含む想定）

### 戻り値モデル（要判断）
- `Syncer::handle(req) -> Vec<SyncerEvent>` で「1リクエスト→複数イベント」を同期返却。

### 非同期性
- `fn handle(...)` で同期APIを保持し、非同期は Transport 抽象が吸収。フェイクTransportで即時応答を作りやすくする。

### Transport 抽象の粒度
- Reliability/ordering は Transport 内部に隠蔽し、Pose用 unordered/unreliable を設定するか？
  - →API層に露出せず Transport 内部で固定。

### レート制御の返し方
- `RateLimited` を Event として返す（push型）。

### エラー表現（方針）
- API公開面では型付き `Error` を返却し、アプリケーション境界で anyhow に集約（既存規約に従う）。
- `InvalidPayload` など recoverable なものは `SyncerEvent::Error` または `RateLimited` として通知し、致命エラーのみ Result の Err に載せる想定。

## メッセージエンベロープ仕様（DataChannel / Signaling 共通）

### JSON構造
- すべてのDataChannelおよびBloomシグナリングメッセージは、以下の単一オブジェクトで包む。

```json
{
  "v": 1,
  "kind": "pose",
  "body": { "..." }
}
```

- `v`: `u32`。バージョン1のみ受理。未知バージョンを受信した場合は即座に `InvalidPayload::UnsupportedVersion` として破棄する。
- `kind`: 文字列（スネークケース）。下表の列挙のみ許可し、未知値は `InvalidPayload::UnknownKind`。
- `body`: JSONオブジェクト。各 `kind` 固有のスキーマに従う。バイナリ/圧縮は行わず、UTF-8 JSONのみ許容する。
- 予約フィールド: 将来互換のため `meta`（オブジェクト）を予約するが、MVPでは省略必須。受信時に存在しても無視（ログ出力不要）。

### kind一覧と用途
| kind | 用途 | body概要 |
| --- | --- | --- |
| `pose` | Pose配送（unordered/unreliable） | `PoseMessage`（head/hand姿勢 + バージョン）|
| `chat` | テキストチャット | `ChatMessage`（message, sender, timestamp 等）|
| `control.join` | 参加通知 | `ControlJoin`（participant_id, reconnect_token など）|
| `control.leave` | 離脱通知 | `ControlLeave`（participant_id, reason, disconnect_reason）|
| `signaling.offer` | SDP Offer | `SignalingOffer`（sdp, room_id, participant_id, ice_policy）|
| `signaling.answer` | SDP Answer | `SignalingAnswer`（sdp, room_id, participant_id）|
| `signaling.ice` | ICE candidate | `SignalingIce`（candidate, sdp_mid, sdp_mline_index）|

> 備考: Control/Signaling系は後述のセクションで詳細スキーマを追加予定。`kind` 名は将来のネームスペース衝突を避けるため `.` 区切りを許可する。

#### Signalingメッセージ詳細
- Offer/Answer
  - `sdp` はプレーンUTF-8文字列のまま格納し、Base64エンコードは行わない。
  - 必須フィールド: `version`(固定1) / `room_id` / `participant_id` / `auth_token` / (`ice_policy`はOfferのみ)。
  - `sdp` が空文字の場合は `InvalidPayload::SchemaViolation(kind="signaling", reason="missing_sdp")` として即座に破棄する。
- ICE
  - Bloom WS仕様のRelayIceに合わせ、`candidate`, `sdpMid`, `sdpMLineIndex`, `participant_id`, `auth_token` をそのままJSONに乗せる。
  - `candidate` 文字列は1024文字上限。超過した場合は `reason="invalid_candidate"` を返しログにはハッシュのみ出力する。
  - 再送（同一candidateの再通知）はBloom側の制御に委ね、Syncerは受信順にWebRTCスタックへブリッジする。冪等化が必要になった場合に備え、`participant_id`/`room_id`/`candidate`の組をハッシュ化して将来の重複検出用メタデータに活用できるよう、型の余地を残す。

### InvalidPayloadの分類と扱い
- `InvalidPayload` は以下の enum バリアントを持つ。イベント/ログはこの粒度で発火。
  - `MissingVersion`: `v`欠損。
  - `UnsupportedVersion { received: u32 }`: 既知のメジャー以外。
  - `UnknownKind { value: String }`: 未定義 `kind`。
  - `BodyTooLarge { bytes: usize }`: 受信時に 64 KiB（MVP上限、今後設定化）を超過。
  - `BodyJsonMalformed`: `body` のJSONデコード失敗。
  - `SchemaViolation { kind: String, reason: &'static str }`: 必須フィールド欠損や型不整合。
- これらは recoverable とし、`SyncerEvent::Error { kind: ErrorKind::InvalidPayload(...) }` でSidecarへ通知すると同時に、当該メッセージはドロップ。
- RateLimitカウンタには加算しない（攻撃検知は将来のテレメトリで扱う）。

### ロギングベストプラクティス
- すべての `InvalidPayload` は `warn!` レベルで単発ログを出す。`room_id`, `participant_id`, `stream_kind`, `invalid_reason`, `kind`, `payload_size` をフィールドとして構造化記録する。
- `body` の原文ログは禁止。ユーザー入力を含む恐れがあるため、最大でもSHA-256ハッシュ等のダイジェストをフィールド化する（必要になった時のみ導入）。
- 未知 `kind` / `v` はスパム防止のため秒間1回に抑制する（将来の structured rate-limit）。MVPでは `warn!` + `debug!`（詳細原因）を1度だけ出す。
- ログメッセージ例: `warn!(room_id = ?, participant_id = ?, stream_kind = "signaling", invalid_reason = "missing_version", "dropping invalid message");`

### その他ルール
- `body` JSONはトップレベルオブジェクト必須。配列/値のみは `SchemaViolation`。
- 受信側で `meta` を含む未来バージョンを観測した場合は記録せず無視し、互換性を維持。
- 送信側は常に `v=1` を埋め、`kind`/`body`が後方互換になるよう注意する。Breaking changeを入れる場合は `v=2` を定義し、旧バージョンは受信拒否で明示的に検知できるようにする。

## 実装手順（TDD単位）
1. Syncerファサード/APIのテスト作成（Red）
   - IPC相当の抽象インターフェースを定義（sync/async問わず「1リクエスト→複数イベント返却」型）
   - 最小のE2E的ユースケース（join → pose → receive）をフェイクTransportでRed化
   - domain層とtransport層の境界を明確化

2. メッセージ型（Pose/Chat/Control/Signaling）の型定義＋JSONラウンドトリップ（Red→Green）
   - vフィールド必須、InvalidPayload無視の仕様をテストで固定
   - Pose/Chat/Control/Offer/Answer/Ice などすべてのメッセージ型をGreenにする
   - 将来拡張（量子化・圧縮・TURN設定）が壊れない型構造にリファクタ（Refactor）

3. ピア管理（ParticipantTable）のユニットテスト（Red→Green）
   - join/leave、同一participant_id再接続時の旧セッション破棄、PeerJoined/PeerLeftイベント発火
   - Bloom側のjoin/leave通知も含めた整合性をテストで保証
   - 切断理由フィールドはMVP固定値だが、構造だけ確保（Refactor）

ParticipantTable の基本シナリオ
- **新規参加:** 空テーブルで `apply_join(alice)` を呼ぶと `PeerJoined { participant_id: alice }` を1件返し、内部状態に alice が登録される。
  - 状態確認API（`participants()` など）で alice が含まれることを保証。
  - 冪等性確保のため、同じ participant_id を2回連続で join した際の挙動は別フェーズで定義する。
- **離脱:** `apply_join(alice)` で登録済みの状態から `apply_leave(alice)` を呼ぶと、`PeerLeft { participant_id: alice }` が1度のみ返り、内部状態から alice が除去される。
  - 既に離脱済みのparticipant_idに対して `apply_leave` を呼んでもイベントは発生しない。
- **再接続:** `apply_join(alice)` 済みの状態で再度 `apply_join(alice)` を呼ぶと、旧セッションを破棄した上で `PeerLeft { alice }` → `PeerJoined { alice }` の順で2イベントを返し、内部状態は新セッションIDに差し替わる。
- **Controlメッセージ連携:** Bloomから流入する `ControlMessage` を `PendingPeerEvent` として ParticipantTable に適用した場合でも、同一 `leave` の多重適用では `PeerLeft` が1度しか返らない（冪等）。
- **無効 participant_id:** Controlメッセージ経由で `ParticipantId::from_str` に失敗した場合は `SyncerEvent::Error { kind = InvalidParticipantId }` を1度返し、join/leaveイベントは生成しない。

4. RateLimiterのユニットテスト（Red→Green）
   - 1秒20件超過でRateLimited、1秒後に回復をテストで固定
   - IPCセッション単位でのカウント管理を明確化
   - Pose/Chat/Control混在時の境界ケースもテスト

5. ルーティング（participant間のPose/Chat配送ロジック）のユニットテスト（Red→Green）
   - 「A→Bのみ」「A→A除外」「切断済みpeerへの配送禁止」
   - domain層はWebRTC非依存の純ロジックとして仕上げる
   - stream_kindフィールド付与の位置を統一（Refactor）

6. シグナリングアダプタtraitを定義し、モックでOffer/Answer/ICE往復をGreenにする（Red→Green）
   - Bloom WebSocket仕様（20251125版）に準拠した入出力整形
   - 再Offer時の旧PC破棄ロジックをdomain側テストと整合させる
   - シグナリング処理失敗時のInvalidPayload/PeerLeftクリーンアップもカバー

7. Transport抽象化（DataChannel/AudioTrackをラップする）＋フェイクTransport結合テスト（Red→Green）
   - Transport::send/receive の抽象APIを確立
   - DataChannelのorder/unreliable設定をここに閉じ込め、上位層から隠蔽
   - フェイクTransportで「同期的にA→Bで届く」最低限の結合テストを実施

8. WebRTC実装アダプタを追加し、最小結合テスト（Red→Green）
   - 2つのPeerConnectionを同一プロセス内で接続し、bytesが往復する最小Happy Pathのみ保証
   - ICE失敗/DTLS失敗時にPeerLeftが1回だけ発火し、状態が空になることを確認
   - AEC/AGCはWebRTC任せだが、音声トラックが生成・接続されることだけ検証

9. Pose/Chat配送の統合テスト（Syncer API越し）（Red→Green）
   - join → pose/chat送信 → 他方が受信 の同期ユースケースが通ることを確認
   - Poseはunordered/unreliable、Chat/Controlはordered/reliable の差を検証
   - tracingフィールド（room_id/participant_id/stream_kind）付与をログインターフェース経由で確認

10. 音声片方向配送の統合テスト（Red→Green）
    - 単一方向ストリームでOpusフレームが届くことのみ保証（品質/量子化/AEC/AGCは検証対象外）
    - 音声trackの停止時にPeerLeftを重複発火させないことを確認

11. 再接続/切断クリーンアップ統合テスト（Red→Green）
    - 同一participant_idで再Offer → 旧接続クローズ → 新接続有効化
    - DataChannel/AudioTrackが閉じたときにPeerLeftが一度だけ出る
    - ParticipantTableの整合性とTransportの破棄を検証

12. リファクタリングフェーズ
    - domain層・transport層・adapter層が明確な境界を持つよう再整理
    - 将来のTURN導入（ice_servers/ice_policy）に備え、設定構造を後方互換的に整理
    - IPCポート/認証（固定値 INSECURE_DEV）を抽象化し、後続強化に耐える形へ

## 未決事項 / オープンクエスチョン
### Q. 音声のAEC/AGCをどの層で担うか（WebRTC内蔵で足りるか要検証）。
WebRTCのAEC/AGCをフル活用して下さい。

### Q. Poseの量子化・圧縮を導入する時期と方式（MVPは生JSON）。
MVP段階では量子化・圧縮はしなくて良い。必要な場面に遭遇してから考えます。
MVPでは 生JSONによるPose配送を採用する。
理由：
- 圧縮・量子化は実装コストが高く、MVPの価値（動作確認）に寄与しない
- 現フェーズでは帯域最適化よりも、機能の通しやすさ・デバッグ容易性が重要
ただし将来、ネットワーク負荷や遅延が課題化した際の拡張余地として、
- Poseデータのスキーマはバージョン付き（vフィールド必須）とする
- DataChannelはPose専用（unordered/unreliable）を追加可能な構造にしておく

### Q. IPCポート/認証方式（ローカル想定だが多重起動時の識別）。
- トランスポート: localhost TCP
- ポート: 固定ポート。すでに誰かがlistenしている場合、エラーで落とす。
- 多重起動: MVP段階では多重起動を想定しない。1Machine 1Client 1Syncer。
- 認証方式: IPCプロトコルの最初のメッセージに auth_token フィールドを必須とし、MVPでは固定値 "INSECURE_DEV" を使用する。将来、本番環境ではこの値を起動引数または環境変数から与えることで認証強化を行う。

### Q. TURN導入時の挙動・設定受け渡し方式。
MVPでは STUN のみ対応とし、TURNを利用する処理は実装しない。
ただし後方互換性確保のため、以下のみ仕様として先に固定する：
- Syncerの設定は STUN専用ではなく ICEサーバ一覧（ice_servers）として定義
- Room/Client設定に ice_policy フィールドを予約し、MVPでは "default" のみ使用
- 切断理由・接続経路を表すフィールド（disconnect_reason, connection_path）は定義だけ行い、MVPでは固定値を返す
理由：
- TURN自体の導入はMVP後の課題であり、現段階での実装はYAGNI
- ただし設定形式やエラー表現を STUN に固定すると、TURN導入時に破壊的変更が必要になるため、I/Fの型だけは先に固める
