mod cli_port_cleanup;
mod e2e;
mod utils;

use anyhow::Result;
use clap::Parser;
#[cfg(not(windows))]
use std::process::Command;

use cli_port_cleanup::test_cli_port_release;

/// TUI E2E test suite with selective test execution
#[derive(Parser, Debug)]
#[command(name = "tui_e2e")]
#[command(about = "TUI E2E test suite", long_about = None)]
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

    /// Run only test 0: CLI port release verification
    #[arg(long)]
    test0: bool,

    /// Run only test 1: TUI Slave + CLI Master
    #[arg(long)]
    test1: bool,

    /// Run only test 2: TUI Master + CLI Slave
    #[arg(long)]
    test2: bool,

    /// Run only test 3: Multiple TUI Masters
    #[arg(long)]
    test3: bool,

    /// Run only test 4: Multiple TUI Slaves
    #[arg(long)]
    test4: bool,
}

impl Args {
    /// Check if any specific test is selected
    fn has_specific_tests(&self) -> bool {
        self.test0 || self.test1 || self.test2 || self.test3 || self.test4
    }

    /// Check if a specific test should run
    fn should_run_test(&self, test_num: usize) -> bool {
        if !self.has_specific_tests() {
            // If no specific tests selected, run all tests
            return true;
        }

        match test_num {
            0 => self.test0,
            1 => self.test1,
            2 => self.test2,
            3 => self.test3,
            4 => self.test4,
            _ => false,
        }
    }
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
        config_paths.push(std::path::PathBuf::from(home_dir).join(".config/aoba/aoba_tui_config.json"));
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
        log::info!("✅ TUI config cache cleaned ({} files removed)", removed_count);
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

    // On Unix-like systems, try to setup virtual serial ports
    #[cfg(not(windows))]
    {
        log::info!("🧪 Setting up virtual serial ports...");
        setup_virtual_serial_ports()?;
    }

    // Check if we should run virtual serial port tests
    if !ci_utils::should_run_vcom_tests_with_ports(&args.port1, &args.port2) {
        log::warn!("⚠️ Virtual serial ports not available, skipping E2E tests");
        return Ok(());
    }

    log::info!("🧪 Virtual serial ports available, running E2E tests...");

    // Test 0: CLI port release test - verify CLI properly releases ports on exit
    if args.should_run_test(0) {
        log::info!("🧪 Test 0/4: CLI port release verification");
        test_cli_port_release().await?;

        // Reset ports after CLI cleanup test to remove any lingering locks from the spawned CLI process
        #[cfg(not(windows))]
        {
            log::info!("🧪 Resetting virtual serial ports after Test 0...");

            // Kill all aoba processes to ensure clean state
            let _ = std::process::Command::new("pkill")
                .args(&["-9", "aoba"])
                .output();

            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

            // Reset socat to fully release port locks
            setup_virtual_serial_ports()?;

            // Add extended delay to ensure PTY devices are ready
            log::info!("⏳ Waiting for ports to stabilize (15 seconds)...");
            tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;
        }
    }

    // Test 1: TUI Slave + CLI Master with 3 rounds (refactored with status monitoring)
    // Test 1: TUI Slave + CLI Master with 3 rounds (status monitoring)
    if args.should_run_test(1) {
        log::info!(
            "🧪 Test 1/4: TUI Slave + CLI Master (3 rounds, holding registers, status monitoring)"
        );
        e2e::test_tui_slave_with_cli_master_continuous(&args.port1, &args.port2).await?;

        // Reset ports after test completes (Unix only)
        #[cfg(not(windows))]
        {
            log::info!("🧪 Resetting virtual serial ports after Test 1...");

            // Kill all aoba processes to ensure clean state
            let _ = std::process::Command::new("pkill")
                .args(&["-9", "aoba"])
                .output();

            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

            // Reset socat to fully release port locks
            setup_virtual_serial_ports()?;

            // Add extended delay to ensure PTY devices are ready
            log::info!("⏳ Waiting for ports to stabilize (15 seconds)...");
            tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;

            // Verify status files are clean
            log::info!("🧪 Verifying status files are reset...");
            if let Ok(status_content) = std::fs::read_to_string("/tmp/ci_tui_status.json") {
                log::debug!("Status file content after cleanup: {}", status_content);
            }
        }
    }

    // Test 2: TUI Master + CLI Slave (repeat for stability)
    if args.should_run_test(2) {
        log::info!("🧪 Test 2/4: TUI Master + CLI Slave - Repeat (10 rounds, holding registers)");
        e2e::test_tui_master_with_cli_slave_continuous(&args.port1, &args.port2).await?;

        // Reset ports after test completes (Unix only)
        #[cfg(not(windows))]
        {
            log::info!("🧪 Resetting virtual serial ports after Test 2...");
            setup_virtual_serial_ports()?;
        }
    }

    // Test 3: Multiple TUI Masters on vcom1
    if args.should_run_test(3) {
        log::info!("🧪 Test 3/4: Multiple TUI Masters on vcom1 (E2E test suite)");
        e2e::test_tui_multi_masters(&args.port1, &args.port2).await?;

        // Reset ports after test completes (Unix only)
        #[cfg(not(windows))]
        {
            log::info!("🧪 Resetting virtual serial ports after Test 3...");
            setup_virtual_serial_ports()?;
        }
    }

    // Test 4: Multiple TUI Slaves on vcom2
    if args.should_run_test(4) {
        log::info!("🧪 Test 4/4: Multiple TUI Slaves on vcom2 (E2E test suite)");
        e2e::test_tui_multi_slaves(&args.port1, &args.port2).await?;
    }

    log::info!("🧪 All selected TUI E2E tests passed!");

    Ok(())
}
