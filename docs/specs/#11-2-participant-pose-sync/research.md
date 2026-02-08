# Research & Design Decisions

## Summary

- **Feature**: `client-pose-sync-vertical-slice`
- **Discovery Scope**: Extension
- **Key Findings**:
  - `syncer` は `SendPose` / `PoseReceived` と `PeerJoined` / `PeerLeft` を公開しており、クライアント側に同期セッション境界を追加すれば統合可能である。
  - 既存 `client` は `syncer` 非依存で、`InputSnapshot` は保存のみで未消費、`GodotInputPort` 変換はプレースホルダであるため、入力処理と同期送受信の追加が必須である。
  - OpenXR 依存だけではデバッグ速度が低下するため、デスクトップ（非VR）入力プロファイルを同一 Domain 契約へ正規化する設計が妥当である。
  - Pose を unordered/unreliable で配送する以上、受信順と新旧が一致しないため、`PoseVersion { session_epoch, pose_seq }` による新旧判定規約が必須である。
  - peer lifecycle と pose snapshot は信頼性要件が異なるため、`Control stream`（reliable/ordered）と `Pose stream`（unordered + partial reliability）を論理分離する必要がある。
  - `PeerLeft` 未着は現実的に発生するため、client 側には fail-safe な liveness timeout が必要。ただし timeout は hard delete ではなく `inactive` 遷移として扱うのが妥当である。

## Research Log

### 既存拡張ポイントと統合経路

- **Context**: 既存コードへ最小侵襲で pose 同期を追加するための接続点を確認。
- **Sources Consulted**:
  - `client/domain/src/bridge.rs`
  - `client/domain/src/ecs.rs`
  - `client/godot-adapter/src/godot.rs`
  - `client/godot-adapter/src/ports.rs`
  - `syncer/src/lib.rs`
- **Findings**:
  - Domain のフレーム入力は `BridgePipeline::on_port_input -> RuntimeBridgeAdapter::on_frame -> EcsCore::tick` の単一路。
  - `CoreEcs::tick` は `InputSnapshot` を `World` に一時挿入するが、現行システムは入力を解釈しない。
  - `syncer` 側は `SyncerRequest::SendPose` と `SyncerEvent::PoseReceived` を持ち、peer ライフサイクルイベントも提供済み。
- **Implications**:
  - Domain 側に「入力更新」と「同期イベント反映」を分離したポートを追加すれば、Godot 依存を増やさず拡張できる。

### OpenXR とデスクトップ共存の成立性

- **Context**: Requirement 3/4 の両立可否（VR 入力 + 非VR デバッグモード）を確認。
- **Sources Consulted**:
  - Godot ドキュメント（OpenXRInterface, XRInterface, Viewport）
- **Findings**:
  - Godot の OpenXR 利用は `XRInterface` 初期化と viewport 側の XR 有効化が前提。
  - XR 非有効時でも通常カメラ経路で 3D シーンを実行できる。
- **Implications**:
  - 起動モードを `Vr` / `Desktop` に分離し、Adapter で入力ソースを切替える設計が妥当。

### WebRTC チャネル特性と pose 配送

- **Context**: pose 同期を低遅延優先で扱う設計の妥当性を確認。
- **Sources Consulted**:
  - `syncer/src/lib.rs` (`TransportSendParams::for_stream`)
  - `syncer/tests/webrtc_transport_pose_integration.rs`
  - WebRTC crate / API ドキュメント（DataChannel 設定）
- **Findings**:
  - `StreamKind::Pose` は unordered/unreliable DataChannel へマップされる設計。
  - `StreamKind::ControlJoin` / `ControlLeave` は reliable/ordered 側の扱いが前提で、欠落時は状態整合性リスクが高い。
  - 既存テストで pose 配送とパラメータ設定が検証済み。
  - unordered 前提では古い pose が新しい pose の後に到着しうるため、受信順をそのまま適用すると巻き戻りが発生する。
