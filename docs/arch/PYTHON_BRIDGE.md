# VERS SYSTEM: Python Bridge Architecture

VERS SYSTEM は、Rust の堅牢性と Python の豊かな AI エコシステムを融合させるために、**Python ブリッジ・プラグイン** 方式を採用します。これにより、Kernel の最小性を保ちつつ、PyTorch や TensorFlow といったネイティブ性能を要求する AI ライブラリをシームレスに利用可能にします。

---

## 1. 概要 (Overview)
Python ブリッジは、VERS 公式 SDK でビルドされた Rust 製プラグインであり、内部に Python インタプリタを内包します。これは「通訳者」として機能し、Kernel からのイベントを Python スクリプトへ中継します。

## 2. アーキテクチャ (Architecture)

### 階層構造
1.  **VERS Kernel**: システムの核。イベントのルーティングとプラグイン管理を担当。
2.  **Python Bridge (Rust)**: 公式 SDK 製プラグイン。`Magic Seal` による認証をパスし、Kernel から直接ロードされる。
3.  **Python Interpreter (PyO3)**: ブリッジ内部で稼働するランタイム。ホストシステムの Python 環境とライブラリを利用。
4.  **User Script (.py)**: 開発者が記述するロジック。コンパイル不要で即時実行可能。

### データの流れ
- **Input**: Kernel -> Bridge (Rust) -> JSON Serialization -> Python Function
- **Output**: Python Return -> String/JSON -> Bridge (Rust) -> Kernel

---

## 3. 主要なメリット (Key Benefits)

### ① ネイティブ性能の活用 (Native Performance)
WASM サンドボックスの制限を受けず、ホストマシンの GPU (CUDA/MPS) をフル活用した PyTorch/TensorFlow の推論が可能です。

### ② コンパイル不要の開発体験 (Zero-Compile DevEx)
ロジックの変更は `.py` ファイルの保存のみで完了します。プロトタイピングや頻繁な調整が必要な AI プロンプトエンジニアリングに最適です。

### ③ 資産の継承 (Ecosystem Integration)
LangChain, LlamaIndex, OpenAI Python SDK など、既存の膨大な Python ライブラリをそのまま VERS プラグインとして利用できます。

---

## 4. セキュリティとガバナンス (Security & Governance)

- **パスポート検査**: ブリッジ本体は公式 SDK でビルドされている必要があるため、非公式なバイナリが Kernel に直接介入することを防ぎます。
- **権限の委譲**: Python スクリプトが要求する権限（ネットワーク、ファイルアクセス等）は、ブリッジプラグインのマニフェストを通じて Kernel が管理・認可します。
- **隔離環境**: 可能であれば、Python 側の実行も仮想環境 (venv) やサンドボックスに限定し、ホストシステムへの影響を最小化します。

---

## 5. 実装指針 (Implementation Guidelines)

- **データ交換**: 複雑な型定義の共有を避けるため、データのやり取りは基本的に JSON 文字列を介して行います。
- **ライフサイクル**: Python インタプリタの起動オーバーヘッドを避けるため、プラグインの `on_plugin_init` 時にインタプリタを初期化し、実行時はモジュールをキャッシュして再利用します。
- **エラーハンドリング**: Python 側での例外 (Exception) はブリッジがキャッチし、Rust 側の `anyhow::Error` に変換して Kernel に報告しなければなりません。
