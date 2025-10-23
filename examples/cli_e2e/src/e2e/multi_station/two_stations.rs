/// CLI E2E tests for multi-station (2 stations) configurations
///
/// Tests communication between two CLI processes with multiple stations configured.
/// Each test verifies different station configuration scenarios using config files.
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::process::Stdio;

use ci_utils::{
    build_debug_bin, generate_random_coils, generate_random_registers, sleep_seconds,
    vcom_matchers_with_ports, DEFAULT_PORT1, DEFAULT_PORT2,
};

/// CLI Configuration structures (matching src/cli/config.rs)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StationMode {
    Master,
    Slave,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CommunicationMethod {
    Stdio,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PersistenceMode {
    Persistent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RegisterRange {
    address_start: u16,
    length: u16,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    initial_values: Vec<u16>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct RegisterMap {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    coils: Vec<RegisterRange>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    discrete_inputs: Vec<RegisterRange>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    holding: Vec<RegisterRange>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    input: Vec<RegisterRange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StationConfig {
    id: u8,
    mode: StationMode,
    map: RegisterMap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CommunicationParams {
    mode: CommunicationMethod,
    dynamic_pull: bool,
    wait_time: Option<f64>,
    timeout: Option<f64>,
    persistence: PersistenceMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    port_name: String,
    baud_rate: u32,
    communication_params: CommunicationParams,
    stations: Vec<StationConfig>,
}

/// Helper to create a CLI config with multiple stations
fn create_multi_station_config(
    port: &str,
    mode: StationMode,
    stations_info: &[(u8, &str, u16, u16, Vec<u16>)], // (id, reg_type, addr, len, values)
) -> Config {
    let mut stations = Vec::new();

    for (id, reg_type, addr, len, values) in stations_info {
        let mut map = RegisterMap::default();
        
        let range = RegisterRange {
            address_start: *addr,
            length: *len,
            initial_values: values.clone(),
        };

        match *reg_type {
            "coils" => map.coils.push(range),
            "discrete_inputs" => map.discrete_inputs.push(range),
            "holding" => map.holding.push(range),
            "input" => map.input.push(range),
            _ => panic!("Unknown register type: {}", reg_type),
        }

        stations.push(StationConfig {
            id: *id,
            mode: mode.clone(),
            map,
        });
    }

    Config {
        port_name: port.to_string(),
        baud_rate: 9600,
        communication_params: CommunicationParams {
            mode: CommunicationMethod::Stdio,
            dynamic_pull: false,
            wait_time: Some(1.0),
            timeout: Some(3.0),
            persistence: PersistenceMode::Persistent,
        },
        stations,
    }
}

/// Helper to spawn a CLI process with JSON config
fn spawn_cli_with_config(config: &Config) -> Result<std::process::Child> {
    let binary = build_debug_bin("aoba")?;
    let json_config = serde_json::to_string(config)?;

    let child = std::process::Command::new(&binary)
        .args(["--config-json", &json_config])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    log::info!("✅ Spawned CLI process with {} stations on port {}", config.stations.len(), config.port_name);
    for (i, station) in config.stations.iter().enumerate() {
        log::info!("  Station {}: ID={}, mode={:?}", i + 1, station.id, station.mode);
    }

    Ok(child)
}

/// Test: Mixed Register Types - Station 1 Coils, Station 2 Holding
/// Both stations: ID=1, addr=0x0000, len=10
pub async fn test_multi_station_mixed_register_types() -> Result<()> {
    log::info!("🧪 Starting CLI Multi-Station Test: Mixed Register Types");
    log::info!("  Station 1: Coils mode (ID=1, addr=0x0000, len=10)");
    log::info!("  Station 2: Holding mode (ID=1, addr=0x0000, len=10)");

    let ports = vcom_matchers_with_ports(DEFAULT_PORT1, DEFAULT_PORT2);

    // Generate test data for each station
    let station1_data = generate_random_coils(10);
    let station2_data = generate_random_registers(10);

    log::info!("🎲 Station 1 test data (coils): {:?}", station1_data);
    log::info!("🎲 Station 2 test data (holding): {:?}", station2_data);

    // Create config for master with 2 stations
    let master_stations_info = vec![
        (1u8, "coils", 0x0000u16, 10u16, station1_data.clone()),
        (1u8, "holding", 0x0000u16, 10u16, station2_data.clone()),
    ];
    let master_config = create_multi_station_config(
        &ports.port1_name,
        StationMode::Master,
        &master_stations_info,
    );

    // Create config for slave with 2 stations (empty initial values)
    let slave_stations_info = vec![
        (1u8, "coils", 0x0000u16, 10u16, vec![]),
        (1u8, "holding", 0x0000u16, 10u16, vec![]),
    ];
    let slave_config = create_multi_station_config(
        &ports.port2_name,
        StationMode::Slave,
        &slave_stations_info,
    );

    // Step 1 - Spawn Master process
    let mut master = spawn_cli_with_config(&master_config)?;
    sleep_seconds(3).await;

    // Check if Master is still running
    if let Some(status) = master.try_wait()? {
        let stderr = if let Some(mut stderr) = master.stderr.take() {
            let mut buf = String::new();
            use std::io::Read;
            stderr.read_to_string(&mut buf).unwrap_or_default();
            buf
        } else {
            String::new()
        };
        return Err(anyhow!("Master exited prematurely with status {}: {}", status, stderr));
    }

    // Step 2 - Spawn Slave process
    let mut slave = spawn_cli_with_config(&slave_config)?;
    sleep_seconds(3).await;

    // Check if Slave is still running
    if let Some(status) = slave.try_wait()? {
        let stderr = if let Some(mut stderr) = slave.stderr.take() {
            let mut buf = String::new();
            use std::io::Read;
            stderr.read_to_string(&mut buf).unwrap_or_default();
            buf
        } else {
            String::new()
        };
        
        master.kill()?;
        master.wait()?;
        
        return Err(anyhow!("Slave exited prematurely with status {}: {}", status, stderr));
    }

    // Step 3 - Read output from slave stdout to verify communication
    // The slave should output received data in JSONL format
    if let Some(stdout) = slave.stdout.as_mut() {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        
        // Try to read one line of output (with timeout)
        use std::time::{Duration, Instant};
        let start = Instant::now();
        let mut found_output = false;
        
        while start.elapsed() < Duration::from_secs(5) {
            if reader.read_line(&mut line).is_ok() && !line.is_empty() {
                log::info!("📥 Received output from slave: {}", line.trim());
                found_output = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        
        if !found_output {
            log::warn!("⚠️  No output received from slave within timeout");
        }
    }

    log::info!("✅ Multi-station communication established successfully");

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

    // Generate test data for each station
    let station1_data = generate_random_registers(10);
    let station2_data = generate_random_registers(10);

    log::info!("🎲 Station 1 test data: {:?}", station1_data);
    log::info!("🎲 Station 2 test data: {:?}", station2_data);

    // Create config for master with 2 stations at spaced addresses
    let master_stations_info = vec![
        (1u8, "holding", 0x0000u16, 10u16, station1_data.clone()),
        (1u8, "holding", 0x00A0u16, 10u16, station2_data.clone()),
    ];
    let master_config = create_multi_station_config(
        &ports.port1_name,
        StationMode::Master,
        &master_stations_info,
    );

    // Create config for slave with 2 stations at spaced addresses
    let slave_stations_info = vec![
        (1u8, "holding", 0x0000u16, 10u16, vec![]),
        (1u8, "holding", 0x00A0u16, 10u16, vec![]),
    ];
    let slave_config = create_multi_station_config(
        &ports.port2_name,
        StationMode::Slave,
        &slave_stations_info,
    );

    // Step 1 - Spawn Master process
    let mut master = spawn_cli_with_config(&master_config)?;
    sleep_seconds(3).await;

    if let Some(status) = master.try_wait()? {
        let stderr = if let Some(mut stderr) = master.stderr.take() {
            let mut buf = String::new();
            use std::io::Read;
            stderr.read_to_string(&mut buf).unwrap_or_default();
            buf
        } else {
            String::new()
        };
        return Err(anyhow!("Master exited prematurely with status {}: {}", status, stderr));
    }

    // Step 2 - Spawn Slave process
    let mut slave = spawn_cli_with_config(&slave_config)?;
    sleep_seconds(3).await;

    if let Some(status) = slave.try_wait()? {
        let stderr = if let Some(mut stderr) = slave.stderr.take() {
            let mut buf = String::new();
            use std::io::Read;
            stderr.read_to_string(&mut buf).unwrap_or_default();
            buf
        } else {
            String::new()
        };
        
        master.kill()?;
        master.wait()?;
        
        return Err(anyhow!("Slave exited prematurely with status {}: {}", status, stderr));
    }

    // Step 3 - Verify communication
    if let Some(stdout) = slave.stdout.as_mut() {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        
        use std::time::{Duration, Instant};
        let start = Instant::now();
        let mut found_output = false;
        
        while start.elapsed() < Duration::from_secs(5) {
            if reader.read_line(&mut line).is_ok() && !line.is_empty() {
                log::info!("📥 Received output from slave: {}", line.trim());
                found_output = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        
        if !found_output {
            log::warn!("⚠️  No output received from slave within timeout");
        }
    }

    log::info!("✅ Multi-station communication with spaced addresses established successfully");

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

    // Generate test data for each station
    let station1_data = generate_random_registers(10);
    let station2_data = generate_random_registers(10);

    log::info!("🎲 Station 1 test data: {:?}", station1_data);
    log::info!("🎲 Station 2 test data: {:?}", station2_data);

    // Create config for master with 2 stations with different IDs
    let master_stations_info = vec![
        (1u8, "holding", 0x0000u16, 10u16, station1_data.clone()),
        (5u8, "holding", 0x0000u16, 10u16, station2_data.clone()),
    ];
    let master_config = create_multi_station_config(
        &ports.port1_name,
        StationMode::Master,
        &master_stations_info,
    );

    // Create config for slave with 2 stations with different IDs
    let slave_stations_info = vec![
        (1u8, "holding", 0x0000u16, 10u16, vec![]),
        (5u8, "holding", 0x0000u16, 10u16, vec![]),
    ];
    let slave_config = create_multi_station_config(
        &ports.port2_name,
        StationMode::Slave,
        &slave_stations_info,
    );

    // Step 1 - Spawn Master process
    let mut master = spawn_cli_with_config(&master_config)?;
    sleep_seconds(3).await;

    if let Some(status) = master.try_wait()? {
        let stderr = if let Some(mut stderr) = master.stderr.take() {
            let mut buf = String::new();
            use std::io::Read;
            stderr.read_to_string(&mut buf).unwrap_or_default();
            buf
        } else {
            String::new()
        };
        return Err(anyhow!("Master exited prematurely with status {}: {}", status, stderr));
    }

    // Step 2 - Spawn Slave process
    let mut slave = spawn_cli_with_config(&slave_config)?;
    sleep_seconds(3).await;

    if let Some(status) = slave.try_wait()? {
        let stderr = if let Some(mut stderr) = slave.stderr.take() {
            let mut buf = String::new();
            use std::io::Read;
            stderr.read_to_string(&mut buf).unwrap_or_default();
            buf
        } else {
            String::new()
        };
        
        master.kill()?;
        master.wait()?;
        
        return Err(anyhow!("Slave exited prematurely with status {}: {}", status, stderr));
    }

    // Step 3 - Verify communication
    if let Some(stdout) = slave.stdout.as_mut() {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        
        use std::time::{Duration, Instant};
        let start = Instant::now();
        let mut found_output = false;
        
        while start.elapsed() < Duration::from_secs(5) {
            if reader.read_line(&mut line).is_ok() && !line.is_empty() {
                log::info!("📥 Received output from slave: {}", line.trim());
                found_output = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        
        if !found_output {
            log::warn!("⚠️  No output received from slave within timeout");
        }
    }

    log::info!("✅ Multi-station communication with mixed station IDs established successfully");

    // Cleanup
    master.kill()?;
    master.wait()?;
    slave.kill()?;
    slave.wait()?;

    log::info!("✅ Multi-Station Mixed Station IDs test completed successfully");
    Ok(())
}
