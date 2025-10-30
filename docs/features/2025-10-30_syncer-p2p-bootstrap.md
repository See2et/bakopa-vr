# Syncer P2P Bootstrap 指示書（2025-10-30）

## コンテキスト
- SyncerはSuteraVRにおけるリアルタイム同期レイヤーであり、P2P通信の初期確立がボトルネックになっている。
- 現状の`syncer`クレートは`fn main()`のみのダミー構成で、P2Pセッション管理やライブラリアーキテクチャが未整備。
- まずは最小構成で2ノードが相互に接続し、短いテキストメッセージをラウンドトリップできるところまでを到達目標とする。

## 目的とゴール
- Rust製Syncerが`iroh`を利用してP2Pノードを起動し、明示的なピア同士で自己紹介メッセージを交換できる状態を整える。
- コマンドラインからシンプルなデバッグバイナリを起動し、接続ターゲット（多くはマルチアドレスまたはシードノード情報）を指定できるようにする。
- 将来的なワールド同期・音声同期ロジックを追加しやすいモジュール構成と初期テストを準備する。

## スコープ
- `syncer`クレートのみ。Rustワークスペースの他クレートやUnityクライアントは今回触れない。
- `Cargo.toml`での依存追加、`src/lib.rs`/`src/main.rs`/`src/bin/`のレイアウト変更を含む。
- P2P通信はLAN同一マシン上の2プロセスを想定。STUN/DERPなど高度なNATトラバーサルは対象外。

## アウトオブスコープ
- 恒常的なネットワーク障害対応、NAT超え、暗号鍵の永続化。
- 高頻度データ同期や音声ストリームなどの上位プロトコル実装。
- プロダクション向けの証明書管理・セキュリティ強化。

## 依存と前提
- `iroh`クレート（最新の0.20系を想定。実装前に`cargo search iroh`等で安定版を確認）。
- 非同期ランタイムに`tokio`（マルチスレッドランタイム）を利用。
- Rustエディションは`2024`で統一。MSRVは`rust-toolchain.toml`で固定予定（別タスク）。

## 想定ディレクトリ／ファイル構成
```
syncer/
├─ Cargo.toml
├─ src/
│  ├─ lib.rs              # ノード起動用APIを公開
│  ├─ bin/
│  │   └─ dev.rs          # デバッグ用バイナリ：CLI引数→P2P起動
│  ├─ p2p/
│  │   ├─ mod.rs          # モジュールエントリ
│  │   ├─ node.rs         # ノード生成とライフサイクル管理
│  │   └─ channel.rs      # 簡易メッセージパス（将来拡張の抽象）
│  └─ config.rs           # 起動設定（キー、シードアドレス）
└─ tests/
   └─ p2p_smoke.rs        # 2ノード間メッセージ疎通テスト（失敗する状態で提供）
```

## 段階的な実装ステップ
下記のステップを順番に進めると、初学者でも少しずつP2Pブートストラップを完成させられます。各ステップでGitコミットや動作確認を行い、疑問点があれば次へ進む前に整理して下さい。

1. **ステップ0: 事前確認と準備**
   - Rustツールチェーンがインストール済みかを`rustc --version`で確認。
   - `cargo check`が既存のワークスペースで通るか再確認し、既存エラーがあれば解消。

2. **ステップ1: クレート構造のリファクタリング（雛形）**
   - `syncer/src/lib.rs`を新規作成し、`pub struct SyncerHandle;`のような最小構造体を置く。
   - 既存の`syncer/src/main.rs`を`syncer/src/bin/dev.rs`へ移動し、`fn main()`は`todo!()`に置き換えて一旦ビルドを失敗させる。
   - `syncer/src/p2p/mod.rs`と`syncer/src/config.rs`ファイルを空でも良いので作成し、モジュールを宣言。

3. **ステップ2: Cargo依存追加の学習と反映**
   - `cargo search iroh`で利用可能なバージョンを把握し、`syncer/Cargo.toml`に`iroh = "<version>"`、`tokio = { version = "1", features = ["macros", "rt-multi-thread"] }`を追加。
   - `cargo check -p syncer`で依存解決が成功することを確認（ここではビルド失敗でもOK）。

4. **ステップ3: 設定モジュールの土台作成**
   - `config.rs`に`pub struct NodeConfig`を定義し、`listen_addr: SocketAddr`や`peers: Vec<Multiaddr>`など必要そうなフィールドを列挙（内容は`todo!()`可）。
   - `lib.rs`から`pub use config::NodeConfig;`で公開。

5. **ステップ4: P2Pノード管理の雛形**
   - `p2p/node.rs`を作成し、`pub struct SyncerNode`と`impl SyncerNode { pub async fn start(config: &NodeConfig) -> anyhow::Result<Self> { todo!() } }`の形で骨組みを置く。
   - `p2p/channel.rs`に`pub struct MessageChannel;`を定義し、`pub async fn send(&self, _msg: String) -> anyhow::Result<()> { todo!() }`といったスタブを追加。
   - `p2p/mod.rs`で`pub mod node; pub mod channel;`を宣言し、`pub use node::SyncerNode; pub use channel::MessageChannel;`を提供。

