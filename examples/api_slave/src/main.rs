/// Modbus Slave API Example
///
/// This example demonstrates how to use the Modbus API to create a custom slave
/// that listens for requests and provides fixed test data responses.
use std::sync::Arc;

use anyhow::Result;
use _main::api::modbus::{ModbusBuilder, ModbusHook, ModbusResponse, RegisterMode};

/// Hook for logging slave operations
struct LoggingHook;

impl ModbusHook for LoggingHook {
    fn on_before_request(&self, port: &str) -> Result<()> {
        log::debug!("👂 Listening for request on {}", port);
        Ok(())
    }

    fn on_after_response(&self, port: &str, response: &ModbusResponse) -> Result<()> {
        log::info!(
            "✅ Sent response on {}: station={}, address=0x{:04X}, values={:04X?}",
            port,
            response.station_id,
            response.register_address,
            response.values
        );
        Ok(())
    }

    fn on_error(&self, port: &str, error: &anyhow::Error) {
        log::warn!("❌ Error on {}: {}", port, error);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    log::info!("🚀 Starting Modbus Slave API Example");
    log::info!("📝 This example demonstrates the trait-based Modbus API");

    // Get port from command line or use default
    let args: Vec<String> = std::env::args().collect();
    let port = if args.len() > 1 {
        args[1].clone()
    } else {
        "/tmp/vcom2".to_string()
    };

    log::info!("📍 Using port: {}", port);
    log::info!("🔧 Configuration:");
    log::info!("   - Station ID: 1");
    log::info!("   - Register Mode: Holding");
    log::info!("   - Register Address: 0x0000");
    log::info!("   - Register Length: 5");
    log::info!("   - Initial Register Values: [0x0000, 0x0000, 0x0000, 0x0000, 0x0000]");

    // Build and start the slave
    let slave = ModbusBuilder::new_slave(1)
        .with_port(&port)
        .with_register(RegisterMode::Holding, 0, 5)
        .with_timeout(1000)
        .start_slave(Some(Arc::new(LoggingHook)))?;

    log::info!("✅ Slave started, waiting for requests...");
    log::info!("💡 The slave will respond to master requests");
    log::info!("💡 Register values will be updated by master writes");
    log::info!("💡 Press Ctrl+C to stop");

    // Continuously receive and log responses
    const MAX_REQUESTS: usize = 10;
    let mut count = 0;
    loop {
        if let Some(response) = slave.recv_timeout(std::time::Duration::from_secs(10)) {
            count += 1;
            log::info!(
                "📊 Request #{}: station={}, address=0x{:04X}, values={:04X?}",
                count,
                response.station_id,
                response.register_address,
                response.values
            );

            // Stop after MAX_REQUESTS successful responses for the example
            if count >= MAX_REQUESTS {
                log::info!("🎉 Processed {} requests, example complete!", MAX_REQUESTS);
                break;
            }
        } else {
            log::debug!("⏱️ Waiting for request...");
        }
    }

    log::info!("👋 Example finished");
    Ok(())
}
