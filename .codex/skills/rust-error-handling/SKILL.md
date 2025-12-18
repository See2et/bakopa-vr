---
name: rust-error-handling
description: Rustでのエラー設計を、境界ごとに thiserror / anyhow を使い分けて実装する。ドメイン/ライブラリは型付きエラー(thiserror)、アプリ境界のみ anyow。context付与、unwrap禁止、HTTP/CLI変換の指針を含む。
---

# Rust Error Handling: anyhow / thiserror の境界設計

## 目的
- 例外的な失敗を「握りつぶさず」「原因を辿れる形」で伝搬し、境界で適切に変換する。
- ドメイン層のAPIを型付きエラーで安定させ、上位で集約・ログ化・ユーザー向け変換ができるようにする。

## 適用範囲
- **ライブラリ／ドメイン層**: `thiserror` による型付きエラー（`Result<T, Error>`）
- **アプリケーション境界（main/CLI/HTTPハンドラ等）**: `anyhow::Result` と `.context()` / `.with_context()`

## やらないこと
- ドメイン層の public API に `anyhow::Error` を露出しない。
- 「とりあえず `String` エラー」で返さない（判断不能になる）。

---

## 実装ワークフロー（判断→定義→伝搬→境界変換）

### 1) まず「境界」を確定する
- どこが **ドメイン／ライブラリ** で、どこが **アプリ境界** かを決める。
- 境界でだけ「ログ出力」「HTTP/CLIレスポンスへの変換」「anyhow集約」を行う。

### 2) ドメイン／ライブラリ: Error 型を設計する（thiserror）
設計の基準:
- 使う側が判断に使う粒度で variant を切る（例: NotFound / InvalidInput / Conflict / External / Internal）。
- 下位エラーは `#[from]` でラップして `source` を保持する。

推奨テンプレ:
```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("invalid input: {reason}")]
    InvalidInput { reason: String },

    #[error("entity not found: {id}")]
    NotFound { id: String },

    #[error("conflict: {reason}")]
    Conflict { reason: String },

    #[error("external dependency failed: {0}")]
    External(#[from] ExternalError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, DomainError>;
````

内部で「想定外」を扱う必要がある場合:

* ドメイン内に閉じる形で `Internal(anyhow::Error)` を持つのは可（ただし **public APIの戻り値は DomainError のまま**）。

```rust
#[derive(Debug, Error)]
pub enum DomainError {
    // ...
    #[error("unexpected internal error")]
    Internal(#[source] anyhow::Error),
}
```

### 3) 伝搬: 下位の失敗には文脈を付与する

* ドメイン層は `thiserror` の variant で意味を表現する。
* アプリ境界は `.context()` / `.with_context()` で「何をしていて失敗したか」を必ず付ける。

アプリ境界のテンプレ:

```rust
use anyhow::{Context, Result};

pub fn run() -> Result<()> {
    let cfg = load_config().context("failed to load config")?;
    app(cfg).context("application failed")?;
    Ok(())
}
```

### 4) アプリ境界: 変換（HTTP/CLI）を行う

* ドメインエラーを HTTP ステータスや CLI の終了コードに変換する。
* 変換は **match 一発で明示的に**。曖昧な文字列判定はしない。

HTTP変換の例（擬似）:

```rust
fn to_http(err: DomainError) -> (u16, String) {
    match err {
        DomainError::InvalidInput { reason } => (400, reason),
        DomainError::NotFound { .. } => (404, "not found".into()),
        DomainError::Conflict { reason } => (409, reason),
        DomainError::External(_) => (502, "bad gateway".into()),
        DomainError::Io(_) => (500, "internal error".into()),
        DomainError::Internal(_) => (500, "internal error".into()),
    }
}
```

### 5) 禁止事項

* `unwrap` / `expect` を **通常の実装で使わない**（初期化やテスト以外）。
* ドメイン／ライブラリの public API が `anyhow::Result` を返さない。
* エラーを握りつぶして `Ok(())` にしない（再試行・診断が不能になる）。
* 失敗を「ログだけ出して継続」する場合は、必ず呼び出し側が合意したリカバリ方針を明文化する。

---

## チェックリスト

* [ ] ドメイン層の public API は `Result<T, DomainError>`（または責務別Error）になっている
* [ ] `#[from]` による source 保持ができている（原因追跡できる）
* [ ] アプリ境界で `.context()` / `.with_context()` が付与されている
* [ ] `unwrap/expect` が残っていない（例外: テスト、明示された初期化のみ）
* [ ] HTTP/CLI変換が match で明示され、判断基準が読み取れる
