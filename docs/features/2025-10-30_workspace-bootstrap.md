# Workspace Bootstrap 指示書（2025-10-30）

## コンテキスト
- `docs/architecture.md` のディレクトリ構造が未定義のため、RustワークスペースとUnityクライアントの初期レイアウトを定める。
- 今後のVibeCoachingフローを支える基盤として、ユーザーが自力で実装を拡張しやすい最小構成を準備する。

## 目的とゴール
- Rust製サーバー群（Syncer / Sidecar / Bloom）を1つのワークスペースで管理できる状態にする。
- Unityクライアント向けの作業領域を確保し、Rust側との接点を明確化する。
- 以降のタスクで`cargo check`や個別クレートのテストを行えるように準備する。

## 対象範囲
- リポジトリ直下のワークスペース定義 (`Cargo.toml`, `rust-toolchain.toml`)。
- Rustクレートの配置と雛形ディレクトリ。
- Unityクライアント用ディレクトリの仮置き。
- 将来のテスト配置ポリシーの策定。

## 想定ディレクトリ構造
```
/
├─ Cargo.toml              # ワークスペース定義のみ。実装コードは含めない。
├─ rust-toolchain.toml     # (任意) rustcバージョン固定。1.80.0 もしくは最新版stableを想定。
├─ syncer/
│   ├─ Cargo.toml
│   └─ src/lib.rs          # 空のモジュール。今後`src/bin/main.rs`追加予定。
├─ sidecar/
│   ├─ Cargo.toml
│   └─ src/lib.rs
│─ bloom/
    ├─ Cargo.toml
    └─ src/lib.rs
└─ client/                 # Unityプロジェクトを配置する専用ディレクトリ。
```

## 実装方針
- ワークスペースのトップレベル`Cargo.toml`には`[workspace]`セクションと`members`のみを記述する。依存関係は各クレートの`Cargo.toml`で管理する。
- 各クレートの`Cargo.toml`はresolverとmemberのみ定義し、依存関係は追加しない。
- `src/lib.rs` は最小限のプレースホルダ（例: `pub fn stub() {}`）を含むダミー関数の宣言に留め、今後の実装の足掛りにする。
- Unityディレクトリには`.gitkeep`のみ配置し、今後Unity Hubでプロジェクトを生成しやすいよう空ディレクトリを確保する。

## 手順案
1. ルートに`Cargo.toml`と（必要なら）`rust-toolchain.toml`を追加する。
2. `crates/`以下に`cargo new --lib <name>`相当の構造を手動で作成し、ダミー関数だけを定義する。
3. `client/unity/.gitkeep` を追加し、Unity側の入口を明示する。
4. `docs/tests/workspace/2025-10-30_workspace-bootstrap.md` を作成し、後述テストケースを記載する。
5. `cargo check --workspace` が通ることを確認する（現時点では失敗が想定されるが、最終的な完了条件として明記）。

## テスト方針
- Rustワークスペースでのビルドが成立することを確認するためのテストを先に用意する。

## 完了条件
- ルートワークスペース設定、Rustクレート雛形、Unityディレクトリが整備されていること。
- プロジェクト構造が`docs/architecture.md`に追記できるレベルで明確になっていること。

## 次のアクション候補
1. テスト仕様書を作成し、テスト駆動で`cargo check`を通すための最小実装を整える。
2. Syncerクレートに`iroh`依存を追加し、P2P初期化ロジックの詳細設計を進める。
3. SidecarクレートのWebSocketプロトコル仕様を`docs/architecture.md`へ落とし込む。