6. **ステップ5: start_syncer APIの骨格実装**
   - `lib.rs`で`pub async fn start_syncer(config: NodeConfig) -> anyhow::Result<SyncerHandle>`を宣言し、内部で`let node = SyncerNode::start(&config).await?;`という形のTODOを配置。
   - `SyncerHandle`に`node`と`channel`フィールドを追加し、まだ`todo!()`にしておく。

7. **ステップ6: デバッグ用バイナリのCLI整備**
   1. `syncer/src/bin/dev.rs`を新規作成し、まずは次の最小コードを書いてTokioの非同期エントリポイントだけ用意する。
      ```rust
      #[tokio::main]
      async fn main() -> anyhow::Result<()> {
          todo!("dev binary is not implemented yet");
      }
      ```
   2. 関数の冒頭に`let args: Vec<String> = std::env::args().collect();`を追加し、`dbg!(&args);`で実行時の引数を一度表示して挙動を確認する。
   3. 引数解析は次のステップに回し、仮のアドレス（例: "127.0.0.1:47401"）で`NodeConfig::new(...)`を呼び出す位置だけ決めておく。ここでは`todo!("wire start_syncer")`を置き、まだ実装しないことを明示する。
   4. 関数の最後は`Ok(())`で終わらせ、`// TODO: 引数を解析して start_syncer を呼び出す`とコメントを残す。こうすることで次のステップで詳細を追加しやすくする。

8. **ステップ7: P2P疎通の最小実装**
   - `SyncerNode::start`で`iroh::node::Node::builder()`を呼び、`bind_addr`と鍵設定を`todo!()`から具体実装へ差し替え。
   - `MessageChannel::new`などのコンストラクタを整えて、`start_syncer`から返すハンドルでメッセージ送信が呼べるようにする。
   - 最初は`node.connect(peer).await?`後に`"hello"`を送って`"ack"`を受け取るだけのロジックに限定。

9. **ステップ8: 自動テストの追加**
   - `syncer/tests/p2p_smoke.rs`に`#[tokio::test]`を作成し、2つのノードを並列起動して`unimplemented!();`で失敗させる。
   - 実装が進んだら`tokio::join!`でノードを立ち上げ、`timeout(Duration::from_secs(5), async { ... })`で`ping/pong`を検証するコードに育てる。

10. **ステップ9: 手動検証と後片付け**
    - ターミナルを2つ開き、`cargo run -p syncer --bin dev -- --listen 127.0.0.1:4401`とシード付き実行を試してログを確認。
    - 正常系が動作したら`TODO`や`unimplemented!()`を残さないよう整理し、READMEや`docs/architecture.md`へ学んだ点を追記して終了。

## 実装の考え方メモ
- `SyncerNode`はノード起動と接続維持を担当し、`MessageChannel`はメッセージ交換の窓口として段階的に機能追加していく。
- それぞれのステップで`todo!()`や`unimplemented!()`を意図的に残し、次のステップで解決する流れを意識する。
- `cargo fmt`や`cargo clippy`は各ステップの終わりで実行すると差分が追いやすい。

## 疑似コード（概略）
```rust
// syncer/src/lib.rs
pub async fn start_syncer(config: NodeConfig) -> Result<SyncerHandle> {
    let key = load_or_generate_key(&config).await?;
    let node = NodeBuilder::default()
        .secret_key(key)
        .bind_addr(config.listen_addr)
        .spawn()
        .await?;

    for peer in &config.peers {
        node.connect(peer.clone()).await?;
    }

    let channel = MessageChannel::new(node.clone());
    channel.spawn_echo_task();

    Ok(SyncerHandle { node, channel })
}

// syncer/src/bin/dev.rs
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Cli::parse();
    let config = NodeConfig::from(args);
    let handle = start_syncer(config).await?;

    if let Some(message) = args.message {
        handle.channel.send(message).await?;
    }

    tokio::signal::ctrl_c().await?;
    handle.shutdown().await;
    Ok(())
}
```

## テスト計画
- `syncer/tests/p2p_smoke.rs`
  - `#[tokio::test(flavor = "multi_thread")]`で2ノードを`start_syncer`経由で起動。
  - 一方のノードから`"ping"`送信、もう一方で受信後`"pong"`返信するモックロジックを利用。
  - タイムアウト（例: 5秒）を設け、未接続の場合は`Err`で失敗させる。
  - 初回は未実装のため`unimplemented!()`を呼ぶ形でテストが失敗することを確認。

## 検証方法
- `cargo test -p syncer p2p_smoke`で失敗するテストが追加されていること。
- `cargo run -p syncer --bin dev -- --listen 127.0.0.1:4401`でノード起動。
- 別プロセスからシード指定して起動し、標準出力でメッセージ交換ログが見えること。

## リスクとフォローアップ
- `iroh`のAPI変更が頻繁なため、バージョン固定とAPI確認が必要。
- P2P接続はNATやファイアウォール設定に依存。ローカルテストは成功しても本番環境では追加設定が必要。
- テストで実際のネットワークポートを使用するため、CI環境ではポート衝突や権限不足に注意。

## 次のアクション候補
1. `syncer/Cargo.toml`に`iroh`・`tokio`依存を追加し、ビルド設定を整える。
2. `syncer/src`配下にP2Pモジュールと設定モジュールを新設し、`start_syncer`の骨格を実装する。
3. `p2p_smoke`テストを失敗状態で作成し、TDDでメッセージ疎通を完成させる。
