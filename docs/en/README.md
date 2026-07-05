<p align="center"><img src="https://raw.githubusercontent.com/celestia-island/aoba/master/docs/logo.webp" alt="aoba" width="240" /></p>

<h1 align="center">Aoba</h1>

<p align="center"><strong>Multi-protocol debugging and simulation CLI/TUI tool for Modbus RTU</strong></p>

<div align="center">

[![Checks](https://github.com/celestia-island/aoba/actions/workflows/checks.yml/badge.svg)](https://github.com/celestia-island/aoba/actions/workflows/checks.yml)
[![E2E TUI](https://github.com/celestia-island/aoba/actions/workflows/e2e-tests-tui.yml/badge.svg)](https://github.com/celestia-island/aoba/actions/workflows/e2e-tests-tui.yml)
[![E2E CLI](https://github.com/celestia-island/aoba/actions/workflows/e2e-tests-cli.yml/badge.svg)](https://github.com/celestia-island/aoba/actions/workflows/e2e-tests-cli.yml)
[![License: SySL](https://img.shields.io/badge/license-SySL%201.0-blue)](./LICENSE)
[![Version](https://img.shields.io/github/v/tag/celestia-island/aoba?label=version&sort=semver)](https://github.com/celestia-island/aoba/releases/latest)

</div>

<div align="center">

**English** ·
[简体中文](./docs/zhs/README.md) ·
[繁體中文](./docs/zht/README.md) ·
[日本語](./docs/ja/README.md) ·
[한국어](./docs/ko/README.md) ·
[Français](./docs/fr/README.md) ·
[Español](./docs/es/README.md) ·
[Русский](./docs/ru/README.md) ·
[العربية](./docs/ar/README.md)

</div>

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

3. Install:

   - Build from source: `cargo install aoba`

   - Or install a CI-built release (if available) with `cargo-binstall`:

     - Example: `cargo binstall --manifest-path ./Cargo.toml --version <version>`

     - Use `--target <triple>` to pick a platform-specific artifact (e.g. `x86_64-unknown-linux-gnu`).

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

## Programmatic API

Aoba provides a trait-based Rust API for embedding Modbus functionality in your applications. The API supports both master (client) and slave (server) roles with customizable hooks and data sources.

### Quick API Examples

**Modbus Master (polling a slave):**

```rust
use aoba::api::modbus::{ModbusBuilder, RegisterMode};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create and start a master that polls a slave
    let master = ModbusBuilder::new_master(1)
        .with_port("/dev/ttyUSB0")
        .with_register(RegisterMode::Holding, 0, 10)
        .build_master()?;

    // Receive responses via iterator interface
    while let Some(response) = master.recv_timeout(std::time::Duration::from_secs(2)) {
        println!("Received: {:?}", response.values);
    }
    Ok(())
}
```

**Modbus Slave (responding to requests):**

```rust
use aoba::api::modbus::{ModbusBuilder, RegisterMode};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create and start a slave that responds to master requests
    let slave = ModbusBuilder::new_slave(1)
        .with_port("/dev/ttyUSB0")
        .with_register(RegisterMode::Holding, 0, 10)
        .build_slave()?;

    // Receive request notifications via iterator interface
    while let Some(notification) = slave.recv_timeout(std::time::Duration::from_secs(10)) {
        println!("Processed request: {:?}", notification.values);
    }
    Ok(())
}
```

**Manual Mode (write operations + single-shot polling):**

```rust
use aoba::api::modbus::{ModbusBuilder, RegisterMode};

fn main() -> anyhow::Result<()> {
    let master = ModbusBuilder::new_master(1)
        .with_port("/dev/ttyUSB0")
        .with_timeout(5000)
        .build_master_manual()?;

    // Single-shot poll
    let resp = master.poll_once(RegisterMode::Holding, 0, 10)?;
    println!("Values: {:?}", resp.values);

    // Write single holding register (fc 0x06)
    master.write_holding(0x00, 0x1234)?;

    // Write multiple holding registers (fc 0x10)
    master.write_registers(0x00, &[0x1234, 0x5678])?;

    // Write coils (fc 0x0F)
    master.write_coils(0x00, &[true, false, true])?;

    Ok(())
}
```

### Running the API Examples

**Method 1: Using the test script (recommended)**

A Python test script is provided to run both master and slave examples simultaneously with colored, prefixed output:

```bash
# Run for 30 seconds
python3 scripts/run_api_test.py --duration 30

# Run indefinitely (Ctrl+C to stop)
python3 scripts/run_api_test.py

# Custom ports
python3 scripts/run_api_test.py --master-port /dev/ttyUSB0 --slave-port /dev/ttyUSB1

# Skip auto-build (use existing binaries)
python3 scripts/run_api_test.py --no-build
```

> **Note**: You may see "Operation timed out" warnings in the logs. This is normal behavior:
>
> - The slave times out while waiting for master requests (1s timeout)
> - The master times out while waiting for slave responses (2s timeout)
> - Both automatically retry and continue operation
> - Communication succeeds despite these warnings

**Method 2: Manual execution**

Run in separate terminals:

```bash
# Terminal 1: Start slave first
cargo run --package api_slave -- /tmp/vcom2

# Terminal 2: Start master
cargo run --package api_master -- /tmp/vcom1
```

Note: On Linux/WSL, initialize virtual serial ports first:

```bash
./scripts/socat_init.sh
```

### Complete Examples

For full examples with middleware hooks and data sources, see:

- [`examples/api_master`](../../examples/api_master) - Master with logging hooks
- [`examples/api_slave`](../../examples/api_slave) - Slave with request monitoring and statistics

## License

Licensed under the [Synthetic Source License (SySL), Version 1.0](../../LICENSE).
