# bakopa-vr

## ドキュメント

- プロダクト概要（canonical）: `docs/steering/product.md`
- アーキテクチャ方針（canonical）: `docs/steering/tech.md`
- 構造・責務分割（canonical）: `docs/steering/structure.md`

## 開発環境 (Nix)

WSL から Windows 向け DLL をクロスビルドするための dev shell を用意しています。

### 1) 事前準備

`nix` が使えることを確認してください。

### 2) 開発シェルに入る

```bash
nix develop
```

Windows DLL のクロスビルド用シェルが必要な場合:

```bash
nix develop .#windows
```

### 3) Windows DLL をビルド

```bash
cargo build -p client-godot-adapter --target x86_64-pc-windows-gnu
```

スクリプト経由で Windows ビルドする場合:

```bash
scripts/build-client-core-windows.sh
```

### 4) Godot 側の設定

`client/godot/client_core.gdextension` は以下の DLL を参照します。

```text
client/godot-adapter/target/x86_64-pc-windows-gnu/debug/client_core.dll
client/godot-adapter/target/x86_64-pc-windows-gnu/release/client_core.dll
```
