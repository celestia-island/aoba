//! TUI End-to-End Test Framework
//!
//! This is a comprehensive E2E test suite for AOBA's TUI (Terminal User Interface) Modbus functionality.
//!
//! # Overview
//!
//! This test framework provides automated testing for:
//! - **Single-station** Master/Slave operations with all register types
//! - **Multi-station** configurations with mixed types, addresses, and IDs
//! - **Transaction retry** mechanism for reliable CI/CD testing
//! - **Safe rollback** with multi-layer checkpoints
//!
//! # Architecture
//!
//! The framework is organized into several key components:
//!
//! - **`e2e::common`**: Core testing utilities (see `cargo doc` for detailed API documentation)
//!   - Transaction retry system with configurable attempts and delays
//!   - Safe rollback with adaptive Escape handling to prevent over-navigation
//!   - Station configuration helpers for single and multi-station setups
//!   - Data verification helpers for Master/Slave operations
//!
//! - **`e2e::single_station`**: Single-station test modules
//!   - `master_modes`: Master tests for Coils, DiscreteInputs, Holding, Input registers
//!   - `slave_modes`: Slave tests for all register types
//!
//! - **`e2e::multi_station`**: Multi-station test modules
//!   - `master_modes`: Tests for mixed types, spaced addresses, mixed station IDs
//!   - `slave_modes`: Slave-mode multi-station configurations
//!
//! # Quick Start
//!
//! List all available test modules:
//!
//! ```bash
//! cargo run --package tui_e2e
//! ```
//!
//! Run a specific test module:
//!
//! ```bash
//! # Test Master mode with Coils registers
//! cargo run --package tui_e2e -- --module tui_master_coils
//!
//! # Test Slave mode with Holding registers
//! cargo run --package tui_e2e -- --module tui_slave_holding
//!
//! # Custom serial ports
//! cargo run --package tui_e2e -- --module tui_master_coils \
//!     --port1 /tmp/vcom1 --port2 /tmp/vcom2
//!
//! # Enable debug mode for detailed logging
//! cargo run --package tui_e2e -- --module tui_master_coils --debug
//! ```
//!
//! # Available Test Modules
//!
//! ## Single-Station Tests
//!
//! **Master Mode:**
//! - `tui_master_coils` - Test Coils (01) registers
//! - `tui_master_discrete_inputs` - Test Discrete Inputs (02) registers
//! - `tui_master_holding` - Test Holding (03) registers
//! - `tui_master_input` - Test Input (04) registers
//!
//! **Slave Mode:**
//! - `tui_slave_coils` - Test Coils registers as Slave
//! - `tui_slave_discrete_inputs` - Test Discrete Inputs as Slave
//! - `tui_slave_holding` - Test Holding registers as Slave
//! - `tui_slave_input` - Test Input registers as Slave
//!
//! ## Multi-Station Tests
//!
//! **Master Mode:**
//! - `tui_multi_master_mixed_types` - Multiple stations with different register types
//! - `tui_multi_master_spaced_addresses` - Stations with non-contiguous addresses
//! - `tui_multi_master_mixed_ids` - Stations with different station IDs
//!
//! **Slave Mode:**
//! - `tui_multi_slave_mixed_types` - Multi-station Slave with mixed types
//! - `tui_multi_slave_spaced_addresses` - Multi-station with address gaps
//! - `tui_multi_slave_mixed_ids` - Multi-station with different IDs
//!
//! # Documentation
//!
//! All implementation details are documented as inline comments in the source code.
//! To view the complete API documentation with examples and architecture diagrams:
//!
//! ```bash
//! cargo doc --open
//! ```
//!
//! Navigate to `aoba::examples::tui_e2e::e2e::common` to see detailed documentation for:
//! - Transaction retry mechanism with adaptive Escape handling
//! - Safe rollback with multi-layer checkpoints
//! - Configuration workflows for single and multi-station setups
//! - Test orchestrators with comprehensive examples
//! - All public functions with usage patterns
//!
//! # Testing Best Practices
//!
//! ## CI Environment Considerations
//!
//! The test framework is designed to work reliably in CI environments where
//! terminal responses are 2-4x slower than local development:
//!
//! - Edit mode operations use 800ms delays (vs 200ms locally)
//! - Transaction retry with up to 3 attempts per operation
//! - Safe rollback prevents over-escaping to wrong pages
//!
//! ## Timing Guidelines
//!
//! | Operation | Local | CI | Reason |
//! |-----------|-------|-----|--------|
//! | Edit mode entry | 200ms | 800ms | Wait for cursor to appear |
//! | Edit mode exit | 200ms | 800ms | Wait for value to sync |
//! | Register Count commit | 500ms | 1000ms | UI update required |
//! | Ctrl+S sync | 2s | 5s | Status tree synchronization |
//!
//! ## Troubleshooting
//!
//! ### Test Fails with "Over-escaped to Entry page"
//!
//! **Cause**: Rollback pressed Escape too many times
//!
//! **Solution**: The safe rollback mechanism should prevent this. If it still occurs:
//! 1. Check if custom rollback actions are pressing extra Escapes
//! 2. Verify navigation reset sequence is correct
//! 3. Enable debug mode to see detailed checkpoint logs
//!
//! ### Configuration Not Applied
//!
//! **Cause**: Values not synced to status tree
//!
//! **Solution**:
//! 1. Ensure sufficient delays after field edits
//! 2. Verify Ctrl+S was executed
//! 3. Check status file: `/tmp/ci_tui_status.json`
//! 4. Use `CheckStatus` to wait for specific values
//!
//! ### Station Already Exists Error
//!
//! **Cause**: Previous test left configuration cache
//!
//! **Solution**: The framework automatically calls `cleanup_tui_config_cache()`.
//! If issue persists, manually remove:
//! - `aoba_tui_config.json` (current directory)
//! - `/tmp/aoba_tui_config.json`
//! - `~/.config/aoba/aoba_tui_config.json`
//!
//! ## Debug Tools
//!
//! Enable debug mode to see:
//! - Detailed checkpoint verification logs
//! - Screen captures at each step
//! - Retry attempt notifications
//!
//! Debug screenshots are saved to `/tmp/tui_e2e_debug/`
//!
//! # See Also
//!
//! - `src/e2e/common.rs` - Core implementation with comprehensive inline documentation
//! - `../../docs/zh-chs/CLI_MODBUS.md` - CLI Modbus usage (for reference)
//! - `../../scripts/socat_init.sh` - Virtual serial port setup script
//! - `../ci_utils/` - Testing utilities (terminal capture, cursor actions, status monitoring)

