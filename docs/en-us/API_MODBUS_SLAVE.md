# Modbus Slave API Usage Guide

This document describes how to use Aoba's Modbus Slave API from Rust applications to expose data to Modbus masters. Typical use cases include industrial production lines, process control systems and test benches.

The reference example is the `examples/api_slave` crate.

## 1. Overview

Aoba provides a slave-side API that mirrors the master API style, based on a Builder + Hook pattern. It is useful when you want to:

- Turn your process into a Modbus slave, exposing coil/register data to external masters;
- Quickly build a configurable Modbus device for integration tests or simulation;
- Attach a middleware chain of hooks for logging, statistics, access control and alerts.

The main entrypoint is still `_main::api::modbus::ModbusBuilder`, but you use `new_slave` / `build_slave`:

```rust
use _main::api::modbus::{ModbusBuilder, ModbusHook, ModbusResponse, RegisterMode};
```

---

## 2. Basic slave lifecycle

A simplified version of the example slave looks like this:

```rust
use anyhow::Result;
use std::sync::Arc;
use _main::api::modbus::{ModbusBuilder, ModbusHook, ModbusResponse, RegisterMode};

struct ResponseLoggingHook;

impl ModbusHook for ResponseLoggingHook {
    fn on_before_request(&self, _port: &str) -> Result<()> {
        Ok(())
    }

    fn on_after_response(&self, port: &str, response: &ModbusResponse) -> Result<()> {
        log::info!(
            "sent response on {}: station={}, addr=0x{:04X}, values={:04X?}",
            port,
            response.station_id,
            response.register_address,
            response.values
        );
        Ok(())
    }

    fn on_error(&self, _port: &str, error: &anyhow::Error) {
        log::warn!("error: {}", error);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    let args: Vec<String> = std::env::args().collect();
    let port = if args.len() > 1 { &args[1] } else { "/tmp/vcom2" };

    let hook: Arc<dyn ModbusHook> = Arc::new(ResponseLoggingHook);

    let _slave = ModbusBuilder::new_slave(1)
        .with_port(port)
        .with_register(RegisterMode::Holding, 0, 5)
        .with_timeout(1000)
        .add_hook(hook)
        .build_slave()?;

    // Keep the slave running and listening for master requests
    tokio::signal::ctrl_c().await?;
    Ok(())
}
```

### Core configuration parameters

- **Port**: same format as for the master (`/dev/ttyUSB*`, `/dev/ttyS*`, `/tmp/vcom2`, etc.);
- **Station ID**: must match the station id that masters will use when talking to this slave;
- **Register mode and address range**: define which part of the Modbus address space this slave exposes;
- **Timeout**: used internally to control IO/processing timeouts (usually aligned with master settings).

---

## 3. Hook middleware chain

On the slave side you can also register multiple hooks to form a middleware chain. Typical responsibilities:

- Validate or inspect incoming requests before they are processed;
- Log and post-process responses after they are sent;
- Raise alerts or update statistics when errors occur.

The `examples/api_slave` crate demonstrates three chained hooks:

- `RequestMonitorHook`: monitors requests and logs/alerts on errors;
- `ResponseLoggingHook`: logs every response with register address and values;
- `StatisticsHook`: tracks request counts.

This pattern lets you keep cross‑cutting concerns (logging, metrics, access control, rate limiting, etc.) out of your core business logic and attach them declaratively to a slave instance.

---

## 4. Typical use cases

Common use cases for the slave API in industrial environments and test setups include:

1. **Software‑based device simulator**
   - When real devices are not yet available, simulate a Modbus device in Rust;
   - Periodically update internal register values according to your test scenarios;
   - Drive end‑to‑end integration tests in CI.
2. **Protocol adaptation layer**
   - Your actual devices may speak CAN, proprietary TCP or another fieldbus, while upstream systems expect Modbus;
   - Use the slave API to map those signals into a Modbus register/coil space and present a unified Modbus interface.
3. **Edge gateway exposing processed data**
   - Collect and normalize data from multiple sources inside your process or gateway;
   - Use the slave API to expose the processed/aggregated data to legacy SCADA or third‑party systems via Modbus.

---

## 5. Using master and slave APIs together

Because the master and slave APIs share the same Builder + Hook design, you can easily combine them within a single process:

1. Use the master API to poll several upstream devices and build a unified internal data model;
2. Use the slave API to map that data model to a Modbus register space;
3. Let external systems treat your process as a standard Modbus device.

This pattern is useful for building protocol gateways, aggregation nodes, or test harnesses.

---

## 6. Running the slave example

From the repo root:

```bash
cargo run --package api_slave -- /tmp/vcom2
```

You can pair this with the master example or Aoba CLI/TUI for testing:

- Start the slave example listening on `/tmp/vcom2`;
- Then use the master example or CLI/TUI to poll that port and verify read/write behaviour.

---

## 7. Related documentation

- Master‑side API: `docs/en-us/API_MODBUS_MASTER.md`;
- CLI‑level Modbus usage: `docs/en-us/CLI_MODBUS.md`;
- Data source / export capabilities (HTTP, MQTT, IPC, etc.): see the `DATA_SOURCE_*.md` documents in this directory;
- More end‑to‑end examples live under the `examples` directory.
