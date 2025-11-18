<p align="center">
  <img src="./packages/tui/res/logo.png" alt="Aoba Logo" width="240" />
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

Multi-protocol debugging and simulation tool for Modbus RTU, suitable for both physical serial ports and network-forwarded ports. Provides both CLI and TUI interfaces.

## Features

- Modbus RTU (master/slave) debugging and simulation; supports four register types: holding, input, coils, and discrete.
- Full-featured CLI: port discovery and checks (`--list-ports` / `--check-port`), master/slave operations (`--master-provide` / `--slave-listen`) and persistent modes (`--*-persist`). Outputs can be JSON/JSONL, which is script/CI-friendly.
- Interactive TUI: configure ports, stations, and registers via terminal UI; supports save/load (`Ctrl+S` saves and auto-enables ports) and IPC integration with CLI for testing and automation.
- Multiple data sources and protocols: physical/virtual serial ports (managed via `socat`), HTTP, MQTT, IPC (Unix domain sockets / named pipes), files, and FIFOs.
- Port Forwarding: configure source and target ports within the TUI for data replication, monitoring, or bridging.
- Daemon mode: run headless using a saved TUI configuration to start all configured ports/stations (suitable for embedded/CI deployments).
- Virtual port and test tooling: includes `scripts/socat_init.sh` for virtual serial ports and example tests in `examples/cli_e2e` and `examples/tui_e2e` for local/CI testing.
- Extensible integrations: forward or receive port data via HTTP/MQTT/IPC for (remote) integrations.

> Note: use `--no-config-cache` to disable TUI save/load; `--config-file <FILE>` and `--no-config-cache` are mutually exclusive.

## Quick start

1. Install the Rust toolchain
2. Clone the repo and enter the directory
3. Install: `cargo install --path .` or run via `cargo run`
4. Run `aoba` to start the TUI by default; use TUI to configure ports and save the configuration as needed.

## Persistent configuration (config file)

`--config-file <FILE>` explicitly selects a TUI config file (daemon mode uses `--daemon-config <FILE>`). This conflicts with `--no-config-cache`, which disables loading/saving of TUI config.

Example:

```bash
# Start TUI with a specific config file; load/save enabled
aoba --tui --config-file /path/to/config.json

# Start TUI with no config caching (default) — no load/save
aoba --tui --no-config-cache
```

Run headless with a saved configuration:

```bash
aoba --daemon --config-file /path/to/config.json
```

Systemd example:

```ini
[Unit]
Description=Aoba Modbus RTU Daemon
Wants=network.target
After=network.target network-service
StartLimitIntervalSec=0

[Service]
Type=simple
WorkingDirectory=/home/youruser
ExecStart=/usr/local/bin/aoba --daemon --config-file /home/youruser/config.json
Restart=always
RestartSec=1s

[Install]
WantedBy=multi-user.target
```

## Common use cases

- Automated testing: auto-start Modbus simulators in CI/CD
- Embedded systems: run Aoba as a daemon on embedded devices (e.g., Raspberry Pi) with USB-serial adapters
