# Requirements Document

## Introduction

本仕様は、Godot Engine と bevy_ecs を組み合わせ、GDExtension (gdext) を
通じて Godot から ECS を呼び出せる最小クライアントを対象とする。SteamVR
で起動できることを到達点とし、状態の真実は bevy_ecs に集中させ、Godot
は描画中心に保つ。

## Requirements

### Requirement 1: 起動と終了の最小ライフサイクル

**Objective:** As a 開発者, I want 最小クライアントが起動と終了を行えるように, so that 統合の確認ができる

#### Acceptance Criteria (Requirement 1)

1. When 起動が要求されたとき, the Client shall メインループを開始する
2. While 実行中, the Client shall フレーム更新を継続する
3. When 終了が要求されたとき, the Client shall リソースを解放して終了する
4. If 起動に失敗したとき, the Client shall 失敗理由を明示して終了する

### Requirement 2: SteamVR での起動確認

**Objective:** As a 開発者, I want SteamVR でクライアントが起動できるように, so that 最小の VR 起動経路を確認できる

#### Acceptance Criteria (Requirement 2)

1. When SteamVR が起動済みであるとき, the Client shall SteamVR 上でクライアントを起動できる
2. If SteamVR が未起動のとき, the Client shall 起動不可の理由を通知する
3. When SteamVR 上で起動したとき, the Client shall 最小の描画フレームを表示する

### Requirement 3: GDExtension (gdext) による呼び出しブリッジ

**Objective:** As a 開発者, I want Godot から ECS を呼び出せるように, so that 連携の核を検証できる

#### Acceptance Criteria (Requirement 3)

1. The Client shall GDExtension (gdext) を介した呼び出し経路を提供する
2. When Godot 側から呼び出しが行われたとき, the Client shall bevy_ecs の処理を実行できる
3. If ブリッジ初期化に失敗したとき, the Client shall エラーを通知する
4. Where GDExtension が有効な場合, the Client shall Godot からの API 呼び出しを受け付ける

### Requirement 4: bevy_ecs の最小実行ループ

**Objective:** As a 開発者, I want ECS の更新が行われるように, so that 最小の同期処理を検証できる

#### Acceptance Criteria (Requirement 4)

1. When クライアントが起動したとき, the Client shall bevy_ecs の World を初期化する
2. While フレーム更新中, the Client shall 登録された ECS システムを実行する
3. When ECS の更新が完了したとき, the Client shall 次フレームの更新に移る

### Requirement 5: 状態の真実は bevy_ecs に集中

**Objective:** As a 開発者, I want bevy_ecs が状態の真実を保持するように, so that Godot を描画中心に保てる

#### Acceptance Criteria (Requirement 5)

1. The Client shall ゲーム状態の正本を bevy_ecs に保持する
2. When Godot 側が状態変更を要求したとき, the Client shall ECS を通じてのみ更新を行う
3. When ECS の状態が更新されたとき, the Client shall Godot 側の描画状態へ反映する
4. If Godot 側が直接状態を書き換えようとしたとき, the Client shall 更新を拒否して理由を通知する

### Requirement 6: 実行手順と検証

**Objective:** As a 開発者, I want 実行・検証の手順が明確であること, so that 最小動作を再現できる

#### Acceptance Criteria (Requirement 6)

1. The Client shall 実行手順をドキュメント化する
2. The Client shall 最小動作の検証手順を提供する
3. When 手順に従って実行したとき, the Client shall 最小の統合動作が確認できる
