# Implementation Plan

- [x] 1. クライアント基盤と最小ライフサイクルを整える
- [x] 1.1 起動時の依存初期化と失敗理由の通知を成立させる
  - 起動要求を受けたときに初期化が行われる状態を作る
  - 失敗時に原因が明示できるようにする
  - _Requirements: 1.1, 1.4_

- [x] 1.2 終了時の停止とリソース解放を成立させる
  - 終了要求に対して安全に停止できる状態を作る
  - _Requirements: 1.3_

- [x] 1.3 実行中のフレーム更新を継続できるようにする
  - 実行中にフレーム更新が継続される状態を作る
  - _Requirements: 1.2_

- [x] 2. CoreECS の状態正本と更新サイクルを構築する
- [x] 2.1 ECS World の初期化と更新サイクルを成立させる
  - 起動時に World が初期化される状態を作る
  - フレーム更新でシステムが実行される状態を作る
  - 更新完了後に次フレームへ遷移できる状態を作る
  - _Requirements: 4.1, 4.2, 4.3_

- [x] 2.2 ゲーム状態の正本が ECS に集中していることを保証する
  - ゲーム状態が ECS 内にのみ保持される状態を作る
  - _Requirements: 5.1_

- [x] 3. GodotBridge の橋渡しと入力経路を整備する
- [x] 3.1 GDExtension 経由の橋渡し入口を初期化する
  - Godot から呼び出し可能な入口が初期化される状態を作る
  - 初期化失敗時にエラーを通知できる状態を作る
  - _Requirements: 3.1, 3.3_

- [x] 3.2 1フレーム1入力で ECS 更新を実行できるようにする
  - Godot からのフレーム入力を受け取れる状態を作る
  - 入力に基づく ECS 更新が実行される状態を作る
  - Godot からの API 呼び出しを受け付ける状態を作る
  - _Requirements: 3.2, 3.4_

- [x] 3.3 入力の取り込みと直接書換の拒否を成立させる
  - Godot/XR 入力を ECS への入力として扱える状態を作る
  - Godot 側の直接状態変更要求を拒否できる状態を作る
  - _Requirements: 5.2, 5.4_

- [x] 4. XR 起動と SteamVR 検知を整える
- [x] 4.1 OpenXR を有効化し SteamVR の起動可否を判定する
  - SteamVR 起動時に起動可能となる状態を作る
  - SteamVR 未起動時に理由を通知できる状態を作る
  - _Requirements: 2.1, 2.2_

- [x] 5. 描画投影の最小経路を整備する
- [x] 5.1 ECS 状態を描画へ反映し最小フレームを表示できるようにする
  - 描画に必要な最小フレームを表示できる状態を作る
  - ECS 状態が描画に反映される状態を作る
  - _Requirements: 2.3, 5.3_

- [x] 6. 統合フローと再現性検証を整える
- [x] 6.1 起動・更新・描画の一連フローを結線する
  - 起動から描画までが一貫して動作する状態を作る
  - _Requirements: 1.1, 1.2, 2.1, 2.3, 3.2_

- [x] 6.2 実行・検証の再現性をコードベース内で担保する
  - 実行に必要な前提や操作を出力・確認できる仕組みを用意する
  - 最小統合動作の検証ポイントを実行時に確認できる状態を作る
  - _Requirements: 6.1, 6.2, 6.3_

- [x] 7. テストによる品質確認を整える
- [x] 7.1 CoreECS の更新と状態正本の単体検証を行う
  - 更新サイクルと状態正本の保持を検証できる状態を作る
  - _Requirements: 4.1, 4.2, 4.3, 5.1_

- [x] 7.2 GodotBridge と XR 起動経路の統合検証を行う
  - ブリッジ初期化失敗時の動作を確認できる状態を作る
  - SteamVR 起動可否の判定を確認できる状態を作る
  - _Requirements: 3.1, 3.3, 2.1, 2.2_

- [x] 7.3 SteamVR 上での最小描画を確認する
  - 実機環境で起動と描画の確認ができる状態を作る
  - _Requirements: 2.1, 2.3, 6.3_

- [x] 8. GDExtension から ECS を呼び出す実配線を完成させる
- [x] 8.1 Godot から呼べる GDExtension API を追加する
  - `#[godot_api]` と `#[derive(GodotClass)]` を用いて Godot から呼べる入口を定義する
  - `on_start` / `on_frame` / `on_shutdown` を Godot から直接呼び出せる経路を用意する
  - _Requirements: 3.1, 3.2, 3.4_

- [x] 8.2 GDExtension から CoreECS への呼び出しを実接続する
  - Godot 側入力（最小の InputSnapshot）を生成し CoreECS に渡す
  - `RenderFrame` を受け取り、描画投影用のアダプタに渡す
  - _Requirements: 3.2, 4.2, 5.2, 5.3_

- [x] 8.3 GDExtension 初期化失敗時の通知を Godot 側に反映する
  - 初期化失敗の理由を Godot ログに出力できる状態を作る
  - _Requirements: 3.3_

- [x] 9. RenderStateProjector を実装して描画投影経路を確立する
- [x] 9.1 RenderFrame を Godot ノードに反映する最小投影を実装する
  - 1体分の Pose を Godot の Node3D に反映できる状態を作る
  - _Requirements: 2.3, 5.3_

