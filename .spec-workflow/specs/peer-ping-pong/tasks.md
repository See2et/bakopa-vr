# Tasks Document

- [x] 1. Cargo workspace再構築とcrateスキャフォールド
  - Files: Cargo.toml, rust/crates/shared/Cargo.toml, rust/crates/sidecar/Cargo.toml, rust/crates/peer-cli/Cargo.toml, rust/xtask/Cargo.toml, rust/crates/sidecar/src/lib.rs, rust/crates/peer-cli/src/main.rs (雛形)
  - 既存 `src/main.rs` を workspace 構成に合わせて `rust/crates/sidecar` へ移設し、共通の Rust edition / lint 設定を整える
  - `shared`, `sidecar`, `peer-cli`, `xtask` の crate を空の形で用意し、`cargo build` が通る最小コードを配置する
  - 目的: cargo workspace を Requirements R1 に沿って整備するための土台づくり
  - _Leverage: Cargo.lock, structure.md, tech.md_
  - _Requirements: R1_
  - _Prompt: Implement the task for spec peer-ping-pong, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Rust Infrastructure Engineer | Task: Restructure the repository into a cargo workspace with shared, sidecar, peer-cli, and xtask crates per requirement R1, migrating existing src/main.rs into the new layout and establishing baseline Cargo manifests | Restrictions: Preserve buildability at each step, follow paths defined in structure.md and tech.md, avoid introducing unused crates | Success: cargo build from repo root succeeds, new crates compile with placeholder code, legacy src/main.rs is removed from the root_

- [ ] 2. shared crateのプロトコル定義実装
  - Files: rust/crates/shared/src/lib.rs, rust/crates/shared/src/pingpong.rs, rust/crates/shared/src/config.rs
  - `PingMessage`, `PongMessage`, `RttReport`, `SessionConfig` を定義し、serde / rmp-serde でシリアライズできるようにする
  - 鍵生成・Multiaddr 表示ユーティリティ (`keypair.rs`, `multiaddr.rs`) を用意する
  - 目的: Requirements R2, R3 の基盤となるデータモデル/APIを提供する
  - _Leverage: design.md (Components/Data Models), tech.md のセキュリティ要件_
  - _Requirements: R2, R3_
  - _Prompt: Implement the task for spec peer-ping-pong, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Rust Systems Developer specializing in serialization | Task: Implement shared crate data structures and helpers (ping/pong messages, RTT report, session config, keypair & multiaddr utilities) satisfying requirements R2 and R3 | Restrictions: Use serde and rmp-serde, derive Clone/Serialize/Deserialize consistently, keep helper APIs pure and easily testable | Success: shared crate builds independently, serialization round-trip tests pass, helper fns expose safe abstractions_

- [ ] 3. sidecar crateのPeerSession作成
  - Files: rust/crates/sidecar/src/lib.rs, rust/crates/sidecar/src/session.rs, rust/crates/sidecar/src/error.rs
  - iroh を用いたノード初期化・Noise ハンドシェイク・ストリーム送受信を `PeerSession` として実装する
  - バックオフ付き再試行、ping timeout、ハンドシェイク失敗時のエラー型を定義する
  - 目的: Requirements R2, R3 の機能的要件を満たす通信レイヤーを提供
  - _Leverage: design.md Architecture/Components, tech.md iroh 設計_
  - _Requirements: R2, R3_
  - _Prompt: Implement the task for spec peer-ping-pong, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Rust Networking Engineer | Task: Build the sidecar PeerSession abstraction over iroh to handle listen/dial, Noise handshake, ping/pong send-recv, and retry logic per requirements R2 and R3 | Restrictions: Keep APIs async using tokio, surface errors via a dedicated enum, do not embed CLI-specific logging | Success: sidecar crate compiles with iroh integration, unit or integration tests cover handshake success/failure, retry and timeout logic validated_

- [ ] 4. peer-cli コマンド実装
  - Files: rust/crates/peer-cli/src/main.rs, rust/crates/peer-cli/src/commands/listen.rs, rust/crates/peer-cli/src/commands/dial.rs, rust/crates/peer-cli/src/output.rs
  - Clap で `listen`/`dial` サブコマンドを定義し、`sidecar::PeerSession` を呼び出す
  - listen 起動時に生成 Multiaddr を表示し、dial 成功時に RTT などを JSON 出力する
  - 目的: Requirements R2, R3 達成のための手動操作インターフェースを提供
  - _Leverage: design.md Components, requirements.md Acceptance Criteria_
  - _Requirements: R2, R3_
  - _Prompt: Implement the task for spec peer-ping-pong, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Rust CLI Engineer | Task: Implement peer-cli with listen/dial subcommands that call the sidecar API, print self multiaddr on listen, and emit RTT JSON on pong per requirements R2 and R3 | Restrictions: Avoid global mutable state, ensure structured JSON output for reports, return meaningful exit codes | Success: cargo run -p peer-cli listen/dial works locally, CLI prints expected addresses/JSON, error cases handled gracefully_

- [ ] 5. テストとドキュメント整備
  - Files: rust/crates/shared/tests/pingpong_tests.rs, rust/crates/sidecar/tests/session_tests.rs, rust/crates/peer-cli/tests/integration.rs, docs/protocol/ping-pong.md
  - shared のシリアライズテスト、sidecar のモック/ローカル接続テスト、CLI の E2E テスト (ignored) を追加
  - 試験手順と CLI 使い方をドキュメント化する
  - 目的: Requirements R2, R3 の検証と運用ガイドを確立
  - _Leverage: design.md Testing Strategy, requirements.md Acceptance Criteria_
  - _Requirements: R2, R3_
  - _Prompt: Implement the task for spec peer-ping-pong, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Rust QA Automation Engineer | Task: Add serialization/unit/integration tests for shared, sidecar, and peer-cli plus author usage docs following requirements R2 and R3 | Restrictions: Mark network-heavy tests with #[ignore], keep docs concise with command examples, ensure tests run in CI with default flags | Success: `cargo test --all` passes (ignoring annotated tests), documentation explains listen/dial workflow, acceptance criteria demonstrably met_
