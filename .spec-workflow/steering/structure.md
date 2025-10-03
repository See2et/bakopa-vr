# Project Structure

## Directory Organization

```
project-root/
├── Cargo.toml               # Rust workspace ルート（sidecar / bloom / shared crate を登録）
├── Cargo.lock
├── unity-client/            # Unity プロジェクト（VR/デスクトップ両対応クライアント）
│   ├── Assets/
│   ├── Packages/
│   ├── ProjectSettings/
│   └── UGC/                 # Bloom 経由で配信される AssetBundle のビルド成果物
├── rust/
│   ├── crates/
│   │   ├── sidecar/         # Rust 製サイドカー（bakopa-net）: iroh + IPC 実装
│   │   ├── shared/          # 共通ライブラリ（MessagePack 型、プロトコル定義）
│   │   └── bloom/           # Bloom サーバー（フェデレーション API、STAN ブローカー）
│   ├── xtask/               # cargo xtask によるビルド/CI 補助コマンド
│   └── target/              # Rust ビルド成果物（.gitignore）
├── dashboard/               # Bloom メトリクス可視化 (Next.js + Grafana)
├── docs/                    # プロトコル仕様、運用 Runbook、アーキ資料
├── scripts/                 # ビルド・デプロイ補助スクリプト (PowerShell / Bash / Python)
├── .spec-workflow/          # スペック・ステアリング管理
└── README.md
```

> 現状リポジトリは Rust サイドカーの雛形 (`src/main.rs`) のみだが、MVP 完了までに上記 cargo workspace 構成へ段階的に整理する。

### Rust Workspace パターン
- ルート `Cargo.toml` では `members = ["rust/crates/sidecar", "rust/crates/shared", "rust/crates/bloom", "rust/xtask"]` を定義。
- `cargo build -p sidecar` や `cargo test -p bloom` で個別ビルド、`cargo xtask bundle` で Unity 向け DLL/サービスを一括出力。
- `rust/crates/shared` で MessagePack プロトコル、型、Bloom API クライアントを共通化し、sidecar/bloom 双方から依存。

## Naming Conventions

### Files
- **Unity スクリプト**: `PascalCase.cs`
- **Rust crate ディレクトリ**: `snake_case`
- **Bloom サービスモジュール**: `kebab-case` ディレクトリ（例: `room-registry/`）
- **テスト**: Rust は `mod_name_tests.rs`、Unity は `*_Tests.cs`

### Code
- **Classes/Types**: C# / Rust ともに `PascalCase`
- **Functions/Methods**: C# は `PascalCase`、Rust は `snake_case`
- **Constants**: `UPPER_SNAKE_CASE`
- **Variables**: C# は `camelCase`、Rust は `snake_case`

## Import Patterns

### Import Order
1. 外部依存（Unity: `using UnityEngine;`、Rust: `use tokio::...`）
2. 同一ドメイン内モジュール（`SuteraVR.Core`, `crate::protocol`）
3. 相対インポート / ローカルユーティリティ
4. 条件付き/プラットフォーム別インポート（`#if UNITY_EDITOR`、`cfg(target_os = "android")`）

### Module/Package Organization
- Unity は `SuteraVR.*` 名前空間で機能別に整理（`Networking`, `Interaction`, `Bootstrap`）。
- Rust workspace 内では `sidecar/src/{ipc,transport,session}`、`bloom/src/{services,stan,federation}`、`shared/src/{codec,types,api}` とレイヤー分割。
- `shared` crate で定義した型を通じて cross-crate 依存を制御。Bloom から Unity へ直接依存させない。

## Code Structure Patterns

### Module/Class Organization
1. 依存モジュール / using 宣言
2. 定数・設定（`const` / `ScriptableObject` / `Config`）
3. 型・プロトコル定義（MessagePack struct, gRPC stub）
4. メイン実装（セッション制御、API ハンドラ、UI）
5. ヘルパー / ユーティリティ
6. 公開エクスポート / MonoBehaviour 登録

### Function/Method Organization
- 入力検証・権限チェック
- 核となる処理（P2P 接続、Bloom コール）
- エラー処理とリトライ戦略
- ログ・メトリクス発行
- 明示的な return / Task 完了

### File Organization Principles
- Unity: 1 MonoBehaviour / ScriptableObject につき 1 ファイル。
- Rust: crate ごとに `lib.rs` / `main.rs` を薄く保ち、詳細を `mod.rs` + サブモジュールへ分割。公開 API は `pub(crate)` に制限。
- Bloom: gRPC サービスと STAN ハンドラを `domain/` 層に集約し、外部 I/O と分離。

## Code Organization Principles

1. **Single Responsibility**: ピア接続、Bloom API、UI 表示を明確に分離。
2. **Modularity**: サイドカーと Bloom は cargo workspace 内の別 crate として疎結合化。
3. **Testability**: `shared` crate にテスト用フィクスチャを集約、Unity では PlayMode テストでプロトコルを検証。
4. **Consistency**: Federation + P2P の二層モデルに沿って層別責務を維持。

## Module Boundaries
- **Unity クライアント** → IPC 経由で Rust サイドカーへ依存。Bloom へ直接アクセスしない。
- **Rust サイドカー (sidecar crate)** → Bloom API + STAN ブローカーにアクセス。Unity にはイベント/状態を push するのみ。
- **Bloom (bloom crate)** → Federation 層と STAN サーバーでルーム状態を同期。P2P トラフィックは扱わない。
- **shared crate** → プロトコルや DTO を提供し、Unity 側へは C# バインディングを生成。
- **Dashboard** → Bloom メトリクスの read-only 消費者。コア処理へ書き込み不可。

## Code Size Guidelines
- **Unity Scripts**: 300 行以内。ロジックが複雑な場合は `Interaction` / `UI` レイヤーで分割。
- **Rust Modules**: 400 行以内。非同期制御はサブモジュールに委譲。
- **Functions/Methods**: 40 行を超える場合はヘルパー関数へ抽出。
- **Nesting Depth**: 3 レベルを上限とし、複雑な分岐は状態マシンへ切り出す。

## Dashboard/Monitoring Structure
```
dashboard/
├── src/
│   ├── pages/              # Next.js ページ
│   ├── components/         # 再利用 UI
│   ├── lib/                # Grafana/Prometheus クライアント
│   └── styles/
└── grafana/
    ├── dashboards/         # JSON 設定
    └── datasources/        # Prometheus/Timestream 設定
```
- Bloom は STAN と Postgres のメトリクスを Prometheus Exporter に集約。
- サイドカー・Bloom が停止しても監視コンポーネントは独立稼働。
- CLI から `npm run dev:web`、`cargo xtask bloom-dev` でスタックを同時起動。

## Documentation Standards
- `docs/protocol/` に MessagePack / gRPC スキーマを保存し、変更は ADR で追跡。
- Unity プロジェクトには `docs/unity/SETUP.md`、Rust ワークスペースには `docs/rust/CONTRIBUTING.md` を配置。
- Bloom の Federation ルールは `docs/bloom/` に手順化し、STAN トポロジと DR 計画を明記。
- 主要モジュールは README とアーキ図を併記し、Pull Request で更新を義務化。
