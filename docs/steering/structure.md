# プロジェクト構造

## 組織方針

- Rust workspace でドメインごとに crate を分離し、責務を明確化する
- API 型定義、ドメインロジック、実サーバを分けて依存方向を固定する

## ディレクトリ・パターン

### Bloom API
**Location**: `/bloom/api/`  
**Purpose**: シグナリングのリクエスト/イベント/エラー型の定義  
**Example**: `bloom/api/src/requests.rs`

### Bloom Core
**Location**: `/bloom/core/`  
**Purpose**: ルーム/参加者管理や Join/Leave などのドメインロジック  
**Example**: `bloom/core/src/room.rs`

### Bloom WS Server
**Location**: `/bloom/ws/`  
**Purpose**: `/ws` エンドポイントの WebSocket サーバ実装  
**Example**: `bloom/ws/src/server.rs`

### Syncer
**Location**: `/syncer/`  
**Purpose**: P2P 同期ライブラリ（メッセージ、ルーティング、WebRTC Transport）  
**Example**: `syncer/src/messages/`

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
更新日: 2026-01-22
