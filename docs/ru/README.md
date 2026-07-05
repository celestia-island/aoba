<p align="center"><img src="https://raw.githubusercontent.com/celestia-island/aoba/master/docs/logo.webp" alt="aoba" width="240" /></p>

<h1 align="center">Aoba</h1>

<p align="center"><strong>Многофункциональный инструмент CLI/TUI для отладки и симуляции Modbus RTU</strong></p>

<div align="center">

[![Checks](https://github.com/celestia-island/aoba/actions/workflows/checks.yml/badge.svg)](https://github.com/celestia-island/aoba/actions/workflows/checks.yml)
[![E2E TUI](https://github.com/celestia-island/aoba/actions/workflows/e2e-tests-tui.yml/badge.svg)](https://github.com/celestia-island/aoba/actions/workflows/e2e-tests-tui.yml)
[![E2E CLI](https://github.com/celestia-island/aoba/actions/workflows/e2e-tests-cli.yml/badge.svg)](https://github.com/celestia-island/aoba/actions/workflows/e2e-tests-cli.yml)
[![License: SySL](https://img.shields.io/badge/license-SySL%201.0-blue)](../../LICENSE)
[![Version](https://img.shields.io/github/v/tag/celestia-island/aoba?label=version&sort=semver)](https://github.com/celestia-island/aoba/releases/latest)

</div>

<div align="center">

[English](../en/README.md) ·
[简体中文](../zhs/README.md) ·
[繁體中文](../zht/README.md) ·
[日本語](../ja/README.md) ·
[한국어](../ko/README.md) ·
[Français](../fr/README.md) ·
[Español](../es/README.md) ·
**Русский** ·
[العربية](../ar/README.md)

</div>

Многофункциональный инструмент отладки и симуляции Modbus RTU, подходящий как для физических последовательных портов, так и для перенаправляемых через сеть. Предоставляет интерфейсы CLI и TUI.

## Возможности

- Отладка и симуляция Modbus RTU (ведущий/ведомый); поддержка четырех типов регистров: holding, input, coils и discrete.
- Полнофункциональный CLI: обнаружение и проверка портов (`--list-ports` / `--check-port`), операции ведущий/ведомый (`--master-provide` / `--slave-listen`) и постоянные режимы (`--*-persist`). Вывод в JSON/JSONL, удобный для скриптов и CI.
- Интерактивный TUI: настройка портов, станций и регистров через терминальный интерфейс; поддержка сохранения/загрузки (`Ctrl+S` сохраняет и автоматически активирует порты) и интеграция IPC с CLI для тестирования и автоматизации.
- Множество источников данных и протоколов: физические/виртуальные последовательные порты (управление через `socat`), HTTP, MQTT, IPC (Unix-сокеты / именованные каналы), файлы и FIFO.
- Пересылка портов: настройка исходных и целевых портов в TUI для репликации, мониторинга или мостового соединения данных.
- Режим демона: фоновый запуск с сохраненной конфигурацией TUI для запуска всех настроенных портов/станций (подходит для встроенных/CI развертываний).
- Инструменты виртуальных портов и тестирования: включает `scripts/socat_init.sh` для виртуальных последовательных портов и примеры тестов в `examples/cli_e2e` и `examples/tui_e2e`.

> Примечание: используйте `--no-config-cache` для отключения сохранения/загрузки TUI; `--config-file <FILE>` и `--no-config-cache` взаимно исключают друг друга.

## Быстрый старт

1. Установите инструментарий Rust
2. Клонируйте репозиторий и перейдите в каталог
3. Установка:
   - Сборка из исходников: `cargo install aoba`
   - Или установка CI-собранного релиза (если доступен) с помощью `cargo-binstall`:
     - Пример: `cargo binstall --manifest-path ./Cargo.toml --version <version>`
     - Используйте `--target <triple>` для выбора платформы (например, `x86_64-unknown-linux-gnu`)
4. Запустите `aoba` для запуска TUI по умолчанию

## Документация

- [API Modbus Master](API_MODBUS_MASTER.md)
- [API Modbus Slave](API_MODBUS_SLAVE.md)
- [CLI Modbus](CLI_MODBUS.md)
- [Источник данных HTTP](DATA_SOURCE_HTTP.md)
- [Источник данных IPC](DATA_SOURCE_IPC.md)
- [Источник данных MQTT](DATA_SOURCE_MQTT.md)
- [Пересылка портов](DATA_SOURCE_PORT_FORWARDING.md)

## Языки

| Язык | Ссылка |
|------|--------|
| العربية | [AR](../ar/README.md) |
| English | [EN](../en/README.md) |
| Espanol | [ES](../es/README.md) |
| Francais | [FR](../fr/README.md) |
| 日本語 | [JA](../ja/README.md) |
| 한국어 | [KO](../ko/README.md) |
| Русский | [RU](README.md) |
| 简体中文 | [ZH](../zhs/README.md) |
| 繁體中文 | [ZHT](../zht/README.md) |

## Лицензия

Лицензировано под [Synthetic Source License (SySL), Version 1.0](../../LICENSE).
