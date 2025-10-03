# Requirements Document

## Introduction
Rust 製サイドカーの基盤を検証する第一歩として、cargo workspace 上に試験用の CLI バイナリを整備し、2 つのピアが直接 IP/ポートを指定して P2P セッションを張り、`ping` / `pong` メッセージを往復できるようにする。Bloom や Unity などの上位レイヤーを介さず、サイドカー単体で通信の信頼性と往復遅延を確認することが目的である。

## Alignment with Product Vision
- ピア優先の信頼性: シグナリングを使わず、手動で指定したエンドポイント間で iroh 接続を成立させることで、P2P 基盤の最小コアを検証する。
- 可観測性: CLI から RTT や失敗理由を即時に確認でき、将来 Bloom ダッシュボードへ拡張するための計測指標を固める。
- 設置容易性: cargo workspace の `cargo run -p peer-ping-cli -- listen` などシンプルなコマンドで試験を実施できる状態を整備する。

## Requirements

### Requirement 1: cargo workspace の整備

**User Story:** 開発者として、単一の cargo workspace からサイドカー関連クレートとテスト用 CLI をビルドしたい。そうすれば依存関係とビルド手順を統一できる。

#### Acceptance Criteria

1. WHEN ルート `Cargo.toml` を更新すると THEN members に `rust/crates/sidecar`, `rust/crates/shared`, `rust/crates/peer-cli` が登録される。
2. IF `cargo build` を実行した場合 THEN 3 クレートがビルドに成功し、ワークスペース全体で警告が発生しない。
3. WHEN `cargo xtask setup-peer-cli`（仮）を実行した場合 THEN CLI バイナリ用の設定ファイルや証明書雛形が生成される。

### Requirement 2: 直接接続による P2P ハンドシェイク

**User Story:** テスターとして、ローカルネットワーク上で 2 つの CLI を起動し、片方が待受しもう片方が直接アドレスを指定して接続したい。そうすればシンプルに P2P セッションの成功可否を確認できる。

#### Acceptance Criteria

1. WHEN リスナー側が `cargo run -p peer-cli -- listen --addr 0.0.0.0:9000` を実行すると THEN CLI SHALL iroh ノードを起動し待受を開始する。
2. WHEN 待受が開始された場合 THEN CLI SHALL 自身の Multiaddr（含む公開鍵）を標準出力に表示する。
3. IF ダイヤル側が `cargo run -p peer-cli -- dial --peer <multiaddr>` を実行すると THEN 5 秒以内に Noise ハンドシェイクが完了する。
4. WHEN ハンドシェイクに失敗した場合 THEN CLI SHALL exit code ≠ 0 で終了し、ログに理由（タイムアウト/鍵不一致など）を出力する。

### Requirement 3: `ping/pong` メッセージ往復と計測

**User Story:** テスターとして、接続成立後に `ping` を送信して `pong` を受信し、往復時間を確認したい。そうすればネットワーク品質を判断できる。

#### Acceptance Criteria

1. WHEN 接続が確立すると THEN ダイヤル側 CLI SHALL 自動で `ping` を送信し、受信側 SHALL `pong` を返す。
2. IF `ping` を受信したリスナー側 THEN SHALL 100 ms 以内に `pong` を生成し iroh ストリームで送信する。
3. WHEN `pong` を受信したダイヤル側 THEN SHALL RTT を計測し、標準出力に JSON 形式で `{ "rtt_ms": <value>, "attempt": <n> }` を表示する。

## Non-Functional Requirements

### Code Architecture and Modularity
- **Single Responsibility Principle**: `shared` crate にプロトコル型、`sidecar` に iroh ラッパー、`peer-cli` に CLI ロジックを保持する。
- **Modular Design**: CLI は sidecar crate の公開 API を利用し、直接 iroh に依存しない。
- **Dependency Management**: ワークスペース外のクレート依存は `shared` で集中管理し、再利用性を高める。
- **Clear Interfaces**: `shared::pingpong::Session` など明示的な境界を定義する。

### Performance
- RTT 計測は 150 ms 以下を目標とし、3 回連続で成功するまで自動リトライする。
- CLI は 1 秒に 1 回以上の `ping` を送出しない（不要な負荷を避ける）。

### Security
- Ed25519 鍵ペアを CLI 起動時に生成（もしくは設定ファイルから読み込み）し、Noise プロトコルで暗号化する。
- 表示される Multiaddr には TLS/Noise handshake 情報が含まれることを必須とする。

### Reliability
- CLI は接続失敗時に 500 ms → 1 s → 2 s の指数バックオフで最大 3 回再試行する。
- ハンドシェイク成功後にストリームが切断された場合、即時リトライではなくユーザーへメッセージを表示して終了する。

### Usability
- CLI は `listen` / `dial` サブコマンドと `--addr` / `--peer` オプションを提供し、ヘルプ出力で使用例を案内する。
- 成功時・失敗時ともに exit code とログが整合し、スクリプトから容易に判定できる。
- リスナーは起動時に自身の Multiaddr が表示され、ダイヤル側はその値をコピー＆ペーストすればよい。
