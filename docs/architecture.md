## ディレクトリ構造
// TODO

## SuteraVRを構成する要素
### Syncer
**役割**
- P2Pを用いた音声や位置情報の同期
- BloomからのUGCの読み込み

**技術スタック**
- 言語: Rust
- ライブラリ: iroh

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
- アバターやワールドなどのUser-Generated Contentの管理
- Bloom単位でのモデレーション

**技術スタック**
- 言語: Rust