- **Implications**:
  - 本スライスでは voice を追加せず、pose 経路に集中することで要件達成とリスク低減を両立できる。
  - remote pose の「最新」は `PoseVersion { session_epoch, pose_seq }` の辞書順比較で定義し、`incoming <= last_applied` を stale drop する必要がある。
  - 設計上は論理ストリームを分離し、Control と Pose で配送契約・監視項目・テストケースを別管理する必要がある。

### 観測性とデバッグ要件

- **Context**: Requirement 5（原因追跡）を設計段階で担保。
- **Sources Consulted**:
  - `client/domain/src/bridge.rs`
  - `client/godot-adapter/src/godot.rs`
  - `docs/steering/tech.md`
- **Findings**:
  - 既存コードは `tracing` で frame/started などを出しているが、`room_id` / `participant_id` / mode を統一記録する契約は未定義。
- **Implications**:
  - 同期境界に `TracingContext` を保持し、Domain/Adapter 双方で共通キーを出す設計が必要。

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| Hexagonal 拡張 | Domain に Sync/Input ポートを追加し Adapter で具体化 | 既存方針と整合、テスト容易性が高い | ポート設計の初期コスト | 採用 |
| Adapter 直結 | Godot Adapter で `syncer` と ECS を直接接続 | 実装が速い | Domain ルール逸脱、回帰リスク増 | 非採用 |
| Domain へ Godot 型導入 | Domain が InputEvent を直接解釈 | 変換段が減る | 境界規律違反、テスト難化 | 非採用 |

## Design Decisions

### Decision: Domain に Pose 同期セッション境界を導入する

- **Context**: 1.1-2.3 を満たすには peer 状態と pose 状態の正本を Domain で保持する必要がある。
- **Alternatives Considered**:
  1. Adapter で remote pose を直接保持
  2. Domain に `SyncSessionPort` を追加し状態集約
- **Selected Approach**: Domain が `SyncSessionPort` から同期イベントを受け取り、remote pose 状態を更新して `RenderFrame` へ投影する。
- **Rationale**: 状態正本の集中とテスト容易性を両立できる。
- **Trade-offs**: Domain のモデルと契約が増える。
- **Follow-up**: peer 再参加時の初期化規約をテストで固定する。

### Decision: 入力を VR/Desktop 共通の型付き入力へ正規化する

- **Context**: 3.1-4.4 を同時に満たすには入力デバイス差異を Adapter 境界で吸収する必要がある。
- **Alternatives Considered**:
  1. VR と Desktop で別の Domain API を持つ
  2. 共通 `LocomotionInput` へ変換して Domain へ渡す
- **Selected Approach**: Adapter で `InputProfile`（VrController/Desktop）を選択し、共通 InputSnapshot へ変換する。
- **Rationale**: Domain の単純性とモード切替の保守性が高い。
- **Trade-offs**: Adapter 側変換ロジックが増える。
- **Follow-up**: キー割り当てと感度は設計時に固定し、将来拡張可能な設定点を残す。

### Decision: 本スライスでは Bloom 本番配線と voice を除外する

- **Context**: 6.1-6.3 で 1PR 粒度を維持する必要がある。
- **Alternatives Considered**:
  1. Pose + Voice + Bloom を同時実装
  2. Pose 経路を先に縦通しし、残りは次スライスへ分離
- **Selected Approach**: Pose 同期に限定し、voice と Bloom 本番配線は明示的に deferred。
- **Rationale**: 影響範囲を抑え、検証とレビューを成立させる。
- **Trade-offs**: 音声E2Eの完成は次フェーズへ持ち越し。
- **Follow-up**: 次スライスで signaling 実配線の不確実性を解消する。

### Decision: remote pose 適用に PoseVersion 比較規約を導入する

- **Context**: unordered/unreliable pose 配送では到着順と時系列が一致しないため、`latest` の定義を明示しないと巻き戻りが起こる。
- **Alternatives Considered**:
  1. 受信順をそのまま採用する
  2. `timestamp_micros` 比較で新旧判定する
  3. `PoseVersion { session_epoch, pose_seq }` で判定する