mod e2e;

use anyhow::Result;
use clap::Parser;
#[cfg(not(windows))]
use std::process::Command;

/// TUI E2E test suite with module-based test execution
#[derive(Parser, Debug)]
#[command(name = "tui_e2e")]
#[command(about = "TUI E2E test suite", long_about = None)]
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
}

/// Clean up debug status files from previous test runs
///
/// With `--no-config-cache` flag, TUI no longer saves/loads aoba_tui_config.json,
/// so this function now only cleans up debug status files to ensure clean test state.
///
/// **Note**: Configuration cache cleanup is no longer needed since tests use
/// `--no-config-cache` flag. See `setup_tui_test()` in common.rs.
pub fn cleanup_debug_status_files() -> Result<()> {
    // Clean up debug status files from previous runs
    let status_files = vec![
        std::path::PathBuf::from("/tmp/ci_tui_status.json"),
        std::path::PathBuf::from("/tmp/ci_cli_vcom1_status.json"),
        std::path::PathBuf::from("/tmp/ci_cli_vcom2_status.json"),
    ];

    let mut removed_count = 0;
    for status_file in &status_files {
        if status_file.exists() {
            log::debug!("🗑️  Removing old status file: {}", status_file.display());
            match std::fs::remove_file(status_file) {
                Ok(_) => removed_count += 1,
                Err(e) => log::warn!("⚠️  Failed to remove {}: {}", status_file.display(), e),
            }
        }
    }

    if removed_count > 0 {
        log::debug!("✅ Cleaned {} debug status files", removed_count);
    }

    Ok(())
}

