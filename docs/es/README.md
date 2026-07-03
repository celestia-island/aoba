<p align="center">
  <img src="../../res/logo.webp" alt="Aoba Logo" width="240" />
</p>

<p align="center">
  <h1 align="center">Aoba</h1>
</p>

<p align="center">
  <a href="https://github.com/celestia-island/aoba/actions/workflows/checks.yml">
    <img src="https://github.com/celestia-island/aoba/actions/workflows/checks.yml/badge.svg?branch=master" alt="Checks status" />
  </a>
  <a href="https://github.com/celestia-island/aoba/actions/workflows/e2e-tests-tui.yml">
    <img src="https://github.com/celestia-island/aoba/actions/workflows/e2e-tests-tui.yml/badge.svg?branch=master" alt="E2E TUI status" />
  </a>
  <a href="https://github.com/celestia-island/aoba/actions/workflows/e2e-tests-cli.yml">
    <img src="https://github.com/celestia-island/aoba/actions/workflows/e2e-tests-cli.yml/badge.svg?branch=master" alt="E2E CLI status" />
  </a>
  <a href="../../LICENSE">
    <img src="https://img.shields.io/badge/license-SySL%201.0-blue" alt="License: SySL" />
  </a>
  <a href="https://github.com/celestia-island/aoba/releases/latest">
    <img src="https://img.shields.io/github/v/tag/celestia-island/aoba?label=version&sort=semver" alt="Latest Version" />
  </a>
</p>

<p align="center">
  <a href="../ar/README.md">AR</a> | <a href="../en/README.md">EN</a> | ES | <a href="../fr/README.md">FR</a> | <a href="../ja/README.md">JA</a> | <a href="../ko/README.md">KO</a> | <a href="../ru/README.md">RU</a> | <a href="../zhs/README.md">ZH</a> | <a href="../zht/README.md">ZHT</a>
</p>

Herramienta de depuracion y simulacion multiprotocolo para Modbus RTU, compatible con puertos serie fisicos y puertos reenviados por red. Proporciona interfaces CLI y TUI.

## Caracteristicas

- Depuracion y simulacion de Modbus RTU (maestro/esclavo); compatible con cuatro tipos de registros: holding, input, coils y discrete.
- CLI completo: descubrimiento y verificacion de puertos (`--list-ports` / `--check-port`), operaciones maestro/esclavo (`--master-provide` / `--slave-listen`) y modos persistentes (`--*-persist`). Salidas en JSON/JSONL aptas para scripts y CI.
- TUI interactivo: configuracion de puertos, estaciones y registros mediante interfaz de terminal; soporte para guardar/cargar (`Ctrl+S` guarda y activa puertos automaticamente) e integracion IPC con CLI para pruebas y automatizacion.
- Multiples fuentes de datos y protocolos: puertos serie fisicos/virtuales (gestionados via `socat`), HTTP, MQTT, IPC (sockets Unix / tuberias con nombre), archivos y FIFOs.
- Reenvio de puertos: configuracion de puertos de origen y destino dentro del TUI para replicacion de datos, monitoreo o puenteo.
- Modo demonio: ejecucion sin interfaz usando una configuracion TUI guardada para iniciar todos los puertos/estaciones configurados (apto para despliegues embebidos/CI).
- Herramientas de puerto virtual y pruebas: incluye `scripts/socat_init.sh` para puertos serie virtuales y pruebas de ejemplo en `examples/cli_e2e` y `examples/tui_e2e`.

> Nota: use `--no-config-cache` para deshabilitar guardar/cargar en TUI; `--config-file <FILE>` y `--no-config-cache` son mutuamente excluyentes.

## Inicio rapido

1. Instalar Rust toolchain
2. Clonar el repositorio y entrar al directorio
3. Instalar:
   - Construir desde fuente: `cargo install aoba`
   - O instalar una version precompilada por CI (si esta disponible) con `cargo-binstall`:
     - Ejemplo: `cargo binstall --manifest-path ./Cargo.toml --version <version>`
     - Use `--target <triple>` para elegir una plataforma especifica (ej. `x86_64-unknown-linux-gnu`)
4. Ejecutar `aoba` para iniciar el TUI por defecto

## Documentacion

- [API Modbus Master](API_MODBUS_MASTER.md)
- [API Modbus Slave](API_MODBUS_SLAVE.md)
- [CLI Modbus](CLI_MODBUS.md)
- [Fuente de datos HTTP](DATA_SOURCE_HTTP.md)
- [Fuente de datos IPC](DATA_SOURCE_IPC.md)
- [Fuente de datos MQTT](DATA_SOURCE_MQTT.md)
- [Reenvio de puertos](DATA_SOURCE_PORT_FORWARDING.md)

## Idiomas

| Idioma | Enlace |
|--------|--------|
| العربية | [AR](../ar/README.md) |
| English | [EN](../en/README.md) |
| Espanol | [ES](README.md) |
| Francais | [FR](../fr/README.md) |
| 日本語 | [JA](../ja/README.md) |
| 한국어 | [KO](../ko/README.md) |
| Русский | [RU](../ru/README.md) |
| 简体中文 | [ZH](../zhs/README.md) |
| 繁體中文 | [ZHT](../zht/README.md) |

## Licencia

Licenciado bajo [Synthetic Source License (SySL), Version 1.0](../../LICENSE).
