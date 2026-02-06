# GDExtension API JSON

`client/godot/api/extension_api.4.5.1.full.json` は、`api-custom-json` モードで
`GODOT4_GDEXTENSION_JSON` に指定するためのベース JSON です。

## 運用方針

- 通常開発では `client-godot-adapter` は `godot` の `api-4-5` を使用（`codegen-full` は未使用）。
- `api-custom-json` を試す場合は `client/godot-adapter/Cargo.toml` の `godot` feature を
  `api-custom-json` に切り替え、必要最小限の API を含む JSON をこのフォルダで管理します。
- JSON の絞り込み範囲は、コンパイルエラーに応じて段階的に拡張します。
