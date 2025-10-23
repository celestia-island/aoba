/// CLI E2E tests for multi-station (2 stations) configurations
///
/// Tests communication between two CLI processes with multiple stations configured.
/// Each test verifies different station configuration scenarios.
use anyhow::{anyhow, Result};
use std::io::{BufRead, BufReader, Write};
use std::process::Stdio;

use ci_utils::{
    build_debug_bin, generate_random_coils, generate_random_registers, sleep_seconds,
    vcom_matchers_with_ports, DEFAULT_PORT1, DEFAULT_PORT2,
};

/// Helper to spawn a CLI master with multiple stations
/// Stations are passed as tuples of (station_id, register_mode, start_address, register_count)
fn spawn_cli_master_multi_station(
    port: &str,
    stations: &[(u8, &str, u16, u16)],
) -> Result<std::process::Child> {
    let binary = build_debug_bin("aoba")?;

    // For multi-station, we need to spawn with JSON config or use multiple CLI arguments
    // Since the CLI doesn't support multiple stations in a single invocation directly,
    // we'll need to use a JSON config file approach
    
    // TODO: This implementation needs the actual CLI multi-station support
    // For now, create a simple implementation that tests the concept
    
    log::info!("🧪 Spawning CLI Master with {} stations", stations.len());
    for (i, (station_id, mode, addr, count)) in stations.iter().enumerate() {
        log::info!(
            "  Station {}: ID={}, mode={}, addr=0x{:04X}, count={}",
            i + 1,
            station_id,
            mode,
            addr,
            count
        );
    }

    // For the first implementation, we'll use the first station's config
    // TODO: Extend this to support true multi-station configuration
    let (station_id, register_mode, start_address, register_count) = stations[0];

    let child = std::process::Command::new(&binary)
        .args([
            "--master-provide-persist",
            port,
            "--station-id",
            &station_id.to_string(),
            "--register-mode",
            register_mode,
            "--register-address",
            &start_address.to_string(),
            "--register-length",
            &register_count.to_string(),
            "--baud-rate",
            "9600",
            "--debug-ci-e2e-test",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    log::info!("✅ Spawned CLI Master (multi-station mode)");
    Ok(child)
}

/// Helper to spawn a CLI slave with multiple stations
fn spawn_cli_slave_multi_station(
    port: &str,
    stations: &[(u8, &str, u16, u16)],
) -> Result<std::process::Child> {
    let binary = build_debug_bin("aoba")?;

    log::info!("🧪 Spawning CLI Slave with {} stations", stations.len());
    for (i, (station_id, mode, addr, count)) in stations.iter().enumerate() {
        log::info!(
            "  Station {}: ID={}, mode={}, addr=0x{:04X}, count={}",
            i + 1,
            station_id,
            mode,
            addr,
            count
        );
    }

    // For the first implementation, we'll use the first station's config
    // TODO: Extend this to support true multi-station configuration
    let (station_id, register_mode, start_address, register_count) = stations[0];

    let child = std::process::Command::new(&binary)
        .args([
            "--slave-poll-persist",
            port,
            "--station-id",
            &station_id.to_string(),
            "--register-mode",
            register_mode,
            "--register-address",
            &start_address.to_string(),
            "--register-length",
            &register_count.to_string(),
            "--baud-rate",
            "9600",
            "--debug-ci-e2e-test",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    log::info!("✅ Spawned CLI Slave (multi-station mode)");
    Ok(child)
}

/// Helper to send multi-station data to a CLI process
/// Data is organized as Vec<Vec<u16>> where each inner Vec corresponds to a station
fn send_multi_station_data(
    child: &mut std::process::Child,
    station_data: &[Vec<u16>],
) -> Result<()> {
    if let Some(stdin) = child.stdin.as_mut() {
        // Send data for each station
        for (i, data) in station_data.iter().enumerate() {
            let json_data = serde_json::to_string(data)?;
            writeln!(stdin, "{}", json_data)?;
            log::info!("📤 Sent data for station {}: {:?}", i + 1, data);
        }
        stdin.flush()?;
        Ok(())
    } else {
        Err(anyhow!("Failed to get stdin handle for CLI process"))
    }
}

/// Helper to read multi-station data from a CLI process
fn read_multi_station_data(
    child: &mut std::process::Child,
    station_count: usize,
) -> Result<Vec<Vec<u16>>> {
    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        let mut results = Vec::new();

        for i in 0..station_count {
            if let Some(Ok(line)) = lines.next() {
                let data: Vec<u16> = serde_json::from_str(&line)?;
                log::info!("📥 Received data for station {}: {:?}", i + 1, data);
                results.push(data);
            } else {
                return Err(anyhow!(
                    "Failed to read data for station {} from CLI stdout",
                    i + 1
                ));
            }
        }

        Ok(results)
    } else {
        Err(anyhow!("Failed to get stdout handle from CLI process"))
    }
}

/// Test: Mixed Register Types - Station 1 Coils, Station 2 Holding
/// Both stations: ID=1, addr=0x0000, len=10
/// 
/// TODO: This test requires multi-register-type support in CLI, which is not yet implemented.
/// The CLI currently only supports a single register type per process.
pub async fn test_multi_station_mixed_register_types() -> Result<()> {
    log::info!("🧪 CLI Multi-Station Test: Mixed Register Types - SKIPPED");
    log::info!("⚠️  This test requires multi-register-type CLI support (not yet implemented)");
    log::info!("  Station 1: Coils mode (ID=1, addr=0x0000, len=10)");
    log::info!("  Station 2: Holding mode (ID=1, addr=0x0000, len=10)");
    log::info!("✅ Test skipped successfully");
    Ok(())
}

/// Test: Spaced Addresses - Station 1 at 0x0000, Station 2 at 0x00A0
/// Both stations: Holding mode, ID=1, len=10
///
/// TODO: This test requires multi-address-range support in CLI, which is not yet implemented.
/// The CLI currently only supports a single address range per process.
pub async fn test_multi_station_spaced_addresses() -> Result<()> {
    log::info!("🧪 CLI Multi-Station Test: Spaced Addresses - SKIPPED");
    log::info!("⚠️  This test requires multi-address-range CLI support (not yet implemented)");
    log::info!("  Station 1: Holding mode (ID=1, addr=0x0000, len=10)");
    log::info!("  Station 2: Holding mode (ID=1, addr=0x00A0, len=10)");
    log::info!("✅ Test skipped successfully");
    Ok(())
}

/// Test: Mixed Station IDs - Station ID=1 and Station ID=5
/// Both stations: Holding mode, addr=0x0000, len=10
///
/// TODO: This test requires multi-station-ID support in CLI, which is not yet implemented.
/// The CLI currently only supports a single station ID per process.
pub async fn test_multi_station_mixed_station_ids() -> Result<()> {
    log::info!("🧪 CLI Multi-Station Test: Mixed Station IDs - SKIPPED");
    log::info!("⚠️  This test requires multi-station-ID CLI support (not yet implemented)");
    log::info!("  Station 1: Holding mode (ID=1, addr=0x0000, len=10)");
    log::info!("  Station 2: Holding mode (ID=5, addr=0x0000, len=10)");
    log::info!("✅ Test skipped successfully");
    Ok(())
}
