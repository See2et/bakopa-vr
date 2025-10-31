# Syncer RNG 初期化修正 指示書（2025-10-31）

## コンテキスト
- `syncer/src/p2p/node.rs` で秘密鍵を生成する際、`iroh::SecretKey::generate(&mut OsRng)` を呼び出すと `OsRng: CryptoRng` の制約が満たせずコンパイルが失敗している。
- 依存している `rand` 0.9 系では `OsRng` が `TryCryptoRng` のみを実装し `CryptoRng` を満たさなくなった。一方 `SecretKey::generate` は `CryptoRng` を要求するため型不整合が発生する。
- `rand::rng()` が返す `ThreadRng` は `CryptoRng` を実装しているため、これを使うことで依然として暗号的安全性を保ったまま鍵生成が可能。

## 目的とゴール
- `load_or_generate_private_key`（および同様のユーティリティ）が `rand::rng()` を使って `SecretKey` を生成するように置き換え、`cargo check -p syncer` が通ること。
- OS 乱数を用いる分岐のままでも動作するようにし、秘密鍵ファイルの読み込みロジックは現状を維持する。

## スコープ
- `syncer/src/p2p/node.rs` 内の秘密鍵生成ロジックの修正。
- もし他ファイル（例: 将来のユニットテストや別バイナリ）が `OsRng` を直接渡している場合は同様に `rand::rng()` へ差し替える。
- 依存関係の更新や `rand` バージョン固定は今回の範囲外。

## 実装手順
1. `use rand::rngs::OsRng;` を削除し（不要になるため）、代わりに `use rand::prelude::*;` などは導入しない。`rand::rng()` はパスから直接呼び出せる。
2. `load_or_generate_private_key` の `else` ブロックを以下のように書き換える：
   ```rust
   let mut rng = rand::rng();
   iroh::SecretKey::generate(&mut rng)
   ```
   - これにより `ThreadRng`（ChaCha12 ベース）が使用され、`CryptoRng` 制約を満たす。
3. 将来 OS 乱数を直接使いたい場合は `rand::rngs::adapter::ReseedingRng` などで `TryCryptoRng` → `CryptoRng` へ昇格させる必要があることをコメントで補足しておくと良い。
4. 関数に付随する `dbg!(key);` などの一時デバッグ出力があれば、今回の作業中に除去する（本筋と無関係なノイズを減らすため）。

## 擬似コード
```
fn load_or_generate_private_key(path: &Option<PathBuf>) -> Result<SecretKey> {
    if let Some(path) = path {
        // 既存処理: ファイルから32バイトを読み込み SecretKey::from_bytes
    } else {
        let mut rng = rand::rng();
        Ok(SecretKey::generate(&mut rng))
    }
}
```

## テスト方針
- `#[cfg(test)]` で `load_or_generate_private_key` を呼び、`None` を渡したときに `SecretKey::to_bytes()` が 32 バイトを返しエラーにならないことを確認するユニットテストを追加する。
- 鍵ファイル読み込みの経路については、一時ファイルを生成し 32 バイトを書き込んだものを読み込むテストを別途用意できると尚良い。
- テスト名例：`generates_new_secret_key_when_path_absent` / `reads_secret_key_from_disk`。

## 完了条件
- `cargo check -p syncer` が成功する。
- 新規ユニットテストを `cargo test -p syncer load_or_generate_private_key` のように実行し、失敗しないこと。
- 変更箇所が `syncer/src/p2p/node.rs` など必要最小限に留まっていること。
