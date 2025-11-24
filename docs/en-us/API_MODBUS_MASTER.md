# Modbus Master API Usage Guide

This document describes how to use Aoba's Modbus Master API from Rust applications in typical industrial scenarios (production line monitoring, process control, environment monitoring, etc.), using the `examples/api_master` crate as a reference.

## 1. Overview

Aoba exposes a trait-based Modbus master API intended for embedding into other Rust applications or hardware control software. Typical use cases include:

- Periodic polling of Modbus slave devices (RTU over serial or virtual ports)
- Collecting coil / register values into your own telemetry or control logic
- Integrating with existing logging / monitoring systems via hooks

The core entrypoint is the `ModbusBuilder` type from `_main::api::modbus`.

```rust
use _main::api::modbus::{ModbusBuilder, ModbusHook, ModbusResponse, RegisterMode};
```

> Note: in examples the crate root is called `_main`. In your own project this will usually be the main `aoba` crate or whatever name you give it in `Cargo.toml`.

---

## 2. Basic master lifecycle

A minimal master polling loop looks like this:

```rust
use anyhow::Result;
use std::time::Duration;
use _main::api::modbus::{ModbusBuilder, RegisterMode};

fn main() -> Result<()> {
    let master = ModbusBuilder::new_master(1) // station id of the slave
        .with_port("/dev/ttyUSB0")          // or `/tmp/vcom1` etc.
        .with_register(RegisterMode::Holding, 0, 10)
        .with_timeout(1000)                  // milliseconds
        .build_master()?;

    loop {
        if let Some(resp) = master.recv_timeout(Duration::from_secs(1)) {
            println!("values = {:04X?}", resp.values);
        }
    }
}
```

### Important parameters

- **Port**: any serial or virtual port that Aoba can open (real `/dev/ttyUSB*`, `/dev/ttyS*`, or virtual `/tmp/vcom*` created by socat).
- **Station ID**: Modbus slave address (usually 1–247).
- **Register mode**: one of `RegisterMode::Coils`, `DiscreteInputs`, `Holding`, `Input`.
- **Register address / length**: start address and number of items to read, matching the Modbus address table of your device (for example, a PLC or sensor gateway).
- **Timeout**: request timeout in milliseconds.

The master internally runs a polling loop and feeds responses into a channel; your code simply calls `recv_timeout` to get new data.

---

## 3. Using hooks for logging and monitoring

For production systems (industrial lines, process equipment, on‑site sensors, etc.) you usually want to:

- Log every successful response
- Track errors and timeouts
- Possibly push data into a message bus or database

The `ModbusHook` trait lets you plug in this logic centrally.

```rust
use anyhow::Result;
use std::sync::Arc;
use _main::api::modbus::{ModbusBuilder, ModbusHook, ModbusResponse, RegisterMode};

struct LoggingHook;

impl ModbusHook for LoggingHook {
    fn on_before_request(&self, port: &str) -> Result<()> {
        log::debug!("sending request on {}", port);
        Ok(())
    }

    fn on_after_response(&self, port: &str, resp: &ModbusResponse) -> Result<()> {
        log::info!(
            "resp {}: station={}, addr=0x{:04X}, values={:04X?}",
            port,
            resp.station_id,
            resp.register_address,
            resp.values,
        );
        Ok(())
    }

    fn on_error(&self, port: &str, err: &anyhow::Error) {
        log::warn!("modbus error on {}: {}", port, err);
    }
}

fn main() -> Result<()> {
    env_logger::init();

    let master = ModbusBuilder::new_master(1)
        .with_port("/tmp/vcom1")
        .with_register(RegisterMode::Holding, 0, 5)
        .with_timeout(1000)
        .add_hook(Arc::new(LoggingHook))
        .build_master()?;

    // now poll with recv_timeout as in the basic example
    # let _ = master;
    Ok(())
}
```

You can register multiple hooks (for example, one for logging, one for metrics export).

---

## 4. Integration pattern for industrial / device monitoring

For typical industrial monitoring scenarios (production lines, process units, environment monitoring devices, etc.), a common pattern is:

1. **Configure ports and stations** via Aoba TUI or CLI, or hard‑code them in your app.
2. **Create one master per physical/virtual port** using `ModbusBuilder::new_master`.
3. **Spawn a Tokio task per master** that:
   - calls `recv_timeout` in a loop
   - parses `ModbusResponse::values` into engineering units (pressure, temperature, valve status, etc.)
   - forwards processed data to your monitoring backend (MQTT, HTTP, database, etc.).
4. Use `ModbusHook` to centralize logging, latency measurement, and error counting.

Because Aoba is built on `tokio`, the master API is designed to be used inside an async runtime but exposes a simple, blocking-style `recv_timeout` for convenience in tasks.

---

## 5. Error handling and timeouts

- `build_master()` returns `anyhow::Error` if the port cannot be opened or the configuration is invalid.
- `recv_timeout()` returns `None` on timeout; this is not an error by itself.
- Protocol‑level errors (CRC, exception codes, IO errors) are reported through `ModbusHook::on_error`.

A recommended pattern:

- Treat occasional timeouts as normal in unstable serial environments.
- Use a rolling counter in your hook; if consecutive errors exceed a threshold, raise an alarm.

---

## 6. Running the example

From the repo root:

```bash
cargo run --package api_master -- /tmp/vcom1
```

In a production‑like testbed (such as a hydrogen storage tank bench), you typically:

- Use Aoba CLI/TUI or `examples/modbus_slave` to simulate the slave side.
- Then run the `api_master` example to verify that your Modbus wiring and application‑level logic behave as expected.

---

## 7. Next steps

- For slave‑side APIs, see `examples/api_slave`.
- For CLI‑level Modbus usage, see `docs/en-us/CLI_MODBUS.md`.
- For data export via HTTP / MQTT / IPC, see the `DATA_SOURCE_*.md` docs in this directory.
