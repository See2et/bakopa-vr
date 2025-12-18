## ディレクトリ構造

// TODO

## SuteraVRを構成する要素

### Syncer

**役割**

- P2Pを用いた音声や位置情報の同期
- BloomからのUser-Generated Contentsの読み込み

**技術スタック**

- 言語: Rust
- ライブラリ: WebRTC

### Sidecar

**役割**

- SyncerとClientを橋渡しする

**技術スタック**

- 言語: Rust
- WebSocket

### Client

**役割**

- ユーザーが仮想空間に入るためのクライアント
- VR、あるいはデスクトップモードで操作する

**技術スタック**

- Unity/C#

### Bloom

**役割**

- 従来の分散型SNSで言うところの「インスタンス」
- アバターやワールドなどのUser-Generated Contentsの管理
- Bloom単位でのモデレーション

**技術スタック**

- 言語: Rust

## 短期的な目標

まずはSuteraVRの理念をMinimum Value Productとして提示するために、以下の内容を短期的な実装目標として掲げます。

**目標**

- Bloomはルームの管理とP2Pシグナリングのみを行う
- Client同士をP2Pで接続し、3点（頭と両手）とテキストチャットをリアルタイムに同期
- 固定のワールド／アバターを用いたPCVRのデモを用意

**非目標（MVPでやらないこと）**

- BloomによるUser-Generated Contents（ワールド／アバターなど）の管理
- 大規模同時接続、スケールアウト設計
- アバターやワールドのアップロード／変更

### 実装の優先度

1. Bloomによるシグナリング機能
2. SyncerによるP2P接続と同期機能
3. ClientがBloomとSyncerを通して3D空間の同期を行えるように
