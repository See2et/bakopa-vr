# Godot Input と Rotation の落とし穴（簡易メモ）

## InputEventMouseMotion

- イベント単位の mouse delta（relative, screen_relative）や velocity を提供する。
- ECS に渡す前に `MouseDelta { dx, dy }` のような Domain input struct に変換する。

## Input の蓄積

- Godot はフレーム内の input event を蓄積できる（Input.use_accumulated_input）。
- 再現性やテストを重視するなら、フレームごとに input をスナップショット化して ECS に渡す。

## Rotation（Node3D）

- Node3D の `rotation` / `rotation_degrees` は Euler で、順序は YXZ。
- Godot docs では Euler が完全な 3D 姿勢表現に不向きだと注意している。
- yaw/pitch 分離や quaternion（Basis.get_rotation_quaternion）を優先する。
- YXZ では `rotation` / `rotation_degrees` の pitch が ±90° 近傍で yaw と roll が干渉しやすく、実運用では「上/下を向いた状態での旋回」が gimbal lock の高リスクになるため、Node3D でも yaw/pitch を分離管理する。
- 実装方針として、内部の回転演算・補間は常に `Basis.get_rotation_quaternion` ベースで行い、`rotation` / `rotation_degrees` への変換は UI 表示やデバッグ出力に限定する（Euler 表現は同一姿勢に複数表現があり曖昧になりうる）。
