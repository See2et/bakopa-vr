# Technology Stack

## Project Type
分散型ソーシャル VR プラットフォーム「SuteraVR」の Minimal Value Product。Unity 製 VR/デスクトップクライアントと Rust 製サイドカー（iroh ベース）を組み合わせ、Bloom サーバーを介したフェデレーション + P2P の二層構造でルーム同期を行う。

## Core Technologies

### Primary Language(s)
- **Language**: C# (Unity 2022 LTS)、Rust 1.81 (Edition 2024)
- **Runtime/Compiler**: Unity IL2CPP / Mono、rustc + cargo
- **Language-specific tools**: Unity Package Manager (UPM)、cargo、bindgen、cbindgen

### Key Dependencies/Libraries
- **Unity XR Interaction Toolkit**: VR 入力・インタラクションを統合。
- **Rust Sidecar (bakopa-net)**: Unity から IPC で呼び出される Rust 製常駐プロセス。iroh をラップし、P2P セッション制御・暗号化・ストリーム管理を担当。
- **iroh**: QUIC ベースの P2P ライブラリ。データ/音声ストリームのマルチプレクシングを提供。
- **Bloom**: Rust 製フェデレーションサーバー。シグナリング、UGC メタデータ、STAN ベースのメッセージブローカーを持ち、Federation + P2P の二層同期を調整。

### Application Architecture
Unity クライアントは VR/デスクトップ双方の描画と UX を担い、同一デバイス上で Rust 製サイドカー `bakopa-net` を独立プロセスとして起動。Unity ↔ サイドカー間は gRPC over Unix Domain Socket / Named Pipe で通信し、サイドカーが iroh を通じてピア接続・ストリーミングを管理する。Bloom サーバーはフェデレーションレイヤーとして、STAN 互換のメッセージングでルームメタデータ・UGC を配信し、参加要求を各ピアにブリッジする。Bloom から配布されたノード情報をもとに、リアルタイム同期は iroh P2P が担う二層モデル。

### Data Storage (if applicable)
- **Primary storage**: Bloom 内部で Postgres + オブジェクトストレージ（Unity AssetBundle）。サイドカーはステートレス。
- **Caching**: サイドカー内にピア・Bloom ノードキャッシュ（in-memory + JSON snapshot）。
- **Data formats**: MessagePack ベースのルームプロトコル、UGC は Unity AssetBundle、音声は Opus。

### External Integrations (if applicable)
- **APIs**: Bloom Federation API（ルーム作成、鍵配布、UGC バージョン管理）。
- **Protocols**: QUIC (iroh)、gRPC（Unity ↔ サイドカー、サイドカー ↔ Bloom）、WebSocket（Bloom ダッシュボード）。
- **Authentication**: Bloom で発行する Ed25519 鍵ペア + OAuth2 デリゲーション。ピア間は鍵署名で相互認証。

### Monitoring & Dashboard Technologies (if applicable)
- **Dashboard Framework**: Next.js + Grafana を Bloom メトリクスと統合。
- **Real-time Communication**: Bloom から Prometheus Exporter → Grafana、サイドカーは OpenTelemetry で Bloom に push。
- **Visualization Libraries**: Grafana プラグイン、three.js ネットワークグラフ。
- **State Management**: TimescaleDB（Bloom テレメトリ）、Unity 側は ScriptableObject。

## Development Environment

### Build & Development Tools
- **Build System**: Unity ビルドパイプライン、cargo、docker-compose（Bloom テスト環境）。
- **Package Management**: UPM、cargo、npm（ダッシュボード）。
- **Development workflow**: Unity Play Mode + サイドカーを `cargo watch` でホットリロード、Bloom は docker-compose で再起動。

### Code Quality Tools
- **Static Analysis**: clippy、cargo-deny、Unity Analyzers。
- **Formatting**: rustfmt、dotnet-format、Prettier (Next.js)。
- **Testing Framework**: Rust は cargo test / tokio-test、Unity は PlayMode/Test Runner、Bloom は integration test（cargo）。
- **Documentation**: mdBook（ネットワーク仕様）、Doxygen（FFI）、OpenAPI（Bloom API）。

