<p align="center">
  <img src="./res/logo.png" alt="Aoba Logo" width="240" />
</p>

<p align="center">
  <h1 align="center">
    Aoba
  </h1>
</p>

<p align="center">
  <a href="https://github.com/celestia-island/aoba/actions/workflows/basic-checks.yml">
    <img src="https://img.shields.io/github/actions/workflow/status/celestia-island/aoba/basic-checks.yml?branch=master&label=Basic%20Checks&logo=github" alt="Basic Checks Status" />
  </a>
  <a href="https://github.com/celestia-island/aoba/actions/workflows/e2e-tests-cli.yml">
    <img src="https://img.shields.io/github/actions/workflow/status/celestia-island/aoba/e2e-tests-cli.yml?branch=master&label=CLI%20E2E&logo=github" alt="CLI E2E Status" />
  </a>
  <a href="https://github.com/celestia-island/aoba/actions/workflows/e2e-tests-tui.yml">
    <img src="https://img.shields.io/github/actions/workflow/status/celestia-island/aoba/e2e-tests-tui.yml?branch=master&label=TUI%20E2E&logo=github" alt="TUI E2E Status" />
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

Multi-protocol debugging and simulation CLI tool, supporting Modbus RTU, MQTT, TCP, etc.

> Still developing

## Features

- Serial/network protocol debugging
- Protocol simulation (master/slave, client/server)
- TUI/GUI auto switch
- Create virtual serial ports

## Quick Start

1. Install Rust toolchain
2. `cargo build --bins`
3. `cargo run` or run the generated executable

## Testing

Aoba includes comprehensive E2E testing for both CLI and TUI components using an IPC-based architecture.

### Running Tests Locally

Use the provided CI script to run tests locally:

```bash
# Run all tests (CLI + TUI)
./scripts/run_ci_locally.sh --workflow all

# Run only TUI tests
./scripts/run_ci_locally.sh --workflow tui-rendering    # Fast UI tests
./scripts/run_ci_locally.sh --workflow tui-drilldown   # Full integration tests

# Run only CLI tests
./scripts/run_ci_locally.sh --workflow cli

# Run specific test module
./scripts/run_ci_locally.sh --workflow tui-drilldown --module single_station_master_coils
```

### Test Architecture

- **TUI E2E Tests** (`examples/tui_e2e`): IPC-based testing using Unix domain sockets
  - Screen Capture mode: Fast UI regression testing without process spawning
  - DrillDown mode: Full integration testing with real TUI process
  - No terminal emulation dependencies (expectrl/vt100 removed)
  
- **CLI E2E Tests** (`examples/cli_e2e`): Modbus protocol testing with virtual serial ports
  - Single and multi-station configurations
  - All register types (Coils, Discrete Inputs, Holding, Input)
  - Master/Slave communication validation

For more details, see:
- [TUI E2E Testing Documentation](examples/tui_e2e/README.md)
- [IPC Architecture Details](examples/tui_e2e/IPC_ARCHITECTURE.md)
- [IPC Mode Documentation](docs/IPC_MODE.md)

## Contribution

Feel free to submit issues or PRs!