- [x] 9.2 Godot 側の直接状態変更要求を明示的に拒否する
  - 直接書換を拒否し、理由を Godot 側に通知できる状態を作る
  - _Requirements: 5.4_

- [x] 10. 追加分の検証を整備する
- [x] 10.1 GDExtension 入口の統合テストを追加する
  - Godot からの呼び出しを模したテストで ECS 呼び出しが行えることを検証する
  - _Requirements: 3.1, 3.2, 3.4_

- [x] 10.2 RenderStateProjector の動作を確認する
  - RenderFrame の Pose が Godot 側の表示に反映されることを検証する
  - _Requirements: 2.3, 5.3_

- [x] 11. リファクタ: client/core の責務分割と API 整理を進める
- [x] 11.1 lib.rs のモジュール分割を行う
  - xr / bridge / ecs / render / godot / errors / tests へ分割し、公開 API を最小化する

- [x] 11.2 Bridge 層のラッパー構成を整理する
  - GodotBridgeApi / BridgePipeline / GodotBridgeAdapter の責務を再配置し、透過呼び出しを削減する

- [x] 11.3 FrameId 更新の責務を一元化する
  - SuteraClientBridge と ClientBootstrap の二重インクリメントを解消する

- [x] 11.4 エラーの型情報を保持できる設計に修正する
  - reason 文字列化の依存を減らし、BridgeErrorState に型情報を保持させる

- [x] 11.5 RenderFrame の Pose 構造を明確化する
  - 単一 Pose か複数 Pose かを設計で決め、取得規約を整理する

- [x] 11.6 CoreEcs の初期化フローを安全化する
  - 未初期化状態を露出しない生成 API に寄せる

- [x] 11.7 デモ用の挙動を本番ロジックから分離する
  - advance_frame の仮想アニメを feature または専用システムに切り出す

- [x] 11.8 Windows DLL 配置ロジックを一本化する
  - build.rs と scripts/build-client-core-windows.sh の責務を統合する

- [x] 11.9 GDExtension 検証スクリプトを再利用可能にする
  - verify_gdextension.gd を検証ノード/ユーティリティとして整理する

- [x] 12. godot-bevy-ecs-boundary の境界規律を適用する
- [x] 12.1 Domain (client/core) を純 Rust に分離する
  - Godot 型依存 (`Gd<T>` / Node3D / Transform3D / XrInterface など) を core から除去する
  - Input/Output ports を純粋データ型で固定する
  - _Requirements: 5.1, 5.2_

- [x] 12.2 Adapter 層に Godot 依存を集約する
  - GDExtension 入口/描画投影/XR 初期化を adapter 側へ移す
  - Godot 呼び出しは Adapter でのみ行う
  - _Requirements: 3.1, 3.2, 5.3_

- [x] 12.3 公開 API を再整理する
  - core は Domain 型のみ re-export する
  - adapter は Godot 向け API を公開する
  - _Requirements: 5.1, 5.3_

- [x] 12.4 テスト分離を行う
  - core の unit tests は Godot 依存ゼロにする
  - Godot 依存テストは adapter 側へ移動または feature 分離する
  - _Requirements: 4.1, 5.1, 6.2_

- [x] 12.5 入出力ポートを整備する
  - Godot InputEvent を Domain InputSnapshot に変換する adapter を追加する
  - Domain output を Godot 変換へ適用する処理を adapter 側に集約する
  - _Requirements: 3.2, 5.2, 5.3_

- [x] 13. レビュー指摘の境界不整合を是正する
- [x] 13.1 Noop 固定入力を廃止し、Godot 入力を Domain 入力へ変換する
  - `GodotInputPort::empty()` 依存をやめ、1フレーム内イベント収集経路を実装する
  - `InputEvent::Noop` 以外の最小イベント型（例: Move / Look / Action）を導入する
  - `from_events` の未使用状態を解消し、`InputEvent* -> Domain input` 変換テストを追加する
  - _Requirements: 3.2, 5.2, 5.3_

- [x] 13.2 crate を `client-domain` と `client-godot-adapter` に分割する
  - `client-domain` は pure Rust + bevy_ecs のみ依存し、Godot 依存を完全排除する
  - `client-godot-adapter` は GDExtension 入口 / XR / 描画投影 / 入出力変換を保持する
  - 既存 `client-core` 参照を新 crate 構成へ移行し、ビルド・テスト導線を更新する
  - _Requirements: 5.1, 5.2, 5.3, 6.2_

- [x] 13.3 Domain API から Godot 固有語を排除する
  - `GodotBridge*` 命名を `RuntimeBridge*` など中立名へ置換する
  - Domain の public API から engine 固有概念を除去し、ports で接続する
  - rename 後の参照更新と回帰テストを実施する
  - _Requirements: 5.1, 5.2_

- [x] 13.4 出力投影失敗を観測可能にする
  - `RenderStateProjector::project` の失敗を握りつぶさず、BridgeErrorState に記録する
  - `target invalid` 時の失敗パスに対する adapter テストを追加する
  - _Requirements: 3.3, 6.2_
