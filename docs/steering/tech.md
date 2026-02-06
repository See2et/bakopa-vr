# 技術スタック

## アーキテクチャ

- Federation による運営分散 + P2P による高頻度同期の二層構成
- Bloom は WebRTC シグナリング専用、Syncer は P2P 同期専用

## コア技術

- **言語**: Rust (edition 2021)
- **ランタイム**: Tokio
- **通信**: WebSocket (tokio-tungstenite), WebRTC (webrtc/webrtc-media, feature で切替)
- **シリアライズ**: serde / serde_json
- **ログ**: tracing (+ tracing-subscriber)

## 主要ライブラリ

- tokio / tokio-tungstenite
- webrtc / webrtc-media (syncer の feature)
- serde / serde_json
- tracing / tracing-subscriber

## 開発基準

### エラーハンドリング

- アプリ境界は `anyhow::Result` を許容し、ライブラリ層は型付きエラーを基本とする
- `unwrap` / `expect` は原則禁止（テスト等を除く）

### ロギング

- すべて `tracing` を使用し、ライブラリ側で subscriber を初期化しない

### テスト

- `cargo test` を基本
- `bloom/ws` と `syncer` に統合テストが多い

## 開発環境

### 必須ツール

- Rust toolchain
- Cargo

### よく使うコマンド

```bash
# ビルド: cargo build
# テスト: cargo test
```

## 主要な技術的判断

- ルートは Rust workspace とし、bloom(api/core/ws) と syncer を分割
- シグナリングは WebSocket、同期は P2P/WebRTC に分離
- syncer は WebRTC を feature で切り替え可能にする

---
更新日: 2026-01-22
