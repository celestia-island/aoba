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
    multi_station::{
        test_multi_station_mixed_register_types, test_multi_station_mixed_station_ids,
        test_multi_station_spaced_addresses,
    },
    single_station::{
        test_single_station_coils, test_single_station_discrete_inputs,
        test_single_station_holding_registers, test_single_station_input_registers,
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

/// CLI E2E test suite with module-based test execution
#[derive(Parser, Debug)]
#[command(name = "cli_e2e")]
#[command(about = "CLI E2E test suite", long_about = None)]
struct Args {
    /// Test module to run
    #[arg(long)]
    module: Option<String>,

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
}

/// Setup virtual serial ports by running socat_init script without requiring sudo
/// This function can be called before each test to reset ports
pub fn setup_virtual_serial_ports() -> Result<bool> {
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

    // If no module specified, show available modules and exit
    let module = match &args.module {
        Some(m) => m.as_str(),
        None => {
            log::info!("📋 Available modules:");
            log::info!("  Basic CLI:");
            log::info!("    - help");
            log::info!("    - list_ports");
            log::info!("    - list_ports_json");
            log::info!("    - list_ports_status");
            log::info!("  Modbus CLI:");
            log::info!("    - modbus_slave_listen_temp");
            log::info!("    - modbus_slave_listen_persist");
            log::info!("    - modbus_master_provide_temp");
            log::info!("    - modbus_master_provide_persist");
            log::info!("  Modbus E2E (requires vcom ports):");
            log::info!("    - modbus_slave_listen_vcom");
            log::info!("    - modbus_master_provide_vcom");
            log::info!("    - modbus_master_slave_communication");
            log::info!("    - modbus_basic_master_slave");
            log::info!("    - modbus_multi_masters");
            log::info!("    - modbus_multi_masters_same_station");
            log::info!("    - modbus_multi_slaves");
            log::info!("    - modbus_multi_slaves_same_station");
            log::info!("    - modbus_multi_slaves_adjacent_registers");
            log::info!("  Single-Station Register Mode Tests:");
            log::info!("    - modbus_single_station_coils");
            log::info!("    - modbus_single_station_discrete_inputs");
            log::info!("    - modbus_single_station_holding");
            log::info!("    - modbus_single_station_input");
            log::info!("  Multi-Station Tests (2 stations):");
            log::info!("    - modbus_multi_station_mixed_types");
            log::info!("    - modbus_multi_station_spaced_addresses");
            log::info!("    - modbus_multi_station_mixed_ids");
            log::info!("");
            log::info!("Usage: cargo run --package cli_e2e -- --module <module_name>");
            return Ok(());
        }
    };

    log::info!("🧪 Running module: {}", module);

    // Run the selected module
    match module {
        // Basic CLI tests
        "help" => test_cli_help().await?,
        "list_ports" => test_cli_list_ports().await?,
        "list_ports_json" => test_cli_list_ports_json().await?,
        "list_ports_status" => test_cli_list_ports_json_with_status().await?,

        // Modbus CLI tests (no vcom needed)
        "modbus_slave_listen_temp" => test_slave_listen_temp().await?,
        "modbus_slave_listen_persist" => test_slave_listen_persist().await?,
        "modbus_master_provide_temp" => test_master_provide_temp().await?,
        "modbus_master_provide_persist" => test_master_provide_persist().await?,

        // Modbus E2E tests (require vcom ports)
        "modbus_slave_listen_vcom" => test_slave_listen_with_vcom().await?,
        "modbus_master_provide_vcom" => test_master_provide_with_vcom().await?,
        "modbus_master_slave_communication" => test_master_slave_communication().await?,
        "modbus_basic_master_slave" => test_basic_master_slave_communication().await?,
        "modbus_multi_masters" => test_multi_masters().await?,
        "modbus_multi_masters_same_station" => test_multi_masters_same_station().await?,
        "modbus_multi_slaves" => test_multi_slaves().await?,
        "modbus_multi_slaves_same_station" => test_multi_slaves_same_station().await?,
        "modbus_multi_slaves_adjacent_registers" => test_multi_slaves_adjacent_registers().await?,

        // Single-Station Register Mode Tests
        "modbus_single_station_coils" => test_single_station_coils().await?,
        "modbus_single_station_discrete_inputs" => test_single_station_discrete_inputs().await?,
        "modbus_single_station_holding" => test_single_station_holding_registers().await?,
        "modbus_single_station_input" => test_single_station_input_registers().await?,

        // Multi-Station Tests (2 stations)
        "modbus_multi_station_mixed_types" => test_multi_station_mixed_register_types().await?,
        "modbus_multi_station_spaced_addresses" => test_multi_station_spaced_addresses().await?,
        "modbus_multi_station_mixed_ids" => test_multi_station_mixed_station_ids().await?,

        _ => {
            log::error!("❌ Unknown module: {}", module);
            log::error!("Run without --module to see available modules");
            return Err(anyhow::anyhow!("Unknown module: {}", module));
        }
    }

    log::info!("✅ Module '{}' completed successfully!", module);

    Ok(())
}
