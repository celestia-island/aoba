/// Modbus Slave API Example - Middleware Pattern
///
/// This example demonstrates the new Builder API with:
/// - Multiple hooks in a middleware chain
/// - Interceptor pattern for response processing
use anyhow::Result;
use std::sync::Arc;

use _main::api::modbus::{ModbusBuilder, ModbusHook, ModbusResponse, RegisterMode};

/// Hook 1: Monitors incoming requests
struct RequestMonitorHook;

impl ModbusHook for RequestMonitorHook {
    fn on_before_request(&self, _port: &str) -> Result<()> {
        Ok(())
    }

    fn on_after_response(&self, _port: &str, _response: &ModbusResponse) -> Result<()> {
        Ok(())
    }

    fn on_error(&self, _port: &str, error: &anyhow::Error) {
        log::warn!("[RequestMonitorHook] Error: {}", error);
    }
}

/// Hook 2: Logs all responses with details
struct ResponseLoggingHook;

impl ModbusHook for ResponseLoggingHook {
    fn on_before_request(&self, _port: &str) -> Result<()> {
        Ok(())
    }

    fn on_after_response(&self, port: &str, response: &ModbusResponse) -> Result<()> {
        log::info!(
            "[ResponseLoggingHook] Sent response on {}: station={}, addr=0x{:04X}, values={:04X?}",
            port,
            response.station_id,
            response.register_address,
            response.values
        );
        Ok(())
    }

    fn on_error(&self, _port: &str, _error: &anyhow::Error) {}
}

/// Hook 3: Statistics tracker
struct StatisticsHook {
    total_requests: Arc<std::sync::Mutex<usize>>,
}

impl StatisticsHook {
    fn new() -> Self {
        Self {
            total_requests: Arc::new(std::sync::Mutex::new(0)),
        }
    }
}

impl ModbusHook for StatisticsHook {
    fn on_before_request(&self, _port: &str) -> Result<()> {
        Ok(())
    }

    fn on_after_response(&self, _port: &str, response: &ModbusResponse) -> Result<()> {
        let mut count = self.total_requests.lock().unwrap();
        *count += 1;
        log::info!(
            "[StatisticsHook] Request #{}: Station {}, {} values sent",
            *count,
            response.station_id,
            response.values.len()
        );
        Ok(())
    }

    fn on_error(&self, _port: &str, _error: &anyhow::Error) {}
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    log::info!("Starting Modbus Slave API V2 Example (Middleware Pattern)");
    log::info!("This example demonstrates:");
    log::info!("   - Multiple hooks in a middleware chain");
    log::info!("   - Each hook processes before/after/error events");

    // Get port from command line or use default
    let args: Vec<String> = std::env::args().collect();
    let port = if args.len() > 1 {
        args[1].clone()
    } else {
        "/tmp/vcom2".to_string()
    };

    log::info!("Using port: {}", port);
    log::info!("Configuration:");
    log::info!("   - Station ID: 1");
    log::info!("   - Register Mode: Holding");
    log::info!("   - Register Address: 0x0000");
    log::info!("   - Register Length: 5");

    // Create hooks
    let monitor_hook: Arc<dyn ModbusHook> = Arc::new(RequestMonitorHook);
    let logging_hook: Arc<dyn ModbusHook> = Arc::new(ResponseLoggingHook);
    let stats_hook: Arc<dyn ModbusHook> = Arc::new(StatisticsHook::new());

    log::info!("Building middleware chain:");
    log::info!("   Hooks: RequestMonitorHook -> ResponseLoggingHook -> StatisticsHook");

    // Build slave with multiple hooks
    let _slave = ModbusBuilder::new_slave(1)
        .with_port(&port)
        .with_register(RegisterMode::Holding, 0, 5)
        .with_timeout(1000)
        .add_hook(monitor_hook)
        .add_hook(logging_hook)
        .add_hook(stats_hook)
        .build_slave()?;

    log::info!("Slave started with middleware chain");
    log::info!("Listening for master requests on {}", port);
    log::info!("Each request will be processed by the hook chain:");
    log::info!("   1. RequestMonitorHook logs before processing");
    log::info!("   2. ResponseLoggingHook logs after response");
    log::info!("   3. StatisticsHook tracks request count");
    log::info!("");
    log::info!("Press Ctrl+C to stop");

    // Keep running
    tokio::signal::ctrl_c().await?;

    log::info!("Slave stopped");
    Ok(())
}
