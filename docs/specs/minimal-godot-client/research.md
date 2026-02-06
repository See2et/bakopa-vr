# Research & Design Decisions

<!-- markdownlint-disable MD013 -->

## Summary

- **Feature**: minimal-godot-client
- **Discovery Scope**: Complex Integration
- **Key Findings**:
  - GDExtension は Godot の実行時にネイティブ共有ライブラリを読み込み、エンジン再ビルド無しで拡張できる。互換性は Godot のマイナーバージョンに依存する。
  - godot-rust (gdext) は Godot 4 の GDExtension API を Rust から利用するためのバインディングを提供する。
  - Godot 4 では OpenXR がコアに統合され、SteamVR などの OpenXR ランタイムと連携できる。

## Research Log

### GDExtension の役割と互換性

- **Context**: Godot から Rust を呼び出す経路の前提を確認
- **Sources Consulted**: Godot 公式ドキュメント
- **Findings**:
  - GDExtension は実行時に共有ライブラリをロードし、エンジン再ビルド不要
  - 互換性は Godot のマイナーバージョンに依存し、バージョン不一致で動作しない可能性
- **Implications**:
  - Godot バージョン固定と拡張ビルドの整合性が必要

### godot-rust (gdext) のバインディング

- **Context**: Rust で Godot 4 API を扱う際の公式・事実情報の確認
- **Sources Consulted**: godot-rust の API ドキュメント
- **Findings**:
  - gdext は GDExtension API を利用して Godot 4 のクラス API を Rust へ公開する
  - Godot の API に近い構造の生成コードを持つ
- **Implications**:
  - Godot クラスに直接依存するとテスト難易度が上がるため、抽象化レイヤが必要

### Godot 4 と OpenXR / SteamVR

- **Context**: SteamVR 起動条件と Godot 側の要件を把握
- **Sources Consulted**: Godot 公式ドキュメント、Godot XR 記事
- **Findings**:
  - Godot 4 では OpenXR がコアに統合され、プラグイン無しで利用可能
  - OpenXR は起動時に有効化され、XR 用の viewport 指定が必要
- **Implications**:
  - 起動フローに OpenXR 初期化と XR viewport の設定が必須

### bevy_ecs の位置づけ

- **Context**: ECS を単体利用できるか確認
- **Sources Consulted**: bevy の ECS ドキュメント
- **Findings**:
  - bevy_ecs は Bevy の ECS 実装であり単体利用が可能
- **Implications**:
  - Godot 非依存のコア状態管理として使用できる

## Architecture Pattern Evaluation

| Option     | Description                                | Strengths            | Risks / Limitations          | Notes                            |
| ---------- | ------------------------------------------ | -------------------- | ---------------------------- | -------------------------------- |
| Hexagonal  | Core に依存しない ports と adapters を分離 | テスト容易性、拡張性 | Adapter 実装が増える         | 依存性逆転の要求に合致           |
| Godot 直結 | Godot クラスを直接参照                     | 実装が簡易           | テスト困難、Godot 依存が強い | new_alloc などの実行時依存が強い |
| ECS 内包   | Godot 側に状態を持つ                       | Godot 標準に近い     | 要件 5 に反する              | 状態の真実が分散する             |

## Design Decisions

### Decision: Hexagonal を採用し、Core を Godot から切り離す

- **Context**: Godot 依存の API を直接呼ぶとテスト不可になる問題
- **Alternatives Considered**:
  1. Godot 直結
  2. Hexagonal
- **Selected Approach**: Core は bevy_ecs とドメイン型に閉じ、Godot 依存は Adapter に隔離
- **Rationale**: 依存性逆転とテスト容易性を満たす
- **Trade-offs**: Adapter 層の設計・保守コストが増える
- **Follow-up**: Adapter の契約設計を実装時に精査

### Decision: SteamVR 起動確認は OpenXR を前提とする

- **Context**: SteamVR は OpenXR ランタイムとして利用される
- **Alternatives Considered**:
  1. OpenXR (Godot 4 標準)
  2. OpenVR プラグイン (外部依存)
- **Selected Approach**: Godot 4 の OpenXR に統一
- **Rationale**: コア統合により依存を減らし、起動フローを単純化
- **Trade-offs**: OpenXR 初期化の設定が必須
- **Follow-up**: 具体的な Project Settings と XR viewport 設定の検証

### Decision: XR 追跡は Godot/OpenXR を一次情報として扱う

- **Context**: XR ノードはランタイムにより自動更新され、ECS 側で正本化できない
- **Alternatives Considered**:
  1. ECS がすべての状態正本を保持
  2. XR 追跡のみ Godot/OpenXR を一次情報とする
- **Selected Approach**: XR 追跡は Godot/OpenXR から入力として取り込み、ゲーム状態の正本は ECS に集中
- **Rationale**: XR 追跡の現実と整合し、テスト性も確保できる
- **Trade-offs**: 正本が一元化されないため、境界の明文化が必要
- **Follow-up**: 入力スナップショットの定義とモック戦略の明確化

### Decision: GodotBridge を on_frame(input) に統合

- **Context**: on_input と on_frame の分離は入力フレームと描画フレームのズレを生みやすい
- **Alternatives Considered**:
  1. Bridge/API を統合して 1 フレーム 1 入力に限定
  2. Core を分割 API に変更
  3. FrameId 検証のみ追加
- **Selected Approach**: Bridge は on_frame(input) の単一入口とし、CoreECS の tick に入力を渡す
- **Rationale**: フレーム整合性を構造的に担保できる
- **Trade-offs**: Godot 側で入力をフレームバッファする必要がある
- **Follow-up**: InputSnapshot 生成タイミングの明確化

## Risks & Mitigations

- GDExtension と Godot のバージョン不整合 — Godot のマイナーバージョン固定と拡張ビルドの同期
- Godot API 直結によるテスト不能 — Port/Adapter で隔離し、Core を純 Rust でテスト
- OpenXR 初期化ミスによる SteamVR 起動失敗 — 起動時チェックと明確な失敗理由の提示

## References

- <https://docs.godotengine.org/en/stable/tutorials/scripting/gdextension/what_is_gdextension.html>
- <https://docs.godotengine.org/en/latest/classes/class_openxrinterface.html>
- <https://godot-rust.github.io/docs/gdext/master/godot/>
- <https://docs.rs/bevy/latest/bevy/ecs/index.html>
- <https://godotengine.org/article/godot-openxr-vendors-plugin-400/>
