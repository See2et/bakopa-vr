# プロジェクト構造

## 組織方針

- Rust workspace でドメインごとに crate を分離し、責務を明確化する
- API 型定義、ドメインロジック、実サーバを分けて依存方向を固定する

## ディレクトリ・パターン

### Bloom API

**Location**: `/bloom/api/`  
**Purpose**: シグナリングのリクエスト/イベント/エラー型の定義
(ClientToServer / ServerToClient, RelaySdp/Ice, ErrorCode)  
**Example**: `bloom/api/src/requests.rs`

### Bloom Core

**Location**: `/bloom/core/`  
**Purpose**: ルーム/参加者管理や Join/Leave などのドメインロジック
(最大 8 名、UUID ベースの RoomId/ParticipantId)  
**Example**: `bloom/core/src/room.rs`

### Bloom WS Server

**Location**: `/bloom/ws/`  
**Purpose**: `/ws` エンドポイントの WebSocket サーバ実装  
**Example**: `bloom/ws/src/server.rs`

- バイナリ `main.rs` が subscriber を初期化する
- レート制御は 1 秒あたり 20 メッセージ/セッションを基準とする

### Syncer

**Location**: `/syncer/`  
**Purpose**: P2P 同期ライブラリ（メッセージ、ルーティング、WebRTC Transport）  
**Example**: `syncer/src/messages/`

- `BasicSyncer` が Router/TransportInbox/RateLimiter を束ねる
- WebRTC Transport は DataChannel label `sutera-data` を前提に扱う

### Client Domain

**Location**: `/client/domain/`  
**Purpose**: bevy_ecs ベースのクライアントドメインロジック
（Components/Resources/Systems、ポート定義、純 Rust のエラー型）  
**Example**: `client/domain/src/bridge.rs`

- Godot 型や GDExtension 型を直接参照しない
- `thiserror` による型付きエラーを定義し、境界判断を明確化する

### Client Godot Adapter

**Location**: `/client/godot-adapter/`  
**Purpose**: godot-rust (GDExtension) による Adapter 層
（Godot 入出力と Domain ポートの変換）  
**Example**: `client/godot-adapter/src/godot.rs`

- Godot API 呼び出し・ノードアクセスはこの層に閉じ込める
- Domain output の適用と main-thread 制約の吸収を担当する

### Godot Project

**Location**: `/client/godot/`  
**Purpose**: Godot プロジェクト設定・シーン・GDScript 補助実装  
**Example**: `client/godot/project.godot`

### 仕様

**Location**: `/docs/specs/`  
**Purpose**: 機能ごとの仕様・設計・タスク管理  
**Example**: `docs/specs/*`

## 命名規則

- **Crate**: kebab-case (`bloom-api`, `bloom-ws`)
- **Files/Modules**: snake_case (`rate_limiter.rs`)
- **Types/Enums**: PascalCase
- **Functions/Methods**: snake_case

## Import の整理

```rust
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::messages::Envelope;
use crate::rate_limiter::RateLimiter;
```

- `std` → 外部 crate → `crate` の順で並べる
- モジュール境界は `crate::` / `super::` で明示する

## コード構成の原則

- 公開 API は `Arc<T>` などのラッパー型を露出せず、シンプルな型で設計する
- プリミティブ型の乱用を避け、意味のある型で表現する
- 実装は `src/` 配下に集約し、`tests/` は統合テストとして分離する

---
更新日: 2026-02-07
