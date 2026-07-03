<p align="center"><img src="https://raw.githubusercontent.com/celestia-island/aoba/master/docs/logo.webp" alt="aoba" width="240" /></p>

<h1 align="center">Aoba</h1>

<p align="center"><strong>Modbus RTU를 위한 멀티 프로토콜 디버깅 및 시뮬레이션 CLI/TUI 도구</strong></p>

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
[日本語](../ja/README.md) ·
**한국어** ·
[Français](../fr/README.md) ·
[Español](../es/README.md) ·
[Русский](../ru/README.md) ·
[العربية](../ar/README.md)

</div>

Modbus RTU를 위한 멀티 프로토콜 디버깅 및 시뮬레이션 도구로, 물리적 직렬 포트와 네트워크 전달 포트 모두에 적합합니다. CLI 및 TUI 인터페이스를 제공합니다.

## 기능

- Modbus RTU (마스터/슬레이브) 디버깅 및 시뮬레이션; holding, input, coils, discrete 네 가지 레지스터 유형 지원.
- 풀 기능 CLI: 포트 검색 및 확인 (`--list-ports` / `--check-port`), 마스터/슬레이브 작업 (`--master-provide` / `--slave-listen`) 및 지속 모드 (`--*-persist`). JSON/JSONL 출력으로 스크립트 및 CI 친화적.
- 대화형 TUI: 터미널 UI를 통해 포트, 스테이션, 레지스터 구성; 저장/로드 지원 (`Ctrl+S`로 저장 및 포트 자동 활성화), CLI와 IPC 통합으로 테스트 및 자동화.
- 다양한 데이터 소스 및 프로토콜: 물리적/가상 직렬 포트 (`socat`으로 관리), HTTP, MQTT, IPC (Unix 도메인 소켓 / 네임드 파이프), 파일, FIFO.
- 포트 전달: TUI 내에서 소스 및 대상 포트를 구성하여 데이터 복제, 모니터링 또는 브리징.
- 데몬 모드: 저장된 TUI 구성을 사용하여 헤드리스로 실행, 구성된 모든 포트/스테이션 시작 (임베디드/CI 배포에 적합).
- 가상 포트 및 테스트 도구: 가상 직렬 포트용 `scripts/socat_init.sh` 및 `examples/cli_e2e`, `examples/tui_e2e`의 예제 테스트 포함.

> 참고: `--no-config-cache`로 TUI 저장/로드 비활성화; `--config-file <FILE>`과 `--no-config-cache`는 상호 배타적입니다.

## 빠른 시작

1. Rust 툴체인 설치
2. 저장소 클론 및 디렉토리 진입
3. 설치:
   - 소스에서 빌드: `cargo install aoba`
   - 또는 CI 빌드 릴리스 (사용 가능한 경우)를 `cargo-binstall`로 설치:
     - 예: `cargo binstall --manifest-path ./Cargo.toml --version <version>`
     - `--target <triple>`로 플랫폼별 아티팩트 선택 (예: `x86_64-unknown-linux-gnu`)
4. `aoba` 실행으로 기본 TUI 시작

## 문서

- [API Modbus Master](API_MODBUS_MASTER.md)
- [API Modbus Slave](API_MODBUS_SLAVE.md)
- [CLI Modbus](CLI_MODBUS.md)
- [HTTP 데이터 소스](DATA_SOURCE_HTTP.md)
- [IPC 데이터 소스](DATA_SOURCE_IPC.md)
- [MQTT 데이터 소스](DATA_SOURCE_MQTT.md)
- [포트 전달](DATA_SOURCE_PORT_FORWARDING.md)

## 언어

| 언어 | 링크 |
|------|------|
| العربية | [AR](../ar/README.md) |
| English | [EN](../en/README.md) |
| Espanol | [ES](../es/README.md) |
| Francais | [FR](../fr/README.md) |
| 日本語 | [JA](../ja/README.md) |
| 한국어 | [KO](README.md) |
| Русский | [RU](../ru/README.md) |
| 简体中文 | [ZH](../zhs/README.md) |
| 繁體中文 | [ZHT](../zht/README.md) |

## 라이선스

[Synthetic Source License (SySL), Version 1.0](../../LICENSE)에 따라 라이선스가 부여됩니다.