/// Setup virtual serial ports by running socat_init script without requiring sudo
/// This function can be called before each test to reset ports
pub fn setup_virtual_serial_ports() -> Result<bool> {
    #[cfg(windows)]
    {
        log::info!("🧪 Windows platform: skipping virtual serial port setup (socat not available)");
        Ok(false)
    }

    #[cfg(not(windows))]
    {
        log::info!("🧪 Setting up virtual serial ports...");

        // Find the socat_init.sh script (centralized at repo root)
        // Try both relative paths: from repo root and from examples/tui_e2e
        let script_paths = [
            std::path::Path::new("scripts/socat_init.sh"),
            std::path::Path::new("../../scripts/socat_init.sh"),
        ];

        let script_path = script_paths.iter().find(|p| p.exists()).ok_or_else(|| {
            anyhow::anyhow!(
                "⚠️ socat_init.sh script not found at any of: {}",
                script_paths
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;

        log::debug!(
            "📍 Using socat_init.sh script at: {}",
            script_path.display()
        );

        // Run the script directly; it operates entirely in user-mode
        let output = Command::new("bash")
            .arg(script_path)
            .arg("--mode")
            .arg("tui")
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

    log::info!("🧪 Starting TUI E2E Tests...");
    log::info!(
        "📍 Port configuration: port1={}, port2={}",
        args.port1,
        args.port2
    );

    // Clean up debug status files from previous runs
    log::debug!("🧪 Cleaning up debug status files...");
    cleanup_debug_status_files()?;

    // If no module specified, show available modules and exit
    let module = match &args.module {
        Some(m) => m.as_str(),
        None => {
            log::info!("📋 Available modules:");
            log::info!("  TUI Single-Station Master Mode:");
            log::info!("    - tui_master_coils");
            log::info!("    - tui_master_discrete_inputs");
            log::info!("    - tui_master_holding");
            log::info!("    - tui_master_input");
            log::info!("  TUI Single-Station Slave Mode:");
            log::info!("    - tui_slave_coils");
            log::info!("    - tui_slave_discrete_inputs");
            log::info!("    - tui_slave_holding");
            log::info!("    - tui_slave_input");
            log::info!("  TUI Multi-Station Master Mode:");
            log::info!("    - tui_multi_master_mixed_types");
            log::info!("    - tui_multi_master_spaced_addresses");
            log::info!("    - tui_multi_master_mixed_ids");
            log::info!("  TUI Multi-Station Slave Mode:");
            log::info!("    - tui_multi_slave_mixed_types");
            log::info!("    - tui_multi_slave_spaced_addresses");
            log::info!("    - tui_multi_slave_mixed_ids");
            log::info!("");
            log::info!("Usage: cargo run --package tui_e2e -- --module <module_name>");
            return Ok(());
        }
    };

    log::info!("🧪 Running module: {module}");

    // Run the selected module
    let result = match module {
        // TUI Single-Station Master Mode Tests
        "tui_master_coils" => e2e::test_tui_master_coils(&args.port1, &args.port2).await,
        "tui_master_discrete_inputs" => {
            e2e::test_tui_master_discrete_inputs(&args.port1, &args.port2).await
        }
        "tui_master_holding" => {
            e2e::test_tui_master_holding_registers(&args.port1, &args.port2).await
        }
        "tui_master_input" => e2e::test_tui_master_input_registers(&args.port1, &args.port2).await,

        // TUI Single-Station Slave Mode Tests
        "tui_slave_coils" => e2e::test_tui_slave_coils(&args.port1, &args.port2).await,
        "tui_slave_discrete_inputs" => {
            e2e::test_tui_slave_discrete_inputs(&args.port1, &args.port2).await
        }
        "tui_slave_holding" => {
            e2e::test_tui_slave_holding_registers(&args.port1, &args.port2).await
        }
        "tui_slave_input" => e2e::test_tui_slave_input_registers(&args.port1, &args.port2).await,

        // TUI Multi-Station Master Mode Tests
        "tui_multi_master_mixed_types" => {
            e2e::test_tui_multi_master_mixed_register_types(&args.port1, &args.port2).await
        }
        "tui_multi_master_spaced_addresses" => {
            e2e::test_tui_multi_master_spaced_addresses(&args.port1, &args.port2).await
        }
        "tui_multi_master_mixed_ids" => {
            e2e::test_tui_multi_master_mixed_station_ids(&args.port1, &args.port2).await
        }

        // TUI Multi-Station Slave Mode Tests
        "tui_multi_slave_mixed_types" => {
            e2e::test_tui_multi_slave_mixed_register_types(&args.port1, &args.port2).await
        }
        "tui_multi_slave_spaced_addresses" => {
            e2e::test_tui_multi_slave_spaced_addresses(&args.port1, &args.port2).await
        }
        "tui_multi_slave_mixed_ids" => {
            e2e::test_tui_multi_slave_mixed_station_ids(&args.port1, &args.port2).await
        }

        _ => {
            log::error!("❌ Unknown module: {module}");
            log::error!("Run without --module to see available modules");
            return Err(anyhow::anyhow!("Unknown module: {module}"));
        }
    };

    match result {
        Ok(()) => {
            log::info!("✅ Module '{module}' completed successfully!");
            Ok(())
        }
        Err(err) => {
            log::error!("❌ Module '{module}' failed: {err:?}");
            ci_utils::log_last_terminal_snapshot(&format!("Module '{module}' failure"));
            Err(err)
        }
    }
}
