## Project Overview

このプロジェクトは、分散型Social-VR「SuteraVR」と呼称されるものです。従来のSocial-VRが特にインフラ／通信コストに苛まれていることに課題意識を持ち、それをFederationとP2Pによる二重分散により解決することを志向しています。

詳細は`docs/product.md`と`docs/architecture.md`を参照して下さい。

## Do Test-Driven Development & Spec-Driven Development

和田卓人（t-wada）氏が提唱するテスト駆動開発（TDD）と、仕様駆動開発（SDD）に則って開発を進めて下さい。

- **テストが開発を駆動する:** すべてのプロダクションコードは、失敗するテストをパスさせるためだけに書かれます。テストは後付けの作業ではありません。それ自身が仕様書であり、設計の駆動役です。
- **リファクタリングへの自信:** 包括的なテストスイートは我々のセーフティネットです。これにより、私たちは恐れることなく継続的にコードベースのリファクタリングと改善を行えます。
- **テスト容易性は良い設計に等しい:** コードがテストしにくい場合、それは悪い設計の兆候です。エージェントは、テスト容易性の高いコード作成を最優先しなければなりません。それは自然と、疎結合で凝集度の高いアーキテクチャにつながります。

Coding Agentは、いかに小さな変更であっても、必ずこの反復的なサイクルに従わなければなりません。コードを生成する際は、現在どのフェーズにいるのかを明示してください。

また、**仕様が不明瞭なときは勝手に「良さそうな実装」をしない**で下さい。適宜ユーザーに仕様書の更新提案や質疑応答を行って下さい。
加えて、**YAGNI原則を強く意識し**、仕様書に記載されている内容以上のことに勝手に取り組もうとしないて下さい。

1. Spec: 仕様を明文化する
    - ユーザーのやりたいことが1Spec=1PRとして過剰・過大であるかを判断。そうである場合、適切な要件と規模に落とし込む(skill: sdd-slice-wish)
    - ユーザーからヒアリングした内容を元に、Specのドラフトを作成する(skill: sdd-init)
    - ドラフトの内容に合意できたら、TDDのための充分なテストケースをSpecに網羅する(skill: sdd-test-cases)
1. Red: 失敗するテストを書く(skill: tdd-red)
1. Green: テストをパスさせる(skill: tdd-green)
1. Refactor: コードの品質を向上させる(skill: tdd-refactor)
1. Commit: 進捗を保存する

### フェーズ開始時のSKILL確認（遵守強化）
- RED/GREEN/REFACTOR などフェーズを宣言するときは、対応する `SKILL.md`（例: `tdd-red` / `tdd-green` / `tdd-refactor`）を **毎回開いて確認** すること。  
  - 忘れた場合は作業を一時停止し、SKILLを読み直してから再開する。  

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
