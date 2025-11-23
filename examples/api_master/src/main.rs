/// Modbus Master API Example
///
/// This example demonstrates how to use the Modbus API to create a custom master
/// that polls a slave and logs responses. It uses the trait-based API with a
/// fixed data source for testing.
use std::sync::{Arc, Mutex};

use anyhow::Result;
use _main::api::modbus::{ModbusBuilder, ModbusDataSource, ModbusHook, ModbusResponse, RegisterMode};

/// Fixed test data source that cycles through predefined values
struct FixedTestDataSource {
    values: Vec<Vec<u16>>,
    index: usize,
}

impl FixedTestDataSource {
    fn new() -> Self {
        Self {
            // Define test data patterns - these will be written to slave registers
            values: vec![
                vec![0x0001, 0x0002, 0x0003, 0x0004, 0x0005],
                vec![0x0010, 0x0020, 0x0030, 0x0040, 0x0050],
                vec![0x0100, 0x0200, 0x0300, 0x0400, 0x0500],
                vec![0x1000, 0x2000, 0x3000, 0x4000, 0x5000],
            ],
            index: 0,
        }
    }
}

impl ModbusDataSource for FixedTestDataSource {
    fn next_data(&mut self) -> Option<Vec<u16>> {
        if self.values.is_empty() {
            return None;
        }
        
        self.index += 1;
        let data = self.values[(self.index - 1) % self.values.len()].clone();
        
        log::info!("📤 Providing test data (iteration {}): {:04X?}", self.index, data);
        Some(data)
    }
}

/// Hook for logging master operations
struct LoggingHook;

impl ModbusHook for LoggingHook {
    fn on_before_request(&self, port: &str) -> Result<()> {
        log::debug!("🔄 About to poll on {}", port);
        Ok(())
    }

    fn on_after_response(&self, port: &str, response: &ModbusResponse) -> Result<()> {
        log::info!(
            "✅ Received response from {}: station={}, address=0x{:04X}, values={:04X?}",
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

    log::info!("🚀 Starting Modbus Master API Example");
    log::info!("📝 This example demonstrates the trait-based Modbus API");

    // Get port from command line or use default
    let args: Vec<String> = std::env::args().collect();
    let port = if args.len() > 1 {
        args[1].clone()
    } else {
        "/tmp/vcom1".to_string()
    };

    log::info!("📍 Using port: {}", port);
    log::info!("🔧 Configuration:");
    log::info!("   - Station ID: 1");
    log::info!("   - Register Mode: Holding");
    log::info!("   - Register Address: 0x0000");
    log::info!("   - Register Length: 5");
    log::info!("   - Poll Interval: 1 second");

    // Create fixed test data source
    let data_source = Arc::new(Mutex::new(FixedTestDataSource::new()));

    // Build and start the master
    let master = ModbusBuilder::new_master(1)
        .with_port(&port)
        .with_register(RegisterMode::Holding, 0, 5)
        .with_timeout(1000)
        .start_master(Some(Arc::new(LoggingHook)), Some(data_source))?;

    log::info!("✅ Master started, waiting for responses...");
    log::info!("💡 Press Ctrl+C to stop");

    // Continuously receive and log responses
    const MAX_RESPONSES: usize = 10;
    let mut count = 0;
    loop {
        if let Some(response) = master.recv_timeout(std::time::Duration::from_secs(2)) {
            count += 1;
            log::info!(
                "📊 Response #{}: station={}, address=0x{:04X}, values={:04X?}",
                count,
                response.station_id,
                response.register_address,
                response.values
            );

            // Stop after MAX_RESPONSES successful responses for the example
            if count >= MAX_RESPONSES {
                log::info!("🎉 Received {} responses, example complete!", MAX_RESPONSES);
                break;
            }
        } else {
            log::warn!("⏱️ Timeout waiting for response");
        }
    }

    log::info!("👋 Example finished");
    Ok(())
}
