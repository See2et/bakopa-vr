---
name: godot-bevy-ecs-boundary
description: Godot Engine (GDExtension/godot-rust) と bevy_ecs の Domain を厳密に分離するための skill。Godot 型（Node/Node3D/Camera3D/InputEvent/SceneTree/Multiplayer/WebRTC）に触れる設計やコード生成、ECS の Components/Resources/Systems を定義する場面、またはテスト容易性・ネットワーク同期・サーバ分離が目的の時に使用する。Ports & Adapters (Hexagonal) の設計、input-state-output の分離、Godot 非依存の Domain ロジックをガイドする。
---

# Godot + bevy_ecs の境界規律

## 中核意図

- Domain ECS は純 Rust を維持する: Godot 型、Gd<T>、NodePath、Variant、SceneTree は持ち込まない。
- Godot API の使用は Adapter コードに限定し、Domain はプレーンなデータ型だけで通信する。

## Workflow（デフォルト）

1. Domain 概念をプレーンな Components/Resources（state）として定義する。
2. input ports（input snapshot 型）と output ports（command/result 型）を定義する。
3. Adapter 層で Godot の input/event を読み取り、Domain input に変換する。
4. Domain systems で state を更新し output を生成する。Godot 呼び出しや副作用は行わない。
5. Adapter 層が main thread で output を Godot ノードへ適用する。
6. Domain systems と純粋関数に対して unit tests を追加する。

## 必須の境界

- Domain
  - Components/Resources はプレーンな struct/enum とする。
  - Systems は純粋寄りにする: (state, input) -> state/output。
  - Godot API 呼び出しや Godot 型は一切使わない。
- Adapter
  - Godot 参照と Gd<T> ハンドルを所有する。
  - Godot の event を Domain input に、Domain output を Godot effect にマッピングする。
  - Godot の main-thread 制約を尊重する。並列 Godot 呼び出しは提案しない。

## Design heuristics

- trait で Godot API を写像しない。意図をモデル化する（例: ViewState, MouseDelta, Pose, SyncState）。
- InputEvent* を ECS に渡さない。Domain input struct に変換する。
- yaw/pitch あるいは quaternion を基準にし、Euler の順序依存を避ける。
- 将来の server-side / headless でも使えるよう ports を安定化させる。

## Refactor recipe（直接 Godot 依存がある場合）

1. Godot ノードから Domain state（components/resources）を抽出する。
2. input struct を導入し、Godot events をそれに変換する。
3. output commands を導入し、Adapter のみで適用する。
4. ロジックを Domain systems に移し、Adapter はマッピング専用にする。

## Tests

- Domain systems と純粋関数に対して cargo の unit tests を書く。
- input -> state 遷移は table-driven tests を使う。
- Godot ランタイムの tests は最小限にして分離する。

## references を読むタイミング

- input handling、event 蓄積、rotation の罠: `references/godot-rotation-input.md`
- GDExtension / godot-rust の境界メモ: `references/godot-rust-gdextension.md`
