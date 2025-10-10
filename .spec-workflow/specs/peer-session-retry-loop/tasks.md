# Tasks Document

- [x] 1. リトライ対応ダイヤルループの実装
  - File: rust/crates/sidecar/src/session.rs
  - `PeerSession::dial` を再試行ループ化し、`SessionConfig::max_retries` と `retry_backoff_ms` を尊重するヘルパーを追加する。RTT 集計が成功試行の回数を反映するよう `SessionInner` の試行管理を更新する。
  - Purpose: 再試行制御をセッション層に統合し、接続成功率を向上させる。
  - _Leverage: rust/crates/sidecar/src/session.rs, rust/crates/sidecar/src/error.rs_
  - _Requirements: 1.1,1.2,1.3,2.3_
  - _Prompt: Implement the task for spec peer-session-retry-loop, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Rust Networking Engineer | Task: Introduce a retry-aware `PeerSession::dial` loop using Tokio primitives, honoring existing configuration fields, and updating attempt tracking so RTT reports count the successful attempt | Restrictions: Avoid adding new external dependencies, keep helper functions private to session.rs, maintain backwards-compatible public APIs | _Leverage: rust/crates/sidecar/src/session.rs, rust/crates/sidecar/src/error.rs | _Requirements: 1.1,1.2,1.3,2.3 | Success: Dial retries respect max/backoff settings, failures return aggregated context, RTT attempts reflect the final successful try_

- [x] 2. `PeerEvent::DialRetry` と CLI 出力の拡張
  - File: rust/crates/sidecar/src/session.rs
  - `PeerEvent` に再試行イベントを追加し、`dial` ループから発行する。CLI 側 (`rust/crates/peer-cli/src/commands/dial.rs`) のイベントループを更新し、再試行状況を構造化ログと人間向けメッセージで表示する。
  - Purpose: 再試行の可観測性を高め、オペレーターへ進捗を伝達する。
  - _Leverage: rust/crates/peer-cli/src/commands/dial.rs, rust/crates/sidecar/src/session.rs_
  - _Requirements: 2.1,2.2,2.3_
  - _Prompt: Implement the task for spec peer-session-retry-loop, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Rust CLI Engineer | Task: Add a `PeerEvent::DialRetry` variant emitted by the dial loop and surface those events in the peer-cli dial command with both structured tracing and concise user-facing messages | Restrictions: Do not break existing Ping/Pong event handling, keep logging consistent with current tracing configuration, avoid blocking the async event loop | _Leverage: rust/crates/sidecar/src/session.rs, rust/crates/peer-cli/src/commands/dial.rs | _Requirements: 2.1,2.2,2.3 | Success: CLI shows retry progress per attempt and structured logs include attempt/backoff metadata_

- [x] 3. リトライ経路を検証する自動テストの追加
  - File: rust/crates/sidecar/tests/session_tests.rs
  - 遅延リスナーを用いた「再試行後成功」と、応答しないアドレスを使う「再試行尽きて失敗」の統合テストを実装する。必要に応じてテスト専用ヘルパーを追加する。
  - Purpose: 回帰を防ぎ、複数試行の振る舞いを自動検証する。
  - _Leverage: rust/crates/sidecar/tests/session_tests.rs, rust/crates/sidecar/src/session.rs_
  - _Requirements: 1.1,1.3_
  - _Prompt: Implement the task for spec peer-session-retry-loop, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Rust Integration Test Engineer | Task: Add tokio-based integration tests that cover delayed listener success and maxed-out retry failure, asserting emitted events and final error context | Restrictions: Keep tests deterministic with bounded timeouts, reuse existing helper patterns, skip tests only when platform permissions block socket binding | _Leverage: rust/crates/sidecar/tests/session_tests.rs, rust/crates/sidecar/src/session.rs | _Requirements: 1.1,1.3 | Success: Tests fail on regressions in retry logic and pass reliably in CI_

- [ ] 4. CLI ドキュメントの更新
  - File: docs/protocol/ping-pong.md
  - リトライ挙動、デフォルト、CLI 出力例をドキュメントに追記し、利用者がオプションの効果を理解できるようにする。
  - Purpose: ユーザーガイドと実装を同期させる。
  - _Leverage: docs/protocol/ping-pong.md_
  - _Requirements: 3.1,3.2,3.3_
  - _Prompt: Implement the task for spec peer-session-retry-loop, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Technical Writer | Task: Revise the ping-pong CLI guide to describe retry defaults, tuning guidance, and sample outputs that include retry attempts | Restrictions: Maintain existing document tone and structure, localize content in Japanese, avoid removing unrelated instructions | _Leverage: docs/protocol/ping-pong.md | _Requirements: 3.1,3.2,3.3 | Success: Documentation clearly explains retry semantics and matches the implemented behavior_

- [ ] 5. `tasks.md` ステータス管理と仕上げ
  - File: .spec-workflow/specs/peer-session-retry-loop/tasks.md
  - 各タスク開始時に `[-]`、完了時に `[x]` へ更新し、進捗を記録する。実装完了後に全タスクが `[x]` になっていることを確認する。
  - Purpose: スペックワークフローの進行管理。
  - _Leverage: .spec-workflow/specs/peer-session-retry-loop/tasks.md_
  - _Requirements: 1.1,1.2,1.3,2.1,2.2,2.3,3.1,3.2,3.3_
  - _Prompt: Implement the task for spec peer-session-retry-loop, first run spec-workflow-guide to get the workflow guide then implement the task: Role: Project Maintainer | Task: Keep tasks.md status markers accurate during implementation and ensure the document reflects completion at the end | Restrictions: Do not reorder tasks without discussion, update statuses immediately when work state changes | _Leverage: .spec-workflow/specs/peer-session-retry-loop/tasks.md | _Requirements: 1.1,1.2,1.3,2.1,2.2,2.3,3.1,3.2,3.3 | Success: tasks.md mirrors real progress with correct status indicators_
