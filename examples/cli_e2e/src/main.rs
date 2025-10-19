// TODO: Temporarily commented out - needs update to new config structure
// mod config_mode;
mod e2e;
mod help;
mod list_ports;
mod list_ports_json;
mod list_ports_status;
mod modbus_cli;
mod modbus_e2e;

use anyhow::Result;
use clap::Parser;
#[cfg(not(windows))]
use std::process::Command;

// use config_mode::test_config_mode;
use e2e::{
    basic::test_basic_master_slave_communication,
    multi_masters::{test_multi_masters, test_multi_masters_same_station},
    multi_slaves::{
        test_multi_slaves, test_multi_slaves_adjacent_registers, test_multi_slaves_same_station,
    },
};
use help::test_cli_help;
use list_ports::test_cli_list_ports;
use list_ports_json::test_cli_list_ports_json;
use list_ports_status::test_cli_list_ports_json_with_status;
use modbus_cli::{
    test_master_provide_persist, test_master_provide_temp, test_slave_listen_persist,
    test_slave_listen_temp,
};
use modbus_e2e::{
    test_master_provide_with_vcom, test_master_slave_communication, test_slave_listen_with_vcom,
};

/// CLI E2E test suite with selective test execution
#[derive(Parser, Debug)]
#[command(name = "cli_e2e")]
#[command(about = "CLI E2E test suite", long_about = None)]
struct Args {
    /// Virtual serial port 1 path
    #[arg(long, default_value = "/tmp/vcom1")]
    port1: String,

    /// Virtual serial port 2 path
    #[arg(long, default_value = "/tmp/vcom2")]
    port2: String,

    /// Enable debug mode (show debug breakpoints and additional logging)
    #[arg(long)]
    debug: bool,

    /// Number of test loop iterations
    #[arg(long, default_value = "1")]
    loop_count: usize,

    /// Run only basic CLI tests (help, list-ports)
    #[arg(long)]
    basic: bool,

    /// Run only Modbus CLI tests (temp/persist modes)
    #[arg(long)]
    modbus_cli: bool,

    /// Run only E2E tests with virtual ports
    #[arg(long)]
    e2e: bool,
}

impl Args {
    /// Check if any specific test category is selected
    fn has_specific_tests(&self) -> bool {
        self.basic || self.modbus_cli || self.e2e
    }

    /// Check if basic tests should run
    fn should_run_basic(&self) -> bool {
        !self.has_specific_tests() || self.basic
    }

    /// Check if modbus CLI tests should run
    fn should_run_modbus_cli(&self) -> bool {
        !self.has_specific_tests() || self.modbus_cli
    }

    /// Check if E2E tests should run
    fn should_run_e2e(&self) -> bool {
        !self.has_specific_tests() || self.e2e
    }
}

