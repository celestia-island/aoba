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
            "--slave-listen-persist",
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
pub async fn test_multi_station_mixed_register_types() -> Result<()> {
    log::info!("🧪 Starting CLI Multi-Station Test: Mixed Register Types");
    log::info!("  Station 1: Coils mode (ID=1, addr=0x0000, len=10)");
    log::info!("  Station 2: Holding mode (ID=1, addr=0x0000, len=10)");

    let ports = vcom_matchers_with_ports(DEFAULT_PORT1, DEFAULT_PORT2);

    // Define station configurations
    let station_configs = vec![
        (1u8, "coils", 0x0000u16, 10u16),       // Station 1: Coils
        (1u8, "holding", 0x0000u16, 10u16),     // Station 2: Holding
    ];

    // Generate test data for each station
    let station1_data = generate_random_coils(10);
    let station2_data = generate_random_registers(10);
    let test_data = vec![station1_data.clone(), station2_data.clone()];

    log::info!("🎲 Station 1 test data (coils): {:?}", station1_data);
    log::info!("🎲 Station 2 test data (holding): {:?}", station2_data);

    // TODO: Step 1 - Spawn Master process with multi-station config
    let mut master = spawn_cli_master_multi_station(&ports.port1_name, &station_configs)?;
    sleep_seconds(2).await;

    // Check if Master is still running
    if let Some(status) = master.try_wait()? {
        return Err(anyhow!("Master exited prematurely with status {}", status));
    }

    // TODO: Step 2 - Send data for all stations to Master
    send_multi_station_data(&mut master, &test_data)?;
    sleep_seconds(1).await;

    // TODO: Step 3 - Spawn Slave process with multi-station config
    let mut slave = spawn_cli_slave_multi_station(&ports.port2_name, &station_configs)?;
    sleep_seconds(3).await;

    // TODO: Step 4 - Read data from Slave for all stations
    let received_data = read_multi_station_data(&mut slave, station_configs.len())?;

    // TODO: Step 5 - Verify data for each station
    for (i, (sent, received)) in test_data.iter().zip(received_data.iter()).enumerate() {
        if sent != received {
            log::error!("❌ Station {} data mismatch!", i + 1);
            log::error!("  Expected: {:?}", sent);
            log::error!("  Received: {:?}", received);
            return Err(anyhow!("Station {} data verification failed", i + 1));
        }
        log::info!("✅ Station {} data verified", i + 1);
    }

    // Cleanup
    master.kill()?;
    master.wait()?;
    slave.kill()?;
    slave.wait()?;

    log::info!("✅ Multi-Station Mixed Register Types test completed successfully");
    Ok(())
}

