# AI-DLC and Spec-Driven Development

Kiro-style Spec Driven Development implementation on AI-DLC (AI Development Life Cycle)

## Project Memory
Project memory keeps persistent guidance (steering, specs notes, component docs) so Codex honors your standards each run. Treat it as the long-lived source of truth for patterns, conventions, and decisions.

- Use `docs/steering/` for project-wide policies: architecture principles, naming schemes, security constraints, tech stack decisions, api standards, etc.
- Use local `AGENTS.md` files for feature or library context (e.g. `src/lib/payments/AGENTS.md`): describe domain assumptions, API contracts, or testing conventions specific to that folder. Codex auto-loads these when working in the matching path.
- Specs notes stay with each spec (under `docs/specs/`) to guide specification-level workflows.

## Project Context

### Paths
- Steering: `docs/steering/`
- Specs: `docs/specs/`

### Steering vs Specification

**Steering** (`docs/steering/`) - Guide AI with project-wide rules and context
**Specs** (`docs/specs/`) - Formalize development process for individual features

### Active Specifications
- Check `docs/specs/` for active specifications
- Use `/prompts:kiro-spec-status [feature-name]` to check progress

## Development Guidelines
- Think in English, generate responses in Japanese. All Markdown content written to project files (e.g., requirements.md, design.md, tasks.md, research.md, validation reports) MUST be written in the target language configured for this specification (see spec.json.language).

## Minimal Workflow
- Phase 0 (optional): `/prompts:kiro-steering`, `/prompts:kiro-steering-custom`
- Phase 1 (Specification):
  - `/prompts:kiro-spec-init "description"`
  - `/prompts:kiro-spec-requirements {feature}`
  - `/prompts:kiro-validate-gap {feature}` (optional: for existing codebase)
  - `/prompts:kiro-spec-design {feature} [-y]`
  - `/prompts:kiro-validate-design {feature}` (optional: design review)
  - `/prompts:kiro-spec-tasks {feature} [-y]`
- Phase 2 (Implementation): `/prompts:kiro-spec-impl {feature} [tasks]`
  - `/prompts:kiro-validate-impl {feature}` (optional: after implementation)
- Progress check: `/prompts:kiro-spec-status {feature}` (use anytime)

## Development Rules
- 3-phase approval workflow: Requirements → Design → Tasks → Implementation
- Human review required each phase; use `-y` only for intentional fast-track
- Keep steering current and verify alignment with `/prompts:kiro-spec-status`
- Follow the user's instructions precisely, and within that scope act autonomously: gather the necessary context and complete the requested work end-to-end in this run, asking questions only when essential information is missing or the instructions are critically ambiguous.
- 実装や編集を行った場合は、完了前に必ず `cargo fmt --all` を実行し、フォーマット差分を解消すること。
- 実装や編集を行った場合は、完了前に必ず `cargo clippy --all-targets --all-features -- -D warnings` を実行し、警告・エラーを解消すること。

## Steering Configuration
- Load entire `docs/steering/` as project memory
- Default files: `product.md`, `tech.md`, `structure.md`
- Custom files are supported (managed via `/prompts:kiro-steering-custom`)

## Rust Coding Rules

### Logging/Trace

#### 使用クレートと前提

- ログ／トレースはすべて `tracing` 経由で出すこと。`println!` や標準 `log` マクロを新しく追加しない。
- バイナリクレート側でのみグローバル Subscriber を初期化する。ライブラリコードからは **絶対に Subscriber を初期化しない**。
- 既存コードが `log` を使っている場合は、`tracing-subscriber` の `LogTracer` 等でブリッジする（`log` 呼び出しを消さずに `tracing` に流す）。

#### ログ設計の基本方針

1. **構造化ログを前提とする**
   - メッセージを文字列連結せず、できるだけフィールドとして持たせる。
   - 例: `info!(order_id = order.id, user_id = user.id, "order created");`

2. **span を単位にフローを追えるようにする**
   - 「リクエスト単位」「ジョブ単位」「大きな処理単位」で `span` を張る。
   - HTTP ハンドラ、キューコンシューマ、長時間処理の入口には原則 `#[instrument]` を付与する。
   - 子処理は親 span の中で `info!` などを打ち、どのリクエスト由来かが辿れるようにする。

3. **レベルの使い分け**
   - `error!`… ユーザー影響のある失敗。アラート対象候補。
   - `warn!`… 明確なバグではないが怪しい状態／フェイルオーバー発生。
   - `info!`… ビジネス的に意味のあるイベント（作成・状態遷移など）。
   - `debug!`… デバッグに有用だが通常は不要な詳細。
   - `trace!`… ループ内部など高頻度・超詳細。原則、慎重に使用。

4. **本番向けフォーマット**
   - 本番環境では JSON ログ（機械可読）を前提とする。
   - ログフォーマットやフィルタレベルは環境変数（例: `RUST_LOG`, `LOG_FORMAT`）で切り替え可能にする。

5. **セキュリティ／プライバシー**
   - パスワード、秘密鍵、トークン、クレジットカード情報などの機密情報は絶対にログに出さない。
   - ユーザー識別には内部 ID を使い、生データ（メールアドレス等）は必要な場合のみフィールドとして記録する。

### Error Handling (skill: rust-error-handling)

このプロジェクトでは、エラー処理の基盤として **`anyhow`** と **`thiserror`**を用いる。

- **ドメイン／ライブラリ層**の public API は 責務ごとの **型付き Error（thiserror）** を定義して返すこと。
- **アプリ境界（main/CLI/HTTP入口など）**でのみ `anyhow::Result` を使用してよい。境界では必ず `.context()` / `.with_context()` で「何をしていて失敗したか」を付与すること。
- `unwrap` / `expect` は原則禁止（テスト、明示された初期化コードなど例外を除く）。ランタイムで起こりうる失敗は `Result/Option` として扱い `?` で伝搬する。
- エラーは「使う側が判断できる粒度」で設計する（例: InvalidInput / NotFound / Conflict / External / Internal）。曖昧な文字列エラーや握りつぶしは禁止。

### Use Strong Types, Not Primitive Obsession

❌ 全てを`String`,`u64`,`i32`などのプリミティブ型で表現
✅ 必要に応じて`UserID`,`Timeout`,`EmailAddress`などの意味を持った型やenumを定義

### examples

```diff
- fn send_email(to: String, body: String) { /* ... */ }
+ pub struct EmailAddress(String);
+ pub struct EmailBody(String);
+ 
+ fn send_email(to: EmailAddress, body: EmailBody) { /* ... */ }
```

### 単純な抽象 (Simple Abstractions)

- 公開APIでジェネリクス・ネストを深くしない。
  - 例: `Foo<T>` までは許容、`Foo<Bar<Baz<T>>>` のような型をパブリックに出さない。
- 特に「サービスレベル型」では、`Service<Backend<Store>>` のような多段ネスト型を公開しない。
  - 公開型は `Service` など単純な表現とし、内部で構成要素を隠蔽する。

### ラッパー型／スマートポインタ非露出

- 公開APIの引数・戻り値に `Arc<T>` / `Rc<T>` / `Box<T>` / `RefCell<T>` 等を直接出ささない。
- 原則として `T` / `&T` / `&mut T` を使わせる。
- 共有や所有戦略はライブラリ内部に閉じ込める。
