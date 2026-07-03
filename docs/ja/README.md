<p align="center"><img src="https://raw.githubusercontent.com/celestia-island/aoba/master/docs/logo.webp" alt="aoba" width="240" /></p>

<h1 align="center">Aoba</h1>

<p align="center"><strong>Modbus RTU 向けマルチプロトコル・デバッグ/シミュレーション CLI/TUI ツール</strong></p>

<div align="center">

[![Checks](https://github.com/celestia-island/aoba/actions/workflows/checks.yml/badge.svg?branch=master)](https://github.com/celestia-island/aoba/actions/workflows/checks.yml)
[![E2E TUI](https://github.com/celestia-island/aoba/actions/workflows/e2e-tests-tui.yml/badge.svg?branch=master)](https://github.com/celestia-island/aoba/actions/workflows/e2e-tests-tui.yml)
[![E2E CLI](https://github.com/celestia-island/aoba/actions/workflows/e2e-tests-cli.yml/badge.svg?branch=master)](https://github.com/celestia-island/aoba/actions/workflows/e2e-tests-cli.yml)
[![License: SySL](https://img.shields.io/badge/license-SySL%201.0-blue)](../../LICENSE)
[![Version](https://img.shields.io/github/v/tag/celestia-island/aoba?label=version&sort=semver)](https://github.com/celestia-island/aoba/releases/latest)

</div>

<div align="center">

[English](../en/README.md) ·
[简体中文](../zhs/README.md) ·
[繁體中文](../zht/README.md) ·
**日本語** ·
[한국어](../ko/README.md) ·
[Français](../fr/README.md) ·
[Español](../es/README.md) ·
[Русский](../ru/README.md) ·
[العربية](../ar/README.md)

</div>

Modbus RTU 向けマルチプロトコルデバッグ・シミュレーションツール。物理シリアルポートとネットワーク転送ポートの両方に対応。CLI と TUI の両インターフェースを提供。

## 機能

- Modbus RTU (マスター/スレーブ) のデバッグとシミュレーション；holding、input、coils、discrete の4種類のレジスタに対応。
- フル機能の CLI：ポート検出とチェック (`--list-ports` / `--check-port`)、マスター/スレーブ操作 (`--master-provide` / `--slave-listen`) および永続モード (`--*-persist`)。出力は JSON/JSONL 形式でスクリプトや CI に適応。
- インタラクティブ TUI：ターミナル UI でポート、ステーション、レジスタを設定可能；保存/読み込み対応 (`Ctrl+S` で保存し自動的にポートを有効化)、CLI との IPC 連携によりテストと自動化が可能。
- 複数のデータソースとプロトコル：物理/仮想シリアルポート (`socat` で管理)、HTTP、MQTT、IPC (Unix ドメインソケット / 名前付きパイプ)、ファイル、FIFO。
- ポート転送：TUI 内で送信元ポートと送信先ポートを設定し、データ複製、監視、ブリッジングが可能。
- デーモンモード：保存した TUI 設定を使用してヘッドレス実行し、設定済みの全ポート/ステーションを起動 (組み込み/CI デプロイに最適)。
- 仮想ポートとテストツール：仮想シリアルポート用の `scripts/socat_init.sh` と、`examples/cli_e2e` および `examples/tui_e2e` のサンプルテストを含む。

> 注意：`--no-config-cache` で TUI の保存/読み込みを無効化；`--config-file <FILE>` と `--no-config-cache` は同時に使用できません。

## クイックスタート

1. Rust ツールチェーンをインストール
2. リポジトリをクローンしてディレクトリに移動
3. インストール：
   - ソースからビルド：`cargo install aoba`
   - または CI ビルド済みリリース (利用可能な場合) を `cargo-binstall` でインストール：
     - 例：`cargo binstall --manifest-path ./Cargo.toml --version <version>`
     - `--target <triple>` でプラットフォームを指定 (例：`x86_64-unknown-linux-gnu`)
4. `aoba` を実行してデフォルトで TUI を起動

## ドキュメンテーション

- [API Modbus Master](API_MODBUS_MASTER.md)
- [API Modbus Slave](API_MODBUS_SLAVE.md)
- [CLI Modbus](CLI_MODBUS.md)
- [HTTP データソース](DATA_SOURCE_HTTP.md)
- [IPC データソース](DATA_SOURCE_IPC.md)
- [MQTT データソース](DATA_SOURCE_MQTT.md)
- [ポート転送](DATA_SOURCE_PORT_FORWARDING.md)

## 言語

| 言語 | リンク |
|------|--------|
| العربية | [AR](../ar/README.md) |
| English | [EN](../en/README.md) |
| Espanol | [ES](../es/README.md) |
| Francais | [FR](../fr/README.md) |
| 日本語 | [JA](README.md) |
| 한국어 | [KO](../ko/README.md) |
| Русский | [RU](../ru/README.md) |
| 简体中文 | [ZH](../zhs/README.md) |
| 繁體中文 | [ZHT](../zht/README.md) |

## ライセンス

[Synthetic Source License (SySL), Version 1.0](../../LICENSE) の下でライセンスされています。