- **Selected Approach**: `PoseVersion` を pose イベントに付与し、participant ごとに `last_applied_version` を保持して辞書順比較で反映可否を決める。
- **Rationale**: 送信元ローカル時刻への依存を避けつつ、rejoin を含むセッション切替を明示的に扱える。
- **Trade-offs**: ペイロードと状態モデルが増える。
- **Follow-up**: `stale_drop` のメトリクスを追加し、逆順到着・rejoin の統合テストを固定する。

### Decision: 同期イベントを Control/Pose の論理ストリームに分離する

- **Context**: `PeerJoined` / `PeerLeft` は欠落耐性が低く、`PoseReceived` は鮮度優先で欠落許容できるため、同一配送契約では要件を満たしにくい。
- **Alternatives Considered**:
  1. 単一ストリームで全イベントを同一設定で配送
  2. 論理ストリームを `Control` / `Pose` に分離し契約を分ける
- **Selected Approach**: `Control stream`（reliable/ordered）と `Pose stream`（unordered + partial reliability）を論理的に分離する。
- **Rationale**: 状態整合性が必要な lifecycle と低遅延優先の pose を両立できる。
- **Trade-offs**: 契約と監視項目、テスト観点が増える。
- **Follow-up**: 実装フェーズで物理チャネル分離の要否を評価し、少なくとも論理契約とメトリクスは先行固定する。

### Decision: timeout は client liveness のみで扱い、削除は PeerLeft に限定する

- **Context**: `PeerLeft` 未着（切断/NAT/プロセス死/ICE 不成立）時に ghost が残る一方、timeout 即削除は誤判定で UX を損なう。
- **Alternatives Considered**:
  1. `PeerLeft` のみで削除し、timeout を持たない
  2. timeout 超過で即 hard delete する
  3. timeout 超過で `inactive` に遷移し、`PeerLeft` でのみ hard delete する
- **Selected Approach**: client は `last_update_instant` による timeout 判定で `SuspectedDisconnected` を管理し、authoritative 削除は `PeerLeft` に限定する。
- **Rationale**: Bloom の membership 権威を保ちつつ、表示上の ghost を減らせる。
- **Trade-offs**: liveness 状態と表示ポリシーの管理コストが増える。
- **Follow-up**: `remote_liveness_transition_total` を監視し、timeout 閾値 `T_inactive` を調整する。

## Risks & Mitigations

- 入力変換の誤差で VR/Desktop の挙動がずれる — InputProfile ごとに同一受入テスト（移動・停止・回転）を定義する。
- peer 離脱時の stale 表示 — `PeerLeft` 到達時に状態破棄と描画削除を同一トランザクションで実行する。
- unordered 受信で古い pose が最新を上書きする — `PoseVersion` 比較で stale drop し、`pose_sync_stale_drop_total` を監視する。
- Control 系イベントの欠落で peer 状態が破綻する — Control stream を reliable/ordered で扱い、`control_delivery_error_total` を監視する。
- `PeerLeft` 未着で ghost が残る — timeout で `inactive` へ遷移し、表示ポリシーを degrade する（hard delete はしない）。
- 同期失敗時に原因切り分けが困難 — `tracing` フィールドを統一（mode/room_id/participant_id/stream_kind/stage）する。
- Bloom 実配線未確定のまま設計が膨張 — 接続契約を `SyncSessionPort` に閉じ込め、transport 実装差異を隔離する。

## References

- <https://docs.godotengine.org/en/stable/classes/class_openxrinterface.html>
- <https://docs.godotengine.org/en/4.4/classes/class_xrinterface.html>
- <https://docs.godotengine.org/en/stable/classes/class_viewport.html>
- <https://docs.rs/webrtc/latest/webrtc/data_channel/data_channel_init/struct.RTCDataChannelInit.html>
- <https://docs.rs/bevy_ecs/latest/bevy_ecs/>
- <https://docs.rs/godot/latest/godot/>
