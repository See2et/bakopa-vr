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

`nix-shell` を使う場合の既定シェル（ネイティブ開発向け）:

```bash
nix-shell
```

Windows DLL のクロスビルド用シェルが必要な場合:

```bash
nix develop .#windows
```

`nix-shell` で Windows クロスビルド用シェルに入る場合:

```bash
nix-shell shell.windows.nix
```

### 3) Windows DLL をビルド

```bash
cargo build -p client-godot-adapter --target x86_64-pc-windows-gnu
```

スクリプト経由で Windows ビルドする場合:

```bash
scripts/build-client-core-windows.sh
```

Linux 向けライブラリをビルドして Godot 配置する場合:

```bash
scripts/build-client-core-linux.sh
```

macOS 向けライブラリをビルドして Godot 配置する場合:

```bash
# ホスト arch を自動判定（または --arch x86_64|arm64）
scripts/build-client-core-macos.sh
```

### 4) Godot 側の設定

`client/godot/client_core.gdextension` は実行時に以下の DLL を参照します。

```text
res://bin/windows/client_core.dll
# 実ファイル: client/godot/bin/windows/client_core.dll
```

`scripts/build-client-core-windows.sh` はビルド成果物を
`client/godot/bin/windows/client_core.dll` へコピーします。
