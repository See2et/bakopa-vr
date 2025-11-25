# Bloomシグナリング機能 仕様書 (MVP)

作成日: 2025-11-25
対象: SuteraVR Bloom (シグナリング専用MVP)

## 概要
Bloomはシグナリング専用の役割を担い、クライアント⇔Bloom間をWebSocketで接続する。音声・位置同期・テキストチャットなど実データはクライアント間WebRTC（SRTP + DataChannel）で直接流し、中央サーバー負荷を最小化する。MVPではWebRTCハンドシェイクに必要な最小限のシグナリングを提供する。

## スコープ / 非スコープ
- スコープ: ルーム作成・参加・離脱、参加者リスト管理、SDP/ICE候補の中継、接続状態のブロードキャスト。
- 非スコープ: WebSocketによるメディア/位置/テキストの中継（すべてWebRTC経由）、認証・課金、永続的UGC管理、永続ストレージ、メディア品質制御、モデレーションUI。

## 用語
- Room: P2P接続単位。room_idで識別。
- Participant: Roomに参加するクライアント。participant_idで識別。
- Offer/Answer/ICE: WebRTCシグナリングメッセージ。
- Signaling Channel: Bloomが提供するWebSocket接続。全メッセージはJSONでやりとり。
- Media/Data Path: クライアント間WebRTC接続（音声はSRTP、位置/テキストはDataChannel）。

## 前提
- シグナリング通信路はWebSocket over TLS（平文WSは開発用途のみ）。
- WebRTC経路はSTUNでネゴシエートし、必要に応じてTURNにフォールバックできる設定をクライアントが保持する（BloomはTURNを中継しない）。
- 認証はMVPでは省略し、room_idのみでアクセスできる前提（将来トークン認証を追加可能な設計）。
- 状態はプロセスメモリに保持し、Bloomプロセス再起動で消える（MVP）。

## 機能要件
1) ルーム管理
- Room作成: `CreateRoom`要求で新しいroom_idを払い出し、作成者を参加者として登録。
- 参加/離脱: `JoinRoom`で参加者登録、`LeaveRoom`またはWS切断で離脱扱い。
- 参加者リスト配信: 参加/離脱時に`RoomParticipants`イベントを全参加者へブロードキャスト。
- 同時接続上限: MVPでは1 Room あたり最大 8 Participant（ハードコード可）。超過時は`RoomFull`エラーを返す。

2) シグナリング中継（WebSocket）
- Offer/Answer/ICEの中継: 送信者→Bloom→指定受信者へ単一配信。受信者不在なら`ParticipantNotFound`エラー。
- 再送方針: Bloomはステートレス中継で再送・キューイングはしない。受信者がオフラインの間に届いたメッセージは破棄。
- メッセージ検証: JSON schemaを検証し、未知フィールドは無視せずエラーとする（プロトコル早期失敗）。
- BloomはWebRTCメディア/データフローを中継しない。

3) 接続状態通知
- `PeerConnected`/`PeerDisconnected`イベントをroom内全員に通知（Join/Leaveと同時でよい）。

4) エラーハンドリング
- 形式不正: `InvalidPayload`エラーを直ちに返却し、そのメッセージは処理しない。
- レート制御:
    - 計測対象メッセージ: クライアント→Bloomのシグナリング全種（`Offer` / `Answer` / `IceCandidate` / `CreateRoom` / `JoinRoom` / `LeaveRoom`）。
    - 単位: 1 接続 = 1 WebSocket セッションごとに独立してカウントする（同じ participant でも再接続したら別セッションとして扱う）。
    - 時間窓: 直近 1 秒のローリングカウントで上限 20 件。
    - 振る舞い: 21 件目で `RateLimited` エラーを即時返却し、同一セッションではその後 1 秒間の到着メッセージをすべてドロップし `RateLimited` のみ返す。1 秒経過後にカウントをリセットして再度受け付ける。

## 非機能要件
- レイテンシ: シグナリング1ホップあたりp95 < 50ms（プロセス内計測）。
- 信頼性: 1 Room 8人接続・Offer/Answer/ICE計50メッセージ/秒まで落ちず処理。
- ロギング: `tracing`でspan単位にroom_id/participant_idを必須フィールドとして付与。subscriber初期化はbinary側。
- ロギング: `tracing`でspan単位にroom_id/participant_idを必須フィールドとして付与。subscriber初期化はbinary側。
  - WSハンドラ（handshake含む）では、各spanに少なくとも`participant_id`フィールドを必ず載せる。
  - roomに紐づく処理（Offer/Answer/Ice/Leaveなど）では、spanに`room_id`も必ず含める。
  - RateLimited時のwarnログは、構造化フィールド`participant_id`（文字列ID）を必須とする。
- セキュリティ: 機密情報をログ出力しない。将来の認証導入を阻害しないAPI設計（Authorizationヘッダやtokenフィールドを拡張可能に）。

