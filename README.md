<p align="center">
  <img src="./packages/tui/res/logo.png" alt="Aoba Logo" width="240" />
</p>

<p align="center">
  <h1 align="center">
    Aoba
  </h1>
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
- **Daemon mode**: non-interactive background operation with auto-configuration loading

## Quick start

1. Install the Rust toolchain
2. Install the tool: `cargo install aoba`
3. Run the tool: execute the installed `aoba` binary (or use your package manager's path)

Notes:

- Detailed documentation is still being written.
  - Examples and some reference material live in the `examples/` and `docs/` directories but are not yet comprehensive.
  - If you want to run automated tests or CI workflows, check `./scripts/` and the example test folders for guidance.

## Daemon Mode

Daemon mode allows `aoba` to run in non-interactive environments, perfect for scenarios requiring TUI configuration features (like transparent port forwarding) without the interactive interface.

### Usage

```bash
# Use default config file (aoba_tui_config.json in current directory)
aoba --daemon

# Or use short form
aoba -d

# Specify custom config file path
aoba --daemon --daemon-config /path/to/config.json

# Specify log file (outputs to both terminal and file)
aoba --daemon --log-file /path/to/daemon.log
```

### How It Works

1. **Configuration Loading**: Loads TUI config from working directory or specified path
2. **Auto-Start**: Automatically starts all configured ports and stations
3. **Dual Logging**: Outputs logs to both terminal and file simultaneously
4. **No UI**: Runs core threads only, no interactive interface

### Preparing Configuration

First use TUI mode to create and configure ports:

```bash
# Start TUI for configuration
aoba --tui

# In TUI:
# 1. Configure ports and Modbus stations
# 2. Press Ctrl+S to save configuration
# 3. Exit TUI
```

Configuration is automatically saved to `./aoba_tui_config.json`.

### Error Handling

If the configuration file doesn't exist, daemon mode will exit with an error:

```
Error: Configuration file not found: ./aoba_tui_config.json

Daemon mode requires a configuration file. You can:
1. Run TUI mode first to create and save a configuration
2. Specify a custom config path with --daemon-config <FILE>
```

### Typical Use Cases

- **Transparent Port Forwarding**: Run forwarding services in the background
- **Automated Testing**: Auto-start Modbus simulators in CI/CD environments
- **Remote Deployment**: Run Modbus services on headless servers

## Contribution

Contributions are welcome — please open issues or pull requests. See the repository for coding guidelines and CI configuration.
