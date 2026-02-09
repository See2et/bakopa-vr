# 実装計画

- [x] 1. 起動モードと入力正規化の基盤を整備する
- [x] 1.1 起動時に VR/desktop モードを確定し、XR 初期化失敗時のフォールバックを確立する
  - 起動パラメータまたは設定から実行モードを一意に決定し、セッション中は変更不可にする。
  - VR 初期化に失敗した場合は desktop へ安全に移行し、診断可能な失敗理由を保持する。
  - モード決定結果を入力変換と同期処理の両方が参照できる共通状態へ接続する。
  - _Requirements: 4.1, 4.3_

- [x] 1.2 desktop 入力を共通の InputSnapshot 意味論へ正規化する
  - WASD とマウス入力を `move` / `turn_yaw` / `look_pitch` / `dt` に変換し、単位と範囲を統一する。
  - 入力がないフレームでは移動意図をゼロにし、不要な位置変化を発生させない。
  - 変換結果を後続の移動更新と同期送信でそのまま利用できる形に揃える。
  - _Requirements: 3.2, 4.2_

- [x] 1.3 VR コントローラー入力を共通の InputSnapshot 意味論へ正規化する
  - VR コントローラー入力を移動・回転の意図値に変換し、desktop と同じ契約で扱えるようにする。
  - 入力取得に失敗した場合は空入力で継続し、異常終了せずに失敗理由を記録する。
  - 更新されたローカル pose が同フレーム系列の同期送信へ渡る前提を満たす入力更新を行う。
  - _Requirements: 3.1, 3.4_

- [x] 1.4 入力契約の共通バリデーションを整備する
  - `move` の正規化、`rad/s` 契約、`dt` の安全側補正をモード非依存で適用する。
  - 異常値は処理を停止せず補正して継続し、後続で原因追跡できる情報を残す。
  - 入力契約の不変条件を満たすことで移動系と同期系の前提差異をなくす。
  - _Requirements: 3.1, 3.2, 4.2_

- [x] 2. ローカル移動と送信対象化の Domain 更新を実装する
- [x] 2.1 MovementSystem で意図値と `dt` に基づくフレームレート非依存の移動更新を実装する
  - `move` / `turn_yaw` / `look_pitch` と `dt` からフレーム差分を計算し、FPS 依存の更新を排除する。
  - 入力なしフレームで移動差分が発生しないことを保証する。
  - VR/desktop 双方で同じ移動意味論を適用できるよう更新経路を統一する。
  - _Requirements: 3.1, 3.2_

- [x] 2.2 PoseSyncCoordinator でローカル更新を同期送信対象へ接続する
  - ローカル pose 更新後に同フレーム系列で送信対象として扱う順序を保証する。
  - runtime mode に依存せず同一の同期経路で送信要求を発行する。
  - 描画用フレーム生成に local pose を必ず含め、後続の投影処理へ渡す。
  - _Requirements: 3.3, 4.4_

- [x] 2.3 ローカル更新系の不変条件を固める
  - フレーム更新中の入力適用順を固定し、再現性のある更新結果を維持する。
  - 入力未発生時に入力起因の位置変化が起きないことをガードする。
  - 失敗時でも更新ループを継続できる安全側の分岐を確保する。
  - _Requirements: 3.2, 3.4_

- [x] 3. remote pose 状態管理と peer ライフサイクル追従を実装する
- [x] 3.1 participant 単位で remote pose の最新版管理を実装する
  - participant ごとに最新 pose と適用済み version を保持し、更新正本を一意化する。
  - 受信 version を比較し、古い pose は stale として破棄する。
  - 最新判定と保持戦略を描画フレーム生成で再利用できるよう整える。
  - _Requirements: 1.2_

- [x] 3.2 PeerJoined/PeerLeft/再参加/非活性の状態遷移を実装する
  - `PeerJoined` で同期状態を初期化し、必要な受信準備を即時有効化する。
  - `PeerLeft` で remote pose と描画対象を破棄し、重複イベントは冪等に扱う。
  - 同一 peer の再参加時は旧状態を再利用せず、新規セッションとして初期化する。
  - _Requirements: 2.1, 2.2, 2.3_

