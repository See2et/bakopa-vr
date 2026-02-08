# 検証手順（2 participant pose sync）

## 目的

- 本スライス内で準備可能な経路のみを使い、2 participant 同期成立を定常的に再現する。
- 失敗時に `入力 / 送信 / 受信 / 投影` のどの段階かをテスト結果から特定できるようにする。
- 音声同期と Bloom 本番シグナリング配線は検証対象に含めない。

## 前提

- 実行ディレクトリ: ワークスペースルート
- Rust toolchain と `cargo` が利用可能
- 対象スライス: `#11-2-participant-pose-sync`

## 6.3 再現検証フロー（実行可能）

次のコマンドを実行する。

```bash
bash scripts/verify-client-pose-sync.sh
```

このスクリプトは以下を順に検証する。

1. `stage=input`: desktop/VR 入力正規化の成立
2. `stage=send`: room 未準備から準備完了への送信再開
3. `stage=receive`: join/rejoin/stale drop を含む 2 participant 同期フロー
4. `stage=projection`: remote pose 更新/削除の描画反映

全ステップが成功した場合、最後に `verification complete` を表示する。

## 失敗時の切り分け

`scripts/verify-client-pose-sync.sh` は段階ごとに開始ログを出す。失敗時は最後に表示された段階ログを起点に切り分ける。

| 段階 | 主な確認対象 | 対応テスト |
|------|--------------|------------|
| `stage=input` | 入力正規化、`dt` 補正、mode 差異吸収 | `desktop_input_normalization_maps_wasd_and_mouse_to_common_semantics` / `vr_input_normalization_maps_controller_input_to_common_semantics` |
| `stage=send` | 未参加時スキップ、ready 後の送信再開 | `pose_sync_coordinator_recovers_send_after_room_ready_transition` |
| `stage=receive` | join/rejoin、stale drop、2 participant 整合 | `two_participant_sync_flow_handles_join_rejoin_and_stale_pose` |
| `stage=projection` | remote 更新/削除の描画追従 | `remote_pose_projection_follows_frame_updates_and_removals` |

## 6.4 MVP 後スモーク回帰（定義）

MVP 完了後の補強として、次を追加回帰とする。

- 連続実行で揺らぎを検知するため、同じ検証セットを複数ラウンド実行する。
- desktop/VR の入力正規化・2 participant 同期・投影反映を同一ループで監視する。
- 受入条件の充足判定には使わず、回帰検知専用の補助ゲートとして扱う。

実行コマンド:

```bash
bash scripts/smoke-pose-sync-regression.sh
```

ラウンド数を変更する場合:

```bash
ITERATIONS=5 bash scripts/smoke-pose-sync-regression.sh
```

完走時は `smoke regression complete` を表示する。

## Godot シーン配線と InputMap の確認（8.4 / 8.5）

1. `client/godot/node.tscn` を起動し、`SuteraClientBridge` ノードが存在することを確認する。
2. 実行開始後、`verify_gdextension.gd` が `on_start` と毎フレーム `on_frame` を呼び出すことを確認する。
3. Desktop 操作で `W/A/S/D` とマウス移動を入力し、`push_input_event` 経由でイベントが Rust 側に到達することを確認する。
4. `client/godot/project.godot` の `[input]` に `move_*` / `look_*` が定義されていることを確認する。
