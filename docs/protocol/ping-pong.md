# ピア Ping-Pong ワークフロー

このドキュメントは、新しい CLI と sidecar crate を使って BakopaVR のピア ping-pong プロトタイプを動作確認する方法をまとめたものです。Bloom や Unity を使わずに、エンドツーエンドの疎通を検証したい開発者向けです。

## 前提条件

- Rust ツールチェーン（1.81 以上）
- 同一マシンで 2 つのシェルを開くか、相互に通信可能な 2 台のマシン

## ビルド

```
cargo build --workspace
```

## リスナーの起動

```
cargo run --bin peer-cli -- listen \
  --addr 127.0.0.1:5000 \
  --max-retries 3 \
  --retry-backoff-ms 500
```

`--addr` には到達可能なアドレスを指定してください。同一マシン内テストなら `127.0.0.1:5000`、他マシンから接続するならそのマシンのローカル IP を利用します。

実行すると広告用の Multiaddr と peer-id が表示されます。例:

```
listening on /ip4/127.0.0.1/udp/5000/quic-v1/p2p/Hu3yYqsmtYf9RTcjtC2z5ucbdivu7kZUR4kbv5b47k3D
peer id: Hu3yYqsmtYf9RTcjtC2z5ucbdivu7kZUR4kbv5b47k3D
```

このプロセスは起動したままにしておきます。sidecar layer が `UdpSocket` で待受し、受信した `ping` に自動で `pong` を返し、イベントを標準出力に表示します。

## リスナーへの接続

別シェル（または別マシン）で次を実行します。

```
cargo run --bin peer-cli -- dial \
  --peer /ip4/127.0.0.1/udp/5000/quic-v1/p2p/Hu3yYqsmtYf9RTcjtC2z5ucbdivu7kZUR4kbv5b47k3D \
  --addr 0.0.0.0:0 \
  --receive-timeout-ms 2000
```

ダイヤラーは `UdpSocket` をローカルバインドし `ping` を送信、`pong` を待機し、RTT を JSON 形式で出力します。

```
ping sent sequence=1 sent_at=2025-10-03 21:24:16.293547188 UTC
pong sequence=1 sent_at=2025-10-03 21:24:16.293785876 UTC received_ping_at=2025-10-03 21:24:16.293547188 UTC
{
  "sequence": 1,
  "rtt_ms": 0.238,
  "attempts": 1,
  "peer": "Hu3yYqsmtYf9RTcjtC2z5ucbdivu7kZUR4kbv5b47k3D"
}
```

接続できない場合、`peer-cli` は非ゼロ終了コードで終了し、sidecar レイヤからのエラーメッセージが表示されます。

## 備考

- リスナーは `Ctrl+C` を受け取ると安全に終了します。
- `rust/crates/peer-cli/tests/` 以下の統合テストには `#[ignore]` を付与し、CI でバイナリを起動しないようにしています。
- sidecar crate はリスナー状態をプロセス内レジストリで保持するため、セッションが Drop されるとクリーンアップが行われます。
