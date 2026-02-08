# Requirements Document

## Introduction

本仕様は、`syncer` を client に接続し、2 participant 間で remote pose が
リアルタイムに反映される最小の縦スライスを対象とする。加えて、デバッグ効率を
高めるために、VR コントローラーによる移動入力と、SteamVR 非依存で検証できる
デスクトップ（非VR）モードを要件に含める。

## Scope

- In Scope:
  - 2 participant の pose 送受信と remote pose 描画反映
  - VR コントローラー入力によるローカル移動
  - デスクトップ（非VR）モードでの WASD + マウス操作
  - 失敗時の診断可能なログと検証手順
- Out of Scope:
  - 音声同期（送受信・再生・キャプチャ）
  - Bloom 本番シグナリング配線

## Requirements

### Requirement 1: 2 participant 間の pose リアルタイム同期

**Objective:** As a 開発者, I want 2 participant 間で pose が同期されるように,
so that remote pose 反映の最小縦スライスを検証できる

#### Acceptance Criteria (Requirement 1)

1. When 2 participant が同一 room 参加済みのとき, the Client shall ローカル
   participant の pose を継続的に送信できる
2. When remote participant の pose を受信したとき, the Client shall
   participant_id ごとに最新 pose を保持する
3. When 最新 pose が更新されたとき, the Client shall 次フレームで remote pose を
   描画へ反映する
4. If room 未参加または peer 未確立のとき, the Client shall pose 同期処理を
   安全にスキップし、診断ログを出力する

### Requirement 2: peer ライフサイクルに追従した状態管理

**Objective:** As a 開発者, I want peer の参加/離脱に追従して状態管理できるように,
so that stale な remote pose が残らない

#### Acceptance Criteria (Requirement 2)

1. When PeerJoined が観測されたとき, the Client shall 対象 peer の同期状態を
   初期化する
2. When PeerLeft が観測されたとき, the Client shall 対象 peer の remote pose と
   描画対象を破棄する
3. If 同一 peer の再参加が発生したとき, the Client shall 旧状態を再利用せず、
   新しいセッションとして再初期化する

### Requirement 3: VR コントローラーによる空間内移動

**Objective:** As a 開発者, I want VR コントローラー入力で移動できるように,
so that VR 実行時に pose 送信元を手動で変化させて同期検証できる

#### Acceptance Criteria (Requirement 3)

1. When VR モードで移動入力が与えられたとき, the Client shall コントローラー
   入力をローカル pose の移動量へ変換する
2. While 移動入力がないとき, the Client shall 入力起因の位置変化を発生させない
3. When 入力で更新された pose が確定したとき, the Client shall 同フレーム系列で
   同期送信対象として扱う
4. If VR 入力が取得できないとき, the Client shall 失敗理由をログに出力し、
   クライアントを異常終了させない

### Requirement 4: デスクトップ（非VR）デバッグモード

**Objective:** As a 開発者, I want SteamVR なしで操作できるデバッグモードがほしい,
so that ローカル環境で pose 同期を迅速に再現検証できる

#### Acceptance Criteria (Requirement 4)

1. The Client shall 起動時に VR モードとデスクトップ（非VR）モードを選択できる
2. When デスクトップモードが有効なとき, the Client shall WASD 入力で移動し、
   マウス入力で視点回転できる
3. When デスクトップモードが有効なとき, the Client shall SteamVR/OpenXR が
   未起動でも起動できる
4. Where デスクトップモードで pose が更新された場合, the Client shall VR モード
   と同じ同期経路で remote peer へ送信する

### Requirement 5: デバッグ可能性と検証手順

**Objective:** As a 開発者, I want 同期失敗時の原因を追跡できるように, so that
実装のデバッグ時間を短縮できる

#### Acceptance Criteria (Requirement 5)

1. The Client shall pose 同期の主要イベント（join/send/receive/leave）を
   `tracing` で記録する
2. Where ログを出力する場合, the Client shall `room_id` / `participant_id` /
   `stream_kind` / 実行モード（VR/desktop）を含める
3. The Client shall 最小の再現手順（2 participant での同期確認）を
   ドキュメント化する
4. If 同期に失敗したとき, the Client shall 失敗箇所を推定できる情報
   （入力・送信・受信・投影のどの段階か）をログに残す

### Requirement 6: スコープ境界の維持

**Objective:** As a 開発者, I want 本スライスの境界を明確に保ちたい, so that
1PR でレビュー可能なサイズを維持できる

#### Acceptance Criteria (Requirement 6)

1. The Client shall 音声同期機能を本スライスの受入条件に含めない
2. The Client shall Bloom 本番シグナリング配線を本スライスの受入条件に含めない
3. When pose 同期を検証するとき, the Client shall 本スライス内で準備可能な
   接続経路（既存テスト可能経路）で成立を示せる

