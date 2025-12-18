# Closeフレーム理由コードによる切断扱い仕様

作成日: 2025-12-01
対象: bloom-ws WebSocketサーバ (切断処理)

## 目的

WebSocket Closeフレームを受信した際、CloseCodeに応じて正常/異常切断を判定し、異常切断のみ猶予付きで `leave_room` を実行する。None (理由コードなし) も異常扱いとする。

## 背景

現状 `Message::Close(_)` を一律で正常切断扱いにするため、Closeフレーム送信でも `leave_room` が呼ばれず、異常切断時の通知を期待するテストが失敗している。クライアント実装によっては理由コードを付けないCloseを送ることがあり、これを異常扱いにしたい。

## 振る舞い

- CloseCodeが `Normal` または `Away` のときのみ正常切断として扱う。
- CloseFrameが `None`、または上記以外のCloseCodeのときは異常切断として扱う。
- 異常切断時: `ABNORMAL_DISCONNECT_GRACE` 経過後に `handle_abnormal_close` を実行し、`leave_room` を1回だけ呼び、残存参加者へ `PeerDisconnected` と最新 `RoomParticipants` を送信する（既存実装踏襲）。
- 正常切断時: 既存どおり異常処理は行わない（Leaveを事前に送るのはクライアント責務）。

## テスト観点（本サイクルでカバー）

- Close(None) を送信した場合、猶予経過後に `leave_room` が1回だけ呼ばれ、残存参加者へ `PeerDisconnected` / `RoomParticipants` が届く。
- Close(Normal) を送信した場合、異常経路に入らず `leave_room` は呼ばれない（現状仕様を維持）。

## 非スコープ

- 正常切断時にサーバ側で自動Leaveするかどうかの再設計は本サイクルでは行わない。
