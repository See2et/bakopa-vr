## Project Overview
このプロジェクトは、分散型Social-VR「SuteraVR」と呼称されるものです。従来のSocial-VRが特にインフラ／通信コストに苛まれていることに課題意識を持ち、それをFederationとP2Pによる二重分散により解決することを志向しています。

詳細は`docs/product.md`と`docs/architecture.md`を参照して下さい。

## Coding Rules
### Do Test-Driven Development & Spec-Driven Development
和田卓人（t-wada）氏が提唱するテスト駆動開発（TDD）と、仕様駆動開発（SDD）に則って開発を進めて下さい。

- **テストが開発を駆動する:** すべてのプロダクションコードは、失敗するテストをパスさせるためだけに書かれます。テストは後付けの作業ではありません。それ自身が仕様書であり、設計の駆動役です。
- **リファクタリングへの自信:** 包括的なテストスイートは我々のセーフティネットです。これにより、私たちは恐れることなく継続的にコードベースのリファクタリングと改善を行えます。
- **テスト容易性は良い設計に等しい:** コードがテストしにくい場合、それは悪い設計の兆候です。エージェントは、テスト容易性の高いコード作成を最優先しなければなりません。それは自然と、疎結合で凝集度の高いアーキテクチャにつながります。

Coding Agentは、いかに小さな変更であっても、必ずこの反復的なサイクルに従わなければなりません。コードを生成する際は、現在どのフェーズにいるのかを明示してください。

また、**仕様が不明瞭なときは勝手に「良さそうな実装」をしない**で下さい。適宜ユーザーに仕様書の更新提案や質疑応答を行って下さい。

1. Spec: 仕様を明文化する
    - ユーザーのやりたいことが1Spec=1PRとして過剰・過大であるかを判断。そうである場合、適切な要件と規模に落とし込む(skill: sdd-slice-wish)
    - ユーザーからヒアリングした内容を元に、Specのドラフトを作成する(skill: sdd-init)
    - ドラフトの内容に合意できたら、TDDのための充分なテストケースをSpecに網羅する(skill: sdd-test-cases)
1. Red: 失敗するテストを書く(skill: tdd-red)
1. Green: テストをパスさせる(skill: tdd-green)
1. Refactor: コードの品質を向上させる(skill: tdd-refactor)
1. Commit: 進捗を保存する

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

### Error Handling

このプロジェクトでは、エラー処理の基盤として **`anyhow`** と **`thiserror`** を使用する。  

#### 前提となる役割分担

- **`anyhow`**
  - `anyhow::Error` と `anyhow::Result<T>` による「型消去された汎用エラー型」。
  - **アプリケーションコード**での「簡易なエラー統合・伝搬・コンテキスト付与」に用いる。
- **`thiserror`**
  - `#[derive(Error)]` で `std::error::Error` 実装を自動生成するためのクレート。
  - **ライブラリ／ドメイン層**での「型付きエラー定義」に用いる。


- **ライブラリ／ドメイン層** → `thiserror` で意味のある Error 型を定義
- **アプリケーション境界（`main` など）** → 複数の Error を `anyhow` でまとめて扱う

#### アプリケーション層（binary crate）でのルール — anyhow

1. **戻り値は `anyhow::Result<T>` を使うのは「最上位だけ」**
   - `main` や CLI ハンドラ、HTTP サーバのエントリポイントなど、  
     「最終的にログを出して終了／レスポンスに変換する層」に限定して `anyhow::Result<()>` を使う。
   - ドメインロジックにまで `anyhow::Result` を広げない。

   ```rust
   use anyhow::Result;

   fn main() -> Result<()> {
       app::run()?;
       Ok(())
   }
   ```


2. **`.context()` / `.with_context()` でエラーに文脈を必ず付ける**

   * 「どの操作中に失敗したのか」がわかるメッセージを付ける。

   ```rust
   use anyhow::{Context, Result};

   fn load_config(path: &str) -> Result<String> {
       std::fs::read_to_string(path)
           .with_context(|| format!("failed to read config from {path}"))
   }
   ```

3. **「ハンドルできない／ハンドルしない」境界でのみ anyhow に集約する**

   * HTTP レイヤや CLI レイヤで「ログを出す」「ユーザー向けメッセージに変換する」直前で、
     下位の `thiserror` ベースのエラーを `anyhow::Error` に吸わせるのは OK。
   * それより下の層では **独自 Error 型のまま** 保つ。

4. **`unwrap` / `expect` の禁止（初期化コードなど例外的ケースを除く）**

   * ランタイムで発生しうる失敗はすべて `Result` / `Option` として扱い、`?` と `anyhow` / `thiserror` で処理する。

#### ライブラリ／ドメイン層でのルール — thiserror

1. **Public API では `anyhow` を返さず、自前の Error 型を定義する**

   * `pub fn ... -> Result<T, Error>` の `Error` は自前の enum / struct。
   * `anyhow::Error` を public API に出すのは禁止。

   ```rust
   use thiserror::Error;

   #[derive(Debug, Error)]
   pub enum RepositoryError {
       #[error("db error: {0}")]
       Db(#[from] sqlx::Error),

       #[error("entity not found: {id}")]
       NotFound { id: String },
   }

   pub type Result<T> = std::result::Result<T, RepositoryError>;
   ```

2. **`#[from]` で外部エラーをラップし、source を保持する**

   * 依存クレートのエラーや IO エラーは、`#[from]` を使って自動変換する。
   * これにより `?` 演算子で自然に伝搬できる。

3. **エラー型は「使う側の判断に必要な粒度」で設計する**

   * 「ユーザー入力ミス」「外部サービスの障害」「内部バグ」など、
     リトライ可否や HTTP ステータス変換などに必要な分類を enum variant として持たせる。

   ```rust
   #[derive(Debug, Error)]
   pub enum DomainError {
       #[error("invalid input: {0}")]
       InvalidInput(String),

       #[error("external service failed: {0}")]
       External(String),

       #[error("unexpected internal error")]
       Internal(#[from] anyhow::Error), // ← ドメイン内だけで包むのはアリ
   }
   ```

4. **Error 型はモジュール／境界ごとに分ける**

   * 1 つの巨大な `Error` enum に何でも詰め込まず、
     「RepositoryError」「DomainError」「ApiError」のように責務ごとに分割する。

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