- [x] 3.3 remote 更新を次フレーム描画へ反映する
  - 最新 remote pose をフレーム境界で取り込み、次フレームの描画入力へ反映する。
  - 削除済み peer が描画に残らないよう、状態と描画対象の整合を維持する。
  - 複数 remote の更新順が変動しても最新版反映の一貫性を保つ。
  - _Requirements: 1.3, 2.2_

- [x] 4. SyncSessionAdapter と同期イベント連携を統合する
- [x] 4.1 room 参加状態を考慮した pose 送信経路を実装する
  - room 未参加または peer 未確立時は送信処理を安全にスキップする。
  - 送信可能な状態ではローカル pose を継続送信し、制御イベントとデータ送信の責務を分離する。
  - 送信スキップ時は原因追跡可能な診断情報を残す。
  - _Requirements: 1.1, 1.4, 6.3_

- [x] 4.2 バックグラウンド同期ワーカーと main 受け渡しを実装する
  - 同期処理を専用ワーカーで駆動し、main フレーム更新から非同期詳細を隠蔽する。
  - イベントを queue で受け渡し、main 側がフレームごとに取り込める形にする。
  - 終了時は新規受理停止と drain 方針を守って安全に停止する。
  - _Requirements: 1.1, 6.3_

- [x] 4.3 lifecycle 優先順で受信イベントを適用する
  - 同一フレームで join/leave を pose 反映より先に適用する順序規約を実装する。
  - 受信 pose に version 情報を付与し、domain 側の stale 判定へ接続する。
  - lifecycle 欠落や再参加シナリオでも状態破綻しない適用手順を維持する。
  - _Requirements: 1.2, 2.1, 2.3_

- [x] 5. 投影・観測性・スコープ境界を強化する
- [x] 5.1 (P) canonical pose を描画空間へ投影して main スレッド反映を確立する
  - canonical world の pose を描画側座標へ一元変換し、反映経路を統一する。
  - remote 更新と削除イベントが描画状態へ即時追従するよう整合を保つ。
  - 同期状態の更新と描画反映の責務境界を維持する。
  - _Requirements: 1.3, 2.2_

- [x] 5.2 (P) 同期段階の構造化トレースを実装する
  - `join/send/receive/leave` と失敗段階を stage として一貫記録する。
  - すべての主要ログに `room_id` / `participant_id` / `stream_kind` / `mode` を付与する。
  - 入力・送信・受信・投影のどの段階で失敗したか判別可能な情報を出力する。
  - _Requirements: 5.1, 5.2, 5.4_

- [x] 5.3 (P) スライス境界を実装で固定し、対象外機能の混入を防ぐ
  - 本スライスの受入に音声同期を含めない実行境界を明確化する。
  - Bloom 本番シグナリング配線を前提にしない接続経路で検証可能にする。
  - 実装と検証で境界逸脱が起きないようガード条件を整備する。
  - _Requirements: 6.1, 6.2_

- [x] 6. 統合検証と回帰テストを整備する
- [x] 6.1 Domain 振る舞いを単体テストで検証する
  - 入力正規化、移動更新、未入力時不変、remote 状態遷移を単体で検証する。
  - stale 破棄・再参加初期化・削除整合など状態管理の境界条件を検証する。
  - 失敗時継続方針が domain 更新ループを壊さないことを確認する。
  - _Requirements: 1.2, 2.2, 2.3, 3.1, 3.2, 3.4, 4.2_

- [x] 6.2 (P) 2 participant 同期の統合テストを整備する
  - pose 送受信が継続成立し、最新版のみが反映されることを検証する。
  - join/leave の制御イベントが順序保証どおり適用されることを確認する。
  - room 未参加時の送信スキップと復帰後の送信再開を確認する。
  - _Requirements: 1.1, 1.2, 2.1, 2.3_

