# 技術スタック

## アーキテクチャ

- Federation による運営分散 + P2P による高頻度同期の二層構成
- Bloom は WebRTC シグナリング専用、Syncer は P2P 同期専用
- メディア/位置など高頻度データは Bloom で中継しない（P2P で配送）
- Client は Godot Adapter と Rust Domain を分離した Ports & Adapters
  (Hexagonal) を採用する

## コア技術

- **言語**: Rust (edition 2021)
- **ランタイム**: Tokio
- **通信**: WebSocket (tokio-tungstenite), WebRTC (webrtc/webrtc-media, feature で切替)
- **シリアライズ**: serde / serde_json
- **ログ**: tracing (+ tracing-subscriber)
- **Client ECS**: bevy_ecs
- **Client Engine Bridge**: godot-rust (GDExtension)
- **Error**: thiserror（境界で anyhow）

## 主要ライブラリ

- tokio / tokio-tungstenite
- webrtc / webrtc-media (syncer の feature)
- serde / serde_json
- tracing / tracing-subscriber

## 通信プロトコル方針

- Bloom の WebSocket メッセージは PascalCase `type` を前提に扱う
- Syncer は `SyncMessage` Envelope v1 を採用し、`kind` で
  `pose/chat/voice/control.*/signaling.*` を識別する
- WebRTC DataChannel の既定 label は `sutera-data`
- Pose 同期は unordered/unreliable チャネル特性を前提に設計する
- 音声は Opus トラック連携を前提に扱う

## 運用・実装制約

- Bloom のレート制御は 1 秒あたり 20 メッセージ/セッションを基準とする
- Syncer のレート制御も 1 秒あたり 20 件/セッションを基準とする
- `tracing` の span には `room_id` / `participant_id` などの識別子を付与する
- subscriber 初期化はバイナリクレート（エントリポイント）側のみで実施する

## クライアント境界設計（Godot + bevy_ecs）

- Domain (`bevy_ecs`) は純 Rust を維持し、Godot 型 (`Node`,
  `InputEvent`, `Variant`, `Gd<T>`) を持ち込まない
- Adapter (`godot-rust` / GDExtension) は Godot API 呼び出しと
  メインスレッド制約を担当する
- Domain とは input port / output port を介して通信し、
  input-state-output の流れを固定する
- Godot イベントは Adapter で Domain 入力型に変換し、
  Domain 出力を Adapter が Godot ノードへ反映する

## 開発基準

### エラーハンドリング

- アプリ境界は `anyhow::Result` を許容し、ライブラリ層は型付きエラーを基本とする
- `unwrap` / `expect` は原則禁止（テスト等を除く）

### ロギング

- すべて `tracing` を使用し、ライブラリ側で subscriber を初期化しない

### テスト

- `cargo test` を基本
- `bloom/ws` と `syncer` は統合テストを中心に検証する
- `client/domain` は Godot から独立した純 Rust ユニットテストを基本とする
- `client/godot-adapter` は Godot ランタイム依存を最小化し、
  ユニットテストと統合テストを実行する

## 開発環境

### 必須ツール

- Rust toolchain
- Cargo

### よく使うコマンド

```bash
# サーバー共通
# ビルド: cargo build
# テスト: cargo test

# クライアント（Godot GDExtension）: ローカル向け
# Linux: scripts/build-client-core-linux.sh [--release]
# macOS: scripts/build-client-core-macos.sh [--arch x86_64|arm64] [--release]
# Windows(gnu): scripts/build-client-core-windows.sh [--release]
#   前提: mingw ツールチェーンを PATH に追加
#   任意: CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS で追加ライブラリパスを指定

# クライアント（Godot GDExtension）: クロスビルド推奨
# cargo-cross を使う場合（Docker 必須）:
#   cargo install cross --git https://github.com/cross-rs/cross
#   cross build -p client-godot-adapter --target x86_64-pc-windows-gnu --release
# Docker 直接利用例（追加依存を固定したい場合）:
#   docker run --rm -it -v "$PWD":/work -w /work \
#     ghcr.io/cross-rs/x86_64-pc-windows-gnu:latest \
#     cargo build -p client-godot-adapter --target x86_64-pc-windows-gnu --release
```

## 主要な技術的判断

- ルートは Rust workspace とし、bloom(api/core/ws) と syncer を分割
- シグナリングは WebSocket、同期は P2P/WebRTC に分離
- syncer は WebRTC を feature で切り替え可能にする
- クライアントは `client/domain` と `client/godot-adapter` を分離し、
  Godot 依存を Adapter 層に閉じ込める
- Domain ロジックはユニットテスト可能な純粋処理を優先し、
  Godot ランタイム依存のテストは最小化する

---
更新日: 2026-02-07
