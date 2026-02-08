# GDExtension / godot-rust の境界メモ

- GDExtension は Godot 4 のネイティブ拡張インターフェース。
- godot-rust は GDExtension を使った Godot 4 向け Rust バインディング。
- Godot は I/O 層（rendering, input, scene graph）として扱い、Domain state は Rust ECS に保持する。