/// Setup virtual serial ports by running socat_init script without requiring sudo
/// This function can be called before each test to reset ports
pub fn setup_virtual_serial_ports(port1: &str, port2: &str) -> Result<bool> {
    #[cfg(windows)]
    {
        log::info!("🧪 Windows platform: skipping virtual serial port setup (socat not available)");
        return Ok(false);
    }

    #[cfg(not(windows))]
    {
        log::info!("🧪 Setting up virtual serial ports...");

        // Find the socat_init.sh script (centralized at repo root)
        let script_path = std::path::Path::new("scripts/socat_init.sh");

        if !script_path.exists() {
            log::warn!(
                "⚠️ socat_init.sh script not found at {}",
                script_path.display()
            );
            return Ok(false);
        }

        // Run the script (no sudo required) to reset/reinitialize virtual serial ports
        let output = Command::new("bash")
            .arg(script_path)
            .arg("--mode")
            .arg("cli")
            .output()?;

        if output.status.success() {
            // Don't use environment variables anymore, just log the ports being used
            log::info!("✅ Virtual serial ports reset successfully");
            log::info!("🔗 Using ports: PORT1={port1}, PORT2={port2}");
            Ok(true)
        } else {
            log::warn!("⚠️ Failed to setup virtual serial ports:");
            log::warn!(
                "stdout: {stdout}",
                stdout = String::from_utf8_lossy(&output.stdout)
            );
            log::warn!("stderr: {}", String::from_utf8_lossy(&output.stderr));
            Ok(false)
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    // Set debug mode if requested
    if args.debug {
        std::env::set_var("DEBUG_MODE", "1");
        log::info!("🐛 Debug mode enabled");
    }

    log::info!(
        "📍 Port configuration: port1={}, port2={}",
        args.port1,
        args.port2
    );

    if args.loop_count > 1 {
        log::info!(
            "🧪 Running tests in loop mode: {} iterations",
            args.loop_count
        );
    }

    for iteration in 1..=args.loop_count {
        if args.loop_count > 1 {
            log::info!("🧪 ===== Iteration {iteration}/{} =====", args.loop_count);
        }

        log::info!("🧪 Starting CLI E2E Tests...");

        if args.should_run_basic() {
            test_cli_help().await?;
            test_cli_list_ports().await?;
            test_cli_list_ports_json().await?;
            test_cli_list_ports_json_with_status().await?;
        }

        if args.should_run_modbus_cli() {
            log::info!("🧪 Testing Modbus CLI features (basic)...");
            test_slave_listen_temp().await?;
            test_slave_listen_persist().await?;
            test_master_provide_temp().await?;
            test_master_provide_persist().await?;
        }

        // Check if we can setup virtual serial ports for E2E tests
        if args.should_run_e2e() && setup_virtual_serial_ports(&args.port1, &args.port2)? {
            log::info!("🧪 Virtual serial ports available, running E2E tests...");

            // Run each E2E test with fresh port initialization
            log::info!("🧪 Test 1/7: Slave listen with virtual ports");
            setup_virtual_serial_ports(&args.port1, &args.port2)?;
            test_slave_listen_with_vcom().await?;

            log::info!("🧪 Test 2/7: Master provide with virtual ports");
            setup_virtual_serial_ports(&args.port1, &args.port2)?;
            test_master_provide_with_vcom().await?;

            log::info!("🧪 Test 3/7: Master-slave communication");
            setup_virtual_serial_ports(&args.port1, &args.port2)?;
            test_master_slave_communication().await?;

            log::info!("🧪 Test 4/7: Basic master-slave communication");
            setup_virtual_serial_ports(&args.port1, &args.port2)?;
            test_basic_master_slave_communication().await?;

            log::info!("🧪 Test 5/7: Configuration mode test");
            setup_virtual_serial_ports(&args.port1, &args.port2)?;
            // TODO: Fix config_mode.rs to use new config structure
            // test_config_mode().await?;
            log::warn!("⚠️  Skipping config_mode test (needs update to new config structure)");

            log::info!("🧪 Test 6/7: Multi-master configurations");
            setup_virtual_serial_ports(&args.port1, &args.port2)?;
            test_multi_masters().await?;

            log::info!("🧪 Test 7/7: Multi-master same station configurations");
            setup_virtual_serial_ports(&args.port1, &args.port2)?;
            test_multi_masters_same_station().await?;

            log::info!("🧪 Test 8/7: Multi-slave configurations");
            setup_virtual_serial_ports(&args.port1, &args.port2)?;
            test_multi_slaves().await?;

            log::info!("🧪 Test 9/7: Multi-slave same station configurations");
            setup_virtual_serial_ports(&args.port1, &args.port2)?;
            test_multi_slaves_same_station().await?;

            log::info!("🧪 Test 10/7: Multi-slave adjacent registers configurations");
            setup_virtual_serial_ports(&args.port1, &args.port2)?;
            test_multi_slaves_adjacent_registers().await?;
        } else if args.should_run_e2e() {
            log::warn!("⚠️ Virtual serial ports setup failed, skipping E2E tests");
        }

        if args.loop_count > 1 {
            log::info!(
                "✅ Iteration {iteration}/{} completed successfully!",
                args.loop_count
            );
        } else {
            log::info!("🧪 All CLI E2E tests passed!");
        }
    }

    if args.loop_count > 1 {
        log::info!(
            "🎉 All {} iterations completed successfully!",
            args.loop_count
        );
    }

    Ok(())
}
