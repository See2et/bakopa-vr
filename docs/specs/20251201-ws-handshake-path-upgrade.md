# WSハンドシェイクのパス/Upgrade検証仕様

作成日: 2025-12-01  
対象: bloom-ws WebSocketサーバ（ハンドシェイク層）

## 目的
WebSocket接続要求に対し、(1) パス不一致と (2) Upgradeヘッダ欠如/不正 を区別して応答する。従来は `/ws` 以外のパスも 426 Upgrade Required を返していたが、HTTP意味論に沿って 404 を返す。

## 背景
`bloom/ws/src/server.rs` のハンドシェイクコールバックではパス検証しか行っておらず、`/ws` 以外のリクエストにも 426 を返していた。シニアレビューで 404 の方が望ましいとの指摘を受け、Upgrade欠如時の応答も明示的に定義する。

## 振る舞い
- 許可パスは `/ws` のみ。
- `/ws` 以外のパス: `404 Not Found` を返す。Upgrade/Connection ヘッダは付けない。
- パスが `/ws` だが `Upgrade: websocket` または `Connection: Upgrade` が欠如・不正な場合: `426 Upgrade Required` を返す。レスポンスヘッダに `Upgrade: websocket` と `Connection: Upgrade` を含める。
- `/ws` かつ Upgradeヘッダが適正な場合: 従来どおり `101 Switching Protocols` でWebSocketを確立する。
- その他の必須ヘッダ（`Sec-WebSocket-Key` など）が欠如する場合の扱いは現行ライブラリのデフォルト（400系エラー）に委ねる。

## テスト観点（本サイクルで実装するもの）
- `/foo` など `/ws` 以外へのHTTPリクエストに対し、ステータスラインが 404 となり Upgrade/Connection ヘッダが付与されないこと。
- `/ws` へ UpgradeヘッダなしでHTTPリクエストした場合、ステータスが 426 となり `Upgrade: websocket` と `Connection: Upgrade` が付くこと。

## 非スコープ
- TLS終端やHTTP/2経由のWebSocket Upgrade対応は本サイクルでは扱わない。
- `Sec-WebSocket-Key` 等の不備に対するステータスコード詳細設計は行わない（ライブラリ準拠）。