/// Test: Spaced Addresses - Station 1 at 0x0000, Station 2 at 0x00A0
/// Both stations: Holding mode, ID=1, len=10
pub async fn test_multi_station_spaced_addresses() -> Result<()> {
    log::info!("🧪 Starting CLI Multi-Station Test: Spaced Addresses");
    log::info!("  Station 1: Holding mode (ID=1, addr=0x0000, len=10)");
    log::info!("  Station 2: Holding mode (ID=1, addr=0x00A0, len=10)");

    let ports = vcom_matchers_with_ports(DEFAULT_PORT1, DEFAULT_PORT2);

    // Define station configurations with spaced addresses
    let station_configs = vec![
        (1u8, "holding", 0x0000u16, 10u16),     // Station 1: addr 0x0000
        (1u8, "holding", 0x00A0u16, 10u16),     // Station 2: addr 0x00A0 (160 in decimal)
    ];

    // Generate test data for each station
    let station1_data = generate_random_registers(10);
    let station2_data = generate_random_registers(10);
    let test_data = vec![station1_data.clone(), station2_data.clone()];

    log::info!("🎲 Station 1 test data: {:?}", station1_data);
    log::info!("🎲 Station 2 test data: {:?}", station2_data);

    // TODO: Step 1 - Spawn Master process
    let mut master = spawn_cli_master_multi_station(&ports.port1_name, &station_configs)?;
    sleep_seconds(2).await;

    if let Some(status) = master.try_wait()? {
        return Err(anyhow!("Master exited prematurely with status {}", status));
    }

    // TODO: Step 2 - Send data for all stations
    send_multi_station_data(&mut master, &test_data)?;
    sleep_seconds(1).await;

    // TODO: Step 3 - Spawn Slave process
    let mut slave = spawn_cli_slave_multi_station(&ports.port2_name, &station_configs)?;
    sleep_seconds(3).await;

    // TODO: Step 4 - Read data from Slave
    let received_data = read_multi_station_data(&mut slave, station_configs.len())?;

    // TODO: Step 5 - Verify data for each station
    for (i, (sent, received)) in test_data.iter().zip(received_data.iter()).enumerate() {
        if sent != received {
            log::error!("❌ Station {} data mismatch!", i + 1);
            return Err(anyhow!("Station {} verification failed", i + 1));
        }
        log::info!("✅ Station {} data verified", i + 1);
    }

    // Cleanup
    master.kill()?;
    master.wait()?;
    slave.kill()?;
    slave.wait()?;

    log::info!("✅ Multi-Station Spaced Addresses test completed successfully");
    Ok(())
}

/// Test: Mixed Station IDs - Station ID=1 and Station ID=5
/// Both stations: Holding mode, addr=0x0000, len=10
pub async fn test_multi_station_mixed_station_ids() -> Result<()> {
    log::info!("🧪 Starting CLI Multi-Station Test: Mixed Station IDs");
    log::info!("  Station 1: Holding mode (ID=1, addr=0x0000, len=10)");
    log::info!("  Station 2: Holding mode (ID=5, addr=0x0000, len=10)");

    let ports = vcom_matchers_with_ports(DEFAULT_PORT1, DEFAULT_PORT2);

    // Define station configurations with different IDs
    let station_configs = vec![
        (1u8, "holding", 0x0000u16, 10u16),     // Station ID 1
        (5u8, "holding", 0x0000u16, 10u16),     // Station ID 5
    ];

    // Generate test data for each station
    let station1_data = generate_random_registers(10);
    let station2_data = generate_random_registers(10);
    let test_data = vec![station1_data.clone(), station2_data.clone()];

    log::info!("🎲 Station 1 test data: {:?}", station1_data);
    log::info!("🎲 Station 2 test data: {:?}", station2_data);

    // TODO: Step 1 - Spawn Master process
    let mut master = spawn_cli_master_multi_station(&ports.port1_name, &station_configs)?;
    sleep_seconds(2).await;

    if let Some(status) = master.try_wait()? {
        return Err(anyhow!("Master exited prematurely with status {}", status));
    }

    // TODO: Step 2 - Send data for all stations
    send_multi_station_data(&mut master, &test_data)?;
    sleep_seconds(1).await;

    // TODO: Step 3 - Spawn Slave process
    let mut slave = spawn_cli_slave_multi_station(&ports.port2_name, &station_configs)?;
    sleep_seconds(3).await;

    // TODO: Step 4 - Read data from Slave
    let received_data = read_multi_station_data(&mut slave, station_configs.len())?;

    // TODO: Step 5 - Verify data for each station
    for (i, (sent, received)) in test_data.iter().zip(received_data.iter()).enumerate() {
        if sent != received {
            log::error!("❌ Station {} data mismatch!", i + 1);
            return Err(anyhow!("Station {} verification failed", i + 1));
        }
        log::info!("✅ Station {} data verified", i + 1);
    }

    // Cleanup
    master.kill()?;
    master.wait()?;
    slave.kill()?;
    slave.wait()?;

    log::info!("✅ Multi-Station Mixed Station IDs test completed successfully");
    Ok(())
}