- [x] 6.3 (P) 2 participant 再現検証フローを実行可能な形で整備する
  - 本スライス内で準備可能な接続経路を使い、同期成立の再現手順を定常的に実行できるようにする。
  - 同期失敗時に入力・送信・受信・投影のどこで失敗したかをテスト結果とログで特定可能にする。
  - mode 差異（VR/desktop）をまたいでも同一経路で検証できることを確認する。
  - _Requirements: 5.3, 5.4, 6.3_

- [x] 6.4 (P) MVP 後に実施する追加スモーク回帰を定義する
  - 6.1〜6.3 完了後の補強として、desktop/VR それぞれの連続操作時の同期安定性を検証する。
  - 既存受入条件を満たした実装に対する回帰検知を目的とし、実装本体の完了条件には含めない。
  - 追加検証結果を通常の同期経路の品質監視に接続する。
  - _Requirements: 3.1, 4.2, 4.4_

- [x] 7. 検証で検出された設計乖離と完了状態を是正する
- [x] 7.1 SyncSessionAdapter の実装を本番経路へ統合する
  - `syncer` 依存を client 側へ配線し、`SyncSessionPort` 実装として
    `SyncSessionAdapter` を導入する。
  - バックグラウンドワーカー（Tokio）と main 側 `poll_events()` 連携
    （MPSC queue）を実装し、join/leave 優先適用を維持する。
  - 既存の 2 participant フローが実アダプタ経路でも成立することをテストで確認する。
  - _Requirements: 1.1, 1.2, 1.4, 2.1, 2.3, 6.3_

- [x] 7.2 タスク完了チェックを実装実態に整合させる
  - 1〜6 の親タスクについて、完了基準とサブタスク完了状況を再確認する。
  - 完了している親タスクは `[x]` へ更新し、未完了がある場合は不足項目を明記する。
  - 検証レポートと `tasks.md` の状態が乖離しないことを確認する。
  - _Requirements: 5.3, 6.3_

- [x] 7.3 ECS 実行契約とログ契約の不足を補完する
  - Domain `Schedule` を `ExecutorKind::SingleThreaded` で明示し、実行契約をコードに固定する。
  - 入力/送信/受信/投影ログで `stage` / `room_id` / `participant_id` /
    `stream_kind` / `mode` の統一付与を完了する。
  - 追加テストで契約逸脱を検知できる状態にする。
  - _Requirements: 5.1, 5.2, 5.4_

- [x] 8. Desktop / VR 実入力と Adapter ログ契約の残課題を解消する
- [x] 8.1 Godot 入力イベントを WASD + マウスの実データで正規化する
  - `map_event_slots_to_input_events` のプレースホルダ変換を廃止し、
    `InputEvent` 実ペイロードから `Move` / `Look` / `Action` を抽出する。
  - Desktop 実行時は `SuteraClientBridge` から `DesktopInputState` を構築し、
    `GodotInputPort::from_desktop_state` 経路で `InputSnapshot` へ接続する。
  - Desktop モードで WASD + マウス操作が local/remote pose へ反映されることを
    テストで検証する。
  - _Requirements: 3.2, 4.2, 4.4_

- [x] 8.2 Adapter の `mode` ログを runtime mode と一致させる
  - 入力/投影ログの `mode = "unknown"` を廃止し、起動時に確定した
    `RuntimeMode` を `mode` フィールドへ連携する。
  - `room_id` / `participant_id` / `stream_kind` / `mode` の統一付与が
    Adapter〜Domain 経路で保持されることをテストで検証する。
  - _Requirements: 5.2, 5.4_

