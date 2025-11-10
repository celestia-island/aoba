<p align="center">
  <img src="./packages/tui/res/logo.png" alt="Aoba Logo" width="240" />
</p>

<p align="center">
  <h1 align="center">
    Aoba
  </h1>
</p>

<p align="center">
  <a href="https://github.com/celestia-island/aoba/actions/workflows/basic-checks.yml">
    <img src="https://github.com/celestia-island/aoba/actions/workflows/basic-checks.yml/badge.svg?branch=master" alt="Basic Checks status" />
  </a>
  <a href="https://github.com/celestia-island/aoba/actions/workflows/e2e-tests-tui.yml">
    <img src="https://github.com/celestia-island/aoba/actions/workflows/e2e-tests-tui.yml/badge.svg?branch=master" alt="E2E TUI status" />
  </a>
  <a href="https://github.com/celestia-island/aoba/actions/workflows/e2e-tests-cli.yml">
    <img src="https://github.com/celestia-island/aoba/actions/workflows/e2e-tests-cli.yml/badge.svg?branch=master" alt="E2E CLI status" />
  </a>
  <a href="https://github.com/celestia-island/aoba/blob/master/LICENSE">
    <img src="https://img.shields.io/github/license/celestia-island/aoba?color=blue" alt="License" />
  </a>
  <a href="https://github.com/celestia-island/aoba/releases/latest">
    <img src="https://img.shields.io/github/v/tag/celestia-island/aoba?label=version&sort=semver" alt="Latest Version" />
  </a>
</p>

<p align="center">
  EN | <a href="./README_zh.md">ZH</a>
</p>

Multi-protocol debugging and simulation CLI tool, supporting Modbus RTU, MQTT, TCP and more.

> Under active development

## Features

- Serial and network protocol debugging
- Protocol simulation (master/slave, client/server)
- Automatic TUI/GUI switching
- Create and manage virtual serial ports

## Quick start

1. Install the Rust toolchain
2. Install the tool: `cargo install aoba`
3. Run the tool: execute the installed `aoba` binary (or use your package manager's path)

Notes:

- Detailed documentation is still being written.
  - Examples and some reference material live in the `examples/` and `docs/` directories but are not yet comprehensive.
  - If you want to run automated tests or CI workflows, check `./scripts/` and the example test folders for guidance.

## Contribution

Contributions are welcome — please open issues or pull requests. See the repository for coding guidelines and CI configuration.
