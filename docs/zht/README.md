<p align="center"><img src="https://raw.githubusercontent.com/celestia-island/aoba/master/docs/logo.webp" alt="aoba" width="240" /></p>

<h1 align="center">Aoba</h1>

<p align="center"><strong>面向 Modbus RTU 的多協定偵錯與模擬 CLI/TUI 工具</strong></p>

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
**繁體中文** ·
[日本語](../ja/README.md) ·
[한국어](../ko/README.md) ·
[Français](../fr/README.md) ·
[Español](../es/README.md) ·
[Русский](../ru/README.md) ·
[العربية](../ar/README.md)

</div>

Modbus RTU 多協定偵錯與模擬工具，適用於實體序列埠與網路轉發埠。提供 CLI 與 TUI 雙介面。

## 功能

- Modbus RTU（主/從）偵錯與模擬；支援四種暫存器類型：holding、input、coils 與 discrete。
- 全功能 CLI：連接埠偵測與檢查（`--list-ports` / `--check-port`）、主/從操作（`--master-provide` / `--slave-listen`）及持續模式（`--*-persist`）。輸出可為 JSON/JSONL，適用於腳本與 CI。
- 互動式 TUI：透過終端機介面設定連接埠、站台與暫存器；支援儲存/載入（`Ctrl+S` 儲存並自動啟用連接埠），並可透過 IPC 與 CLI 整合進行測試與自動化。
- 多種資料來源與協定：實體/虛擬序列埠（透過 `socat` 管理）、HTTP、MQTT、IPC（Unix 域通訊端 / 命名管道）、檔案與 FIFO。
- 連接埠轉發：在 TUI 內設定來源與目標連接埠，進行資料複製、監控或橋接。
- 守護進程模式：使用已儲存的 TUI 設定以無頭模式執行，啟動所有已設定的連接埠/站台（適用於嵌入式/CI 部署）。
- 虛擬連接埠與測試工具：包含用於虛擬序列埠的 `scripts/socat_init.sh`，以及 `examples/cli_e2e` 與 `examples/tui_e2e` 中的範例測試。

> 注意：使用 `--no-config-cache` 停用 TUI 儲存/載入；`--config-file <FILE>` 與 `--no-config-cache` 互斥。

## 快速開始

1. 安裝 Rust 工具鏈
2. 複製儲存庫並進入目錄
3. 安裝：
   - 從原始碼建置：`cargo install aoba`
   - 或使用 `cargo-binstall` 安裝 CI 預建置版本（如有提供）：
     - 範例：`cargo binstall --manifest-path ./Cargo.toml --version <version>`
     - 使用 `--target <triple>` 指定目標平台（例如 `x86_64-unknown-linux-gnu`）
4. 執行 `aoba` 以預設啟動 TUI

## 文件

- [API Modbus Master](API_MODBUS_MASTER.md)
- [API Modbus Slave](API_MODBUS_SLAVE.md)
- [CLI Modbus](CLI_MODBUS.md)
- [HTTP 資料來源](DATA_SOURCE_HTTP.md)
- [IPC 資料來源](DATA_SOURCE_IPC.md)
- [MQTT 資料來源](DATA_SOURCE_MQTT.md)
- [連接埠轉發](DATA_SOURCE_PORT_FORWARDING.md)

## 語言

| 語言 | 連結 |
|------|------|
| العربية | [AR](../ar/README.md) |
| English | [EN](../en/README.md) |
| Espanol | [ES](../es/README.md) |
| Francais | [FR](../fr/README.md) |
| 日本語 | [JA](../ja/README.md) |
| 한국어 | [KO](../ko/README.md) |
| Русский | [RU](../ru/README.md) |
| 简体中文 | [ZH](../zhs/README.md) |
| 繁體中文 | [ZHT](README.md) |

## 授權條款

依據 [Synthetic Source License (SySL), Version 1.0](../../LICENSE) 授權。