### Version Control & Collaboration
- **VCS**: Git (GitHub)。
- **Branching Strategy**: trunk-based + short-lived feature branch。
- **Code Review Process**: GitHub Pull Request、最低 1 名承認 + CI（Unity build、cargo test、lint）。

### Dashboard Development (if applicable)
- **Live Reload**: Next.js dev server (Bloom Dashboard)。
- **Port Management**: Bloom 443/8443、サイドカー制御ポート 47000（UDS/NPIPE）、Grafana 3000。
- **Multi-Instance Support**: docker-compose で Bloom シャーディング、ローカルで複数サイドカー起動可能。

## Deployment & Distribution (if applicable)
- **Target Platform(s)**: Windows PCVR (OpenXR) とデスクトップモード（同一 Unity ビルドの headless 表示モード）、Meta Quest は後続フェーズ。Bloom は Kubernetes or bare-metal Linux。
- **Distribution Method**: 自社配布ポータルまたは直接ダウンロードパッケージ（PC 用インストーラとサイドカー同梱）。
- **Installation Requirements**: Quest 版は後続検証、PC 版は Rust サイドカーを常駐サービスとしてインストール。
- **Update Mechanism**: Unity Addressables + Bloom からのマニフェスト、サイドカーは delta patch、自動再起動。Bloom は rolling update。

## Technical Requirements & Constraints

### Performance Requirements
- サイドカー経由の音声 RTT 120ms 以下、データチャンネル 30Hz 更新。
- Quest2 基準で 6 ピア / 10 分セッション時に CPU 合計 60% 以下（Quest 対応時を想定）。
- Bloom フェデレーション同期遅延 1 秒以内（同一リージョン）。

### Compatibility Requirements
- **Platform Support**: Windows 11 + OpenXR、Windows デスクトップモード、Bloom は Ubuntu 22.04 x86_64。
- **Dependency Versions**: iroh 0.15 以上、Unity 2022.3 LTS、Rust 1.81、Postgres 15。
- **Standards Compliance**: OpenXR 1.1、QUIC draft-34、OAuth2/OIDC。

### Security & Compliance
- **Security Requirements**: Bloom ↔ サイドカー間は mTLS、ピア間は Ed25519 と Noise パターン。UGC は Bloom で署名検証。
- **Compliance Standards**: GDPR 準拠（Bloom に個人特定情報を保存しない）。
- **Threat Model**: サイドカー乗っ取り、悪意ある Bloom ノード、Sybil 攻撃を想定。鍵ローテーションとフェデレーション認証を実施。

### Scalability & Reliability
- **Expected Load**: デイリー 200 セッション、初期運用は Bloom 1 インスタンスで開始し、必要に応じてフェデレーション拡張。
- **Availability Requirements**: Bloom SLA 99.5%、サイドカー自動再接続、P2P 失敗時は Bloom リレー（STAN 経由）にフォールバック。
- **Growth Projections**: 2026 年までに Bloom クラスタを地理フェデレーションし、20 ピア / セッションを支える SFU ハイブリッドを検討。

## Technical Decisions & Rationale

### Decision Log
1. **Unity ↔ Rust サイドカー IPC**: Unity 内で直接 iroh を埋め込むよりも、Rust プロセスを独立させた方がクラッシュ分離とホットアップデートが容易。FFI ではなく gRPC IPC を採用。
2. **Bloom フェデレーション導入**: 完全 P2P では UGC 配信と招待管理が負荷となるため、Bloom（シグナリング + STAN + UGC 管理）をハブとしてメタデータ管理とピア発見を集約しつつ、リアルタイム同期はピア間で完結させる二層構造を選択。
3. **MessagePack プロトコル**: JSON より低遅延かつバイナリ互換で、Unity/Rust 双方に成熟ライブラリが存在。ProtoBuf も検討したがスキーマ更新の柔軟性を優先し MessagePack を選択。

## Known Limitations
- **Bloom 単一運用**: 初期フェーズは Bloom 1 インスタンスのみで運用するため、リージョン障害時の冗長性が不足。フェデレーション拡張の検証が必要。
- **Quest サイドカー接続**: Android での UDS/NPIPE 代替として AIDL or gRPC over QUIC の検証が未完。
- **NAT 超え**: Bloom リレーの実装が暫定版で、企業ネットワークでは接続率が低下。フェデレーテッド中継ノードの設計が必要。
