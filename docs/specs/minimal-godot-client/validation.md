# 実行・検証手順（最小）

## 前提
- Godot 4.5.1 を使用する
- OpenXR を有効化し、SteamVR をインストール済みであること
- GDExtension のビルド成果物が `client/godot/bin/windows/client_core.dll` に配置されていること

## ビルド
```bash
cargo build -p client-godot-adapter --target x86_64-pc-windows-gnu
```

## 起動
1. `client/godot/project.godot` を Godot で開く
2. シーン `node.tscn` を実行する
3. 実行ログに以下が出力されることを確認する
   - `OK: GDExtension is loaded`
   - `OpenXR interface found`
   - `OpenXR initialize result: true`（SteamVR 起動時）
   - `Viewport use_xr enabled`

## 検証ポイント
- SteamVR 未起動の場合
  - `OpenXR initialize result: false` が出力される
- SteamVR 起動済みの場合
  - `OpenXR initialized: true` が出力される

## 最小描画の確認（手動）
- `OpenXR: No viewport was marked with use_xr` の警告が出ていないことを確認する。
- 画面中央にテスト用の立方体が表示されることを確認する。
- 本段階では `RenderFrame` は原点・単位姿勢の 1 体分を返すため、原点付近に表示される。
