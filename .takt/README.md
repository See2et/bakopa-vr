# TAKT cc-sdd 実装自動化

このリポジトリでは、`cc-sdd` の実装フェーズ（`/prompts:kiro-spec-impl` 相当）を `takt` で自動実行するために、custom piece `cc-sdd-impl-codex` を用意しています。

## 前提

- Codex を利用するため、`TAKT_OPENAI_API_KEY`（または `OPENAI_API_KEY`）が必要
- 対象 spec は `docs/specs/<feature>/` 配下に存在すること
- `spec.json` で `approvals.tasks.approved=true` かつ `ready_for_implementation=true` であること

## ローカル実行

```bash
npx --yes takt@0.8.0 --pipeline \
  --piece cc-sdd-impl-codex \
  --task $'feature=<spec-dir-name>\ntasks=auto\nbatch_size=2'
```

PR まで自動化する場合:

```bash
npx --yes takt@0.8.0 --pipeline \
  --piece cc-sdd-impl-codex \
  --task $'feature=<spec-dir-name>\ntasks=auto\nbatch_size=2' \
  --auto-pr --repo OWNER/REPO
```

## task 文字列フォーマット

`--task` は以下の key-value 形式で渡します。

```text
feature=<docs/specs のディレクトリ名>
tasks=auto|1.1,1.2
batch_size=1..3
```

- `tasks=auto`: 未完了タスクを先頭から `batch_size` 件ずつ処理
- `tasks=1.1,1.2`: 指定タスクだけ処理
- `batch_size` 未指定または不正値は `2` を使用

## 実行中の挙動

`cc-sdd-impl-codex` piece は次をループ実行します。

1. 対象 spec と承認状態の確認
2. タスク実装（TDD）
3. 品質ゲート実行
   - `cargo fmt --all`
   - `cargo clippy --all-targets --all-features -- -D warnings`
   - `cargo test --workspace --all-targets`
4. 仕様整合の検証
5. 1バッチ分のコミット
6. 未完了タスクがあれば plan に戻って次バッチへ進行

## 補足

- `logs/`, `reports/`, `completed/` は `.takt/.gitignore` で除外済み
- 仕様書作成（requirements/design/tasks の生成）は対象外。人間が実施する前提