## ネットワーク/フロー概要
1. クライアントがBloomにWS接続し、`CreateRoom`または`JoinRoom`を送信。
2. Bloomが参加者リストを管理し、参加/離脱をWSイベントでブロードキャスト。
3. クライアントAが`Offer(to=B)`をBloomへ送信。BloomはBに転送。
4. クライアントBが`Answer(to=A)`をBloomへ送信。BloomはAに転送。
5. 双方が`IceCandidate`をBloom経由で交換し、STUN/TURNを用いてP2Pパスを確立。
6. P2P確立後、音声（SRTP）と位置/テキスト（DataChannel）はクライアント間WebRTCで直接流れる。Bloomは関与しない。
7. 切断/タイムアウト時、Bloomが`PeerDisconnected`と最新`RoomParticipants`をWSで通知。

## WebSocketメッセージ仕様良めーじ（JSON）
```
// クライアント→Bloom
{ "type": "CreateRoom" }
{ "type": "JoinRoom", "room_id": "..." }
{ "type": "LeaveRoom" }
{ "type": "Offer", "to": "participant_id", "sdp": "..." }
{ "type": "Answer", "to": "participant_id", "sdp": "..." }
{ "type": "IceCandidate", "to": "participant_id", "candidate": "..." }

// Bloom→クライアント イベント
{ "type": "RoomCreated", "room_id": "...", "self_id": "..." }
{ "type": "RoomParticipants", "room_id": "...", "participants": ["..."] }
{ "type": "PeerConnected", "participant_id": "..." }
{ "type": "PeerDisconnected", "participant_id": "..." }
{ "type": "Offer" | "Answer" | "IceCandidate", "from": "participant_id", ...payload }
{ "type": "Error", "code": "RoomFull" | "InvalidPayload" | "ParticipantNotFound" | "RateLimited" | "Internal", "message": "..." }
```

## ディレクトリ構造（モノレポ前提の提案）
- `Cargo.toml` : Rustワークスペースルート
- `bloom/`：Bloomのディレクトリ
    - `Cargo.toml`
    - `api/`：プロトコルの定義だけを切り出す
    - `core/`：状態やビジネスロジック
    - `ws/`：WebSocketの取り扱い
- `syncer/`：将来的に追加
- `sidecar/`：将来的に追加
- `docs/`
  - `product.md`, `architecture.md`
  - `specs/20251125-bloom-signaling.md` : 本仕様

## テスト項目
- ルーム作成: CreateRoomでroom_id/self_idを返し、作成者が参加者リストに含まれる。
- 参加/離脱: JoinRoomで既存roomに参加でき、Leaveまたは切断でRoomParticipantsから除外される。
- 上限超過: 8人目まで成功、9人目はRoomFullエラー。
- シグナリング転送: Offer/Answer/ICEが指定受信者にだけ届く。不在受信者ならParticipantNotFound。
- 不正ペイロード: 必須フィールド欠如でInvalidPayloadエラー、内部状態は変化しない。
- レート制御: 1秒に21個送信で超過時にRateLimitedを返し、1秒後に再び許容される。
- ログ: tracing spanがroom_id/participant_idフィールドを持つ（ユニットテストでsubscriberをモックして検証）。
- WebRTC前提の整合性: Bloomがメディアを中継しないこと、TURNフォールバック時もシグナリングAPIが変わらないことを確認。

## 実装手順（TDD単位）
1. `bloom-api`にメッセージ型を定義するテストを書き、JSONラウンドトリップをRed→Green。
2. `bloom-core`にRoom/Participant管理のユニットテスト（CreateRoom/Join/Leave/上限超過）を追加しGreen。
3. シグナリング中継のルーティングテスト（Offer/Answer/ICEの宛先分配、不在時エラー）。
4. WebSocketサーバのハンドラ統合テスト（メッセージ受信→core呼び出し→応答送信）。
5. レート制御のテスト（1接続あたりメッセージ数カウント）。
6. ログ付与のテスト（instrument付きハンドラでroom_id/participant_idがspanに載る）。
7. WebRTCフロー整合テスト（モックSTUN/TURN設定でOffer/Answer/ICE交換後にBloomがメディアを中継しないことを確認）。

## 未決事項 / オープンクエスチョン
- 認証方式（トークン化の方式と導入タイミング）。
    - 将来的に公開鍵認証、あるいはBloomを介したAuthを導入するかもしれません。現段階は放念してOK。
- room_id/participant_idのフォーマット（UUIDか短縮IDか）。
    - とりあえずUUIDで進めましょう。
- メッセージ再送やキューイングを将来導入するか。
    - MVPではシグナリングメッセージの再送・キューイングはサーバー側で行わない。
    - 接続失敗時はクライアント側が再度Create/Join/Offer/Answerなどを行う責務とする。
- 部屋のアイドルタイムアウト値。
    - とりあえず60sec程度。
- TURNサーバ配置戦略とコスト上限（フォールバック頻度次第で検討）。
    - 本仕様書のスコープではSTUNのみ。TURNは将来的に扱う。
