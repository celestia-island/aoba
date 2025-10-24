mod cli_port_cleanup;
mod e2e;
mod utils;

use anyhow::Result;
use clap::Parser;
#[cfg(not(windows))]
use std::process::Command;

use cli_port_cleanup::test_cli_port_release;

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

/// Clean up TUI configuration cache file to ensure clean test state
///
/// TUI saves port configurations to aoba_tui_config.json and auto-loads them on startup.
/// This can cause tests to inherit state from previous runs, leading to unexpected behavior
/// in multi-station creation tests. This function removes the cache before each test.
pub fn cleanup_tui_config_cache() -> Result<()> {
    // Paths to check for config files
    let mut config_paths = vec![
        std::path::PathBuf::from("aoba_tui_config.json"),
        std::path::PathBuf::from("/tmp/aoba_tui_config.json"),
    ];

    // Also check ~/.config/aoba/
    if let Ok(home_dir) = std::env::var("HOME") {
        config_paths
            .push(std::path::PathBuf::from(home_dir).join(".config/aoba/aoba_tui_config.json"));
    }

    let mut removed_count = 0;
    for config_path in &config_paths {
        if config_path.exists() {
            log::info!("🗑️  Removing TUI config cache: {}", config_path.display());
            match std::fs::remove_file(&config_path) {
                Ok(_) => {
                    removed_count += 1;
                    log::info!("✅ Removed: {}", config_path.display());
                }
                Err(e) => {
                    log::warn!("⚠️  Failed to remove {}: {}", config_path.display(), e);
                }
            }
        }
    }

    // Also clean up any debug status files from previous runs
    let status_files = vec![
        std::path::PathBuf::from("/tmp/ci_tui_status.json"),
        std::path::PathBuf::from("/tmp/ci_cli_vcom1_status.json"),
        std::path::PathBuf::from("/tmp/ci_cli_vcom2_status.json"),
    ];

    for status_file in &status_files {
        if status_file.exists() {
            log::debug!("🗑️  Removing old status file: {}", status_file.display());
            let _ = std::fs::remove_file(&status_file);
        }
    }

    if removed_count > 0 {
        log::info!(
            "✅ TUI config cache cleaned ({} files removed)",
            removed_count
        );
    } else {
        log::debug!("📂 No TUI config cache found, nothing to clean");
    }

    Ok(())
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

    // Clean up TUI config cache before any tests to ensure clean state
    log::info!("🧪 Cleaning up TUI configuration cache...");
    cleanup_tui_config_cache()?;

    // If no module specified, show available modules and exit
    let module = match &args.module {
        Some(m) => m.as_str(),
        None => {
            log::info!("📋 Available modules:");
            log::info!("  - cli_port_release");
            log::info!("  - modbus_tui_slave_cli_master");
            log::info!("  - modbus_tui_master_cli_slave");
            log::info!("  - modbus_tui_multi_master_mixed_types");
            log::info!("");
            log::info!("Usage: cargo run --package tui_e2e -- --module <module_name>");
            return Ok(());
        }
    };

    log::info!("🧪 Running module: {}", module);

    // Run the selected module
    match module {
        "cli_port_release" => test_cli_port_release().await?,
        "modbus_tui_slave_cli_master" => {
            e2e::test_tui_slave_with_cli_master_continuous(&args.port1, &args.port2).await?
        }
        "modbus_tui_master_cli_slave" => {
            e2e::test_tui_master_with_cli_slave_continuous(&args.port1, &args.port2).await?
        }
        "modbus_tui_multi_master_mixed_types" => {
            e2e::test_tui_multi_master_mixed_types(&args.port1, &args.port2).await?
        }

        _ => {
            log::error!("❌ Unknown module: {}", module);
            log::error!("Run without --module to see available modules");
            return Err(anyhow::anyhow!("Unknown module: {}", module));
        }
    }

    log::info!("✅ Module '{}' completed successfully!", module);

    Ok(())
}