- [x] 8.3 VR モードでスティック軸入力を `VrInputState` へ接続する
  - VR 分岐で `from_events_with_mode` の Desktop 解釈に依存せず、
    スティック軸入力を `move_axis_x` / `move_axis_y` として取得する。
  - 必要に応じて回転入力（`yaw_delta` / `pitch_delta`）も VR 入力ソースから
    取得し、`normalize_vr_input` の経路へ接続する。
  - VR モードで連続スティック操作したときに local/remote pose が
    同期経路へ反映されることをテストで検証する。
  - _Requirements: 3.1, 3.4, 4.4_

- [x] 8.4 Godot シーンで `SuteraClientBridge` の起動・更新・入力配線を実装する
  - `node.tscn` に `SuteraClientBridge` ノードを配置し、対象 `Node3D` を
    `target_node` に接続する。
  - GDScript 側で `on_start` / `on_frame` をフレーム駆動し、
    `_input` または `_unhandled_input` で受けたイベントを `push_input_event`
    へ渡す。
  - Desktop/VR 両モードでイベントが Rust 側 `pending_input_events` に到達することを
    ログまたはテストで検証する。
  - _Requirements: 4.2, 4.4_

- [x] 8.5 Godot InputMap の操作アクションを定義し E2E で検証する
  - `project.godot` の `[input]` に `move_left` / `move_right` /
    `move_forward` / `move_back` / `look_left` / `look_right` /
    `look_up` / `look_down` を定義する。
  - Desktop では WASD + マウス、VR ではコントローラー入力の双方で
    同じ入力意味論へ正規化されることを確認する。
  - ローディング完了後に操作が反映される再現手順を `validation.md` 相当の
    検証観点へ反映する。
  - _Requirements: 3.2, 4.2, 4.4, 5.4_

- [x] 9. 全 feature 構成での移動回帰を解消し品質ゲートを再成立させる
- [x] 9.1 `cargo test --all-targets --all-features` で失敗している Movement 系テストを修正する
  - `movement_system_keeps_pose_unchanged_without_input` と
    `movement_system_updates_position_frame_rate_independently` が
    全 feature 構成でも成立するよう、入力正規化または移動適用の回帰を除去する。
  - テスト前提（無入力時に位置不変、`dt` 分割時の等価移動量）を仕様どおりに固定する。
  - _Requirements: 3.2, 4.2_

- [x] 9.2 bootstrap / pipeline の render frame 期待値回帰を解消する
  - `client_bootstrap_ticks_render_frame_with_core_and_openxr` と
    `gdextension_entry_pipeline_runs_ecs_and_buffers_frame` が
    全 feature 構成で安定して通るよう、初期フレームの pose 生成条件を整合させる。
  - 入力未発生フレームで意図しない `x` 方向ドリフトが発生しないことを確認する。
  - _Requirements: 3.2, 4.4_

- [x] 10. Godot Client の起動失敗（SIGSEGV）を原因解明し恒久対処する
- [x] 10.1 起動失敗を再現可能な形で固定し、失敗段階を特定する
  - `scripts/check_godot_client_startup.sh` の実行結果を基準に、`--import` と通常起動のどの段階で失敗しているかを切り分ける。
  - `client_core.gdextension` のロード、`gdext_rust_init` の初期化、OpenXR 初期化分岐のどこで異常終了するかをログで特定する。
  - 失敗時に原因推定できるログ（stage / mode / library path）を残し、再実行で同じ診断結果が得られる状態にする。
  - _Requirements: 4.3, 5.1, 5.4_

- [x] 10.2 特定した根本原因を修正し、起動時クラッシュを防止する
  - 根本原因に応じて GDExtension 初期化順序、null 安全性、モード分岐のいずれかを修正し、異常系でも process abort せず失敗を返すようにする。
  - Linux の headless 起動（`nix develop`）で `scripts/check_godot_client_startup.sh` が成功し、`client/godot/bin/linux/libclient_core.so` 配置確認を含む品質ゲートが通ることを確認する。
  - 修正後に `cargo test --workspace --all-targets` と既存の Godot 起動検証を再実行し、回帰がないことを確認する。
  - _Requirements: 4.3, 4.4, 5.4, 6.3_
