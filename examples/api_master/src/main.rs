/// Modbus Master API Example - Hook Pattern
///
/// This example demonstrates the Builder API with hooks for monitoring
/// and logging Modbus communication.
use anyhow::Result;
use std::sync::Arc;

use _main::api::modbus::{ModbusBuilder, ModbusHook, ModbusResponse, RegisterMode};

/// Hook for logging master operations
struct LoggingHook;

impl ModbusHook for LoggingHook {
    fn on_before_request(&self, port: &str) -> Result<()> {
        Ok(())
    }

    fn on_after_response(&self, port: &str, response: &ModbusResponse) -> Result<()> {
        log::info!(
            "Received response from {}: station={}, address=0x{:04X}, values={:04X?}",
            port,
            response.station_id,
            response.register_address,
            response.values
        );
        Ok(())
    }

    fn on_error(&self, port: &str, error: &anyhow::Error) {
        log::warn!("Error on {}: {}", port, error);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    log::info!("Starting Modbus Master API Example");
    log::info!("This example demonstrates the trait-based Modbus API with hooks");

    // Get port from command line or use default
    let args: Vec<String> = std::env::args().collect();
    let port = if args.len() > 1 {
        args[1].clone()
    } else {
        "/tmp/vcom1".to_string()
    };

    log::info!("Using port: {}", port);
    log::info!("Configuration:");
    log::info!("   - Station ID: 1");
    log::info!("   - Register Mode: Holding");
    log::info!("   - Register Address: 0x0000");
    log::info!("   - Register Length: 5");
    log::info!("   - Poll Interval: 1 second");

    // Build and start the master with logging hook
    let master = ModbusBuilder::new_master(1)
        .with_port(&port)
        .with_register(RegisterMode::Holding, 0, 5)
        .with_timeout(1000)
        .add_hook(Arc::new(LoggingHook))
        .build_master()?;

    log::info!("Master started, waiting for responses...");
    log::info!("Press Ctrl+C to stop");

    // Continuously receive and log responses
    const MAX_RESPONSES: usize = 10;
    let mut count = 0;
    loop {
        if let Some(response) = master.recv_timeout(std::time::Duration::from_secs(2)) {
            count += 1;
            log::info!(
                "Response #{}: Station {}, {} values received",
                count,
                response.station_id,
                response.values.len()
            );

            if count >= MAX_RESPONSES {
                log::info!("Received {} responses, example completed", MAX_RESPONSES);
                break;
            }
        } else {
            log::warn!("Timeout waiting for response, continuing...");
        }
    }

    Ok(())
}
