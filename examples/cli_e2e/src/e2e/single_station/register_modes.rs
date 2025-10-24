/// CLI E2E tests for single-station with different register modes
///
/// Tests communication between two CLI processes (Master and Slave) using stdio pipes.
/// Each test covers a specific Modbus register mode with proper read/write verification.
use anyhow::{anyhow, Result};
use std::io::{BufRead, BufReader};
use std::process::Stdio;

use ci_utils::{
    build_debug_bin, generate_random_coils, generate_random_registers, sleep_seconds,
    vcom_matchers_with_ports, DEFAULT_PORT1, DEFAULT_PORT2,
};

/// Helper to spawn a CLI master process with specified parameters
fn spawn_cli_master(
    port: &str,
    station_id: u8,
    register_mode: &str,
    start_address: u16,
    register_count: u16,
    data_source: &str,
) -> Result<std::process::Child> {
    let binary = build_debug_bin("aoba")?;

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
            "--data-source",
            data_source,
            "--debug-ci-e2e-test",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    log::info!(
        "✅ Spawned CLI Master: mode={}, addr=0x{:04X}, count={}, data_source={}",
        register_mode,
        start_address,
        register_count,
        data_source
    );

    Ok(child)
}

/// Helper to spawn a CLI slave process with specified parameters
fn spawn_cli_slave(
    port: &str,
    station_id: u8,
    register_mode: &str,
    start_address: u16,
    register_count: u16,
    output_sink: Option<&str>,
) -> Result<std::process::Child> {
    let binary = build_debug_bin("aoba")?;

    let mut args = vec![
        "--slave-poll-persist".to_string(),
        port.to_string(),
        "--station-id".to_string(),
        station_id.to_string(),
        "--register-mode".to_string(),
        register_mode.to_string(),
        "--register-address".to_string(),
        start_address.to_string(),
        "--register-length".to_string(),
        register_count.to_string(),
        "--baud-rate".to_string(),
        "9600".to_string(),
        "--debug-ci-e2e-test".to_string(),
    ];

    if let Some(output) = output_sink {
        args.push("--output".to_string());
        args.push(output.to_string());
    }

    let child = std::process::Command::new(&binary)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    log::info!(
        "✅ Spawned CLI Slave (polling): mode={}, addr=0x{:04X}, count={}, output={:?}",
        register_mode,
        start_address,
        register_count,
        output_sink
    );

    Ok(child)
}

/// Helper to spawn a CLI slave listener process with specified parameters
fn spawn_cli_slave_listener(
    port: &str,
    station_id: u8,
    register_mode: &str,
    start_address: u16,
    register_count: u16,
    data_source: &str,
) -> Result<std::process::Child> {
    let binary = build_debug_bin("aoba")?;

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
            "--data-source",
            data_source,
            "--debug-ci-e2e-test",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    log::info!(
        "✅ Spawned CLI Slave (listener): mode={}, addr=0x{:04X}, count={}, data_source={}",
        register_mode,
        start_address,
        register_count,
        data_source
    );

    Ok(child)
}

/// Helper to spawn a CLI slave listener process without data source
/// (initializes storage to zeros)
fn spawn_cli_slave_listener_no_data(
    port: &str,
    station_id: u8,
    register_mode: &str,
    start_address: u16,
    register_count: u16,
) -> Result<std::process::Child> {
    let binary = build_debug_bin("aoba")?;

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
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    log::info!(
        "✅ Spawned CLI Slave (listener without data): mode={}, addr=0x{:04X}, count={}",
        register_mode,
        start_address,
        register_count
    );

    Ok(child)
}

/// Helper to write data to a file for master to read
fn write_data_to_file(file_path: &std::path::Path, data: &[u16]) -> Result<()> {
    use std::io::Write;
    let json_data = serde_json::json!({
        "values": data
    });
    let json_str = serde_json::to_string(&json_data)?;
    let mut file = std::fs::File::create(file_path)?;
    writeln!(file, "{}", json_str)?;
    file.flush()?;
    log::info!("📤 Wrote data to file {}: {:?}", file_path.display(), data);
    Ok(())
}

/// Helper to read data from a slave's output file
fn read_data_from_file(file_path: &std::path::Path, timeout_secs: u64) -> Result<Vec<u16>> {
    use std::time::{Duration, Instant};

    let start = Instant::now();
    let timeout = Duration::from_secs(timeout_secs);

    // Wait for file to be created and contain data
    loop {
        if start.elapsed() > timeout {
            return Err(anyhow!(
                "Timeout waiting for data in file {}",
                file_path.display()
            ));
        }

        if file_path.exists() {
            // Try to read the first line
            let file = std::fs::File::open(file_path)?;
            let reader = BufReader::new(file);
            let mut lines = reader.lines();

            if let Some(Ok(line)) = lines.next() {
                // Try to parse the JSON
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                    if let Some(values) = json.get("values") {
                        if let Some(arr) = values.as_array() {
                            let data: Vec<u16> = arr
                                .iter()
                                .filter_map(|v| v.as_u64())
                                .map(|v| v as u16)
                                .collect();
                            if !data.is_empty() {
                                log::info!(
                                    "📥 Read data from file {}: {:?}",
                                    file_path.display(),
                                    data
                                );
                                return Ok(data);
                            }
                        }
                    }
                }
            }
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Test 01: Coils mode (0x0000, length 10)
///
/// Tests basic communication with coil registers (bit values: 0 or 1).
/// Master provides coil data, Slave reads and verifies it.
pub async fn test_single_station_coils() -> Result<()> {
    log::info!("🧪 Starting CLI Single-Station Test: 01 Coils Mode");

    let ports = vcom_matchers_with_ports(DEFAULT_PORT1, DEFAULT_PORT2);
    let station_id = 1;
    let start_address = 0x0000;
    let register_count = 10;
    let register_mode = "coils";

    // Generate test data (coils are 0 or 1)
    let test_data = generate_random_coils(register_count as usize);
    log::info!("🎲 Test data: {:?}", test_data);

    // Create temporary files for data exchange
    let temp_dir = std::env::temp_dir();
    let master_data_file = temp_dir.join("cli_e2e_master_coils_data.json");
    let slave_output_file = temp_dir.join("cli_e2e_slave_coils_output.json");

    // Clean up any existing files
    let _ = std::fs::remove_file(&master_data_file);
    let _ = std::fs::remove_file(&slave_output_file);

    // Write test data to master's data file
    write_data_to_file(&master_data_file, &test_data)?;

    // Step 1 - Spawn Master process on port1
    let mut master = spawn_cli_master(
        &ports.port1_name,
        station_id,
        register_mode,
        start_address,
        register_count,
        &format!("file:{}", master_data_file.display()),
    )?;

    // Give Master time to initialize
    sleep_seconds(2).await;

    // Check if Master is still running
    if let Some(status) = master.try_wait()? {
        // Read stderr for error details
        let stderr = if let Some(mut stderr) = master.stderr.take() {
            let mut buf = String::new();
            use std::io::Read;
            stderr.read_to_string(&mut buf).unwrap_or_default();
            buf
        } else {
            String::new()
        };

        // Cleanup
        let _ = std::fs::remove_file(&master_data_file);

        return Err(anyhow!(
            "Master exited prematurely with status {}: {}",
            status,
            stderr
        ));
    }

    // Step 2 - Spawn Slave process on port2
    let mut slave = spawn_cli_slave(
        &ports.port2_name,
        station_id,
        register_mode,
        start_address,
        register_count,
        Some(&format!("file:{}", slave_output_file.display())),
    )?;

    // Give Slave time to initialize and read data
    sleep_seconds(3).await;

    // Step 3 - Read data from Slave output file
    let received_data = read_data_from_file(&slave_output_file, 10)?;

    // Step 4 - Verify data matches
    if test_data != received_data {
        log::error!("❌ Data mismatch!");
        log::error!("  Expected: {:?}", test_data);
        log::error!("  Received: {:?}", received_data);

        // Cleanup
        master.kill()?;
        master.wait()?;
        slave.kill()?;
        slave.wait()?;
        let _ = std::fs::remove_file(&master_data_file);
        let _ = std::fs::remove_file(&slave_output_file);

        return Err(anyhow!("Data verification failed"));
    }

    log::info!("✅ Data verified successfully!");

    // Cleanup
    master.kill()?;
    master.wait()?;
    slave.kill()?;
    slave.wait()?;
    let _ = std::fs::remove_file(&master_data_file);
    let _ = std::fs::remove_file(&slave_output_file);

    log::info!("✅ Test 01 Coils Mode completed successfully");
    Ok(())
}

/// Test 02: Discrete Inputs mode (0x0010, length 10)
///
/// Tests communication with discrete input registers.
/// Slave listens and provides discrete input data, another Slave polls and reads it.
pub async fn test_single_station_discrete_inputs() -> Result<()> {
    log::info!("🧪 Starting CLI Single-Station Test: 02 Discrete Inputs Mode");

    let ports = vcom_matchers_with_ports(DEFAULT_PORT1, DEFAULT_PORT2);
    let station_id = 1;
    let start_address = 0x0010;
    let register_count = 10;
    let register_mode = "discrete";

    // Expected data: all zeros (slave-listen initializes storage to zeros)
    let expected_data: Vec<u16> = vec![0; register_count as usize];
    log::info!("🎲 Expected data (zeros): {:?}", expected_data);

    // Create temporary file for poller output
    let temp_dir = std::env::temp_dir();
    let poller_output_file = temp_dir.join("cli_e2e_poller_discrete_output.json");

    // Clean up any existing files
    let _ = std::fs::remove_file(&poller_output_file);

    // Step 1 - Spawn Slave Listener on port1 (provides data - starts with zeros)
    let mut listener = spawn_cli_slave_listener_no_data(
        &ports.port1_name,
        station_id,
        register_mode,
        start_address,
        register_count,
    )?;

    sleep_seconds(2).await;

    // Check if Listener is still running
    if let Some(status) = listener.try_wait()? {
        let stderr = if let Some(mut stderr) = listener.stderr.take() {
            let mut buf = String::new();
            use std::io::Read;
            stderr.read_to_string(&mut buf).unwrap_or_default();
            buf
        } else {
            String::new()
        };

        return Err(anyhow!(
            "Listener exited prematurely with status {}: {}",
            status,
            stderr
        ));
    }

    // Step 2 - Spawn Slave Poller on port2 (reads data)
    let mut poller = spawn_cli_slave(
        &ports.port2_name,
        station_id,
        register_mode,
        start_address,
        register_count,
        Some(&format!("file:{}", poller_output_file.display())),
    )?;

    sleep_seconds(3).await;

    // Step 3 - Read data from Poller output and verify
    let received_data = read_data_from_file(&poller_output_file, 10)?;
    if expected_data != received_data {
        log::error!("❌ Data mismatch!");
        log::error!("  Expected: {:?}", expected_data);
        log::error!("  Received: {:?}", received_data);

        // Cleanup
        listener.kill()?;
        listener.wait()?;
        poller.kill()?;
        poller.wait()?;
        let _ = std::fs::remove_file(&poller_output_file);

        return Err(anyhow!("Data verification failed"));
    }
    log::info!("✅ Data verified successfully!");

    // Cleanup
    listener.kill()?;
    listener.wait()?;
    poller.kill()?;
    poller.wait()?;
    let _ = std::fs::remove_file(&poller_output_file);

    log::info!("✅ Test 02 Discrete Inputs Mode completed successfully");
    Ok(())
}

/// Test 03: Holding Registers mode (0x0020, length 10)
///
/// Tests basic communication with holding registers (16-bit values).
/// Master provides register data, Slave reads and verifies it.
pub async fn test_single_station_holding_registers() -> Result<()> {
    log::info!("🧪 Starting CLI Single-Station Test: 03 Holding Registers Mode");

    let ports = vcom_matchers_with_ports(DEFAULT_PORT1, DEFAULT_PORT2);
    let station_id = 1;
    let start_address = 0x0020;
    let register_count = 10;
    let register_mode = "holding";

    // Generate test data (full u16 values)
    let test_data = generate_random_registers(register_count as usize);
    log::info!("🎲 Test data: {:?}", test_data);

    // Create temporary files for data exchange
    let temp_dir = std::env::temp_dir();
    let master_data_file = temp_dir.join("cli_e2e_master_holding_data.json");
    let slave_output_file = temp_dir.join("cli_e2e_slave_holding_output.json");

    // Clean up any existing files
    let _ = std::fs::remove_file(&master_data_file);
    let _ = std::fs::remove_file(&slave_output_file);

    // Write test data to master's data file
    write_data_to_file(&master_data_file, &test_data)?;

    // Step 1 - Spawn Master process on port1
    let mut master = spawn_cli_master(
        &ports.port1_name,
        station_id,
        register_mode,
        start_address,
        register_count,
        &format!("file:{}", master_data_file.display()),
    )?;

    sleep_seconds(2).await;

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

        let _ = std::fs::remove_file(&master_data_file);
        return Err(anyhow!(
            "Master exited prematurely with status {}: {}",
            status,
            stderr
        ));
    }

    // Step 2 - Spawn Slave process on port2
    let mut slave = spawn_cli_slave(
        &ports.port2_name,
        station_id,
        register_mode,
        start_address,
        register_count,
        Some(&format!("file:{}", slave_output_file.display())),
    )?;

    sleep_seconds(3).await;

    // Step 3 - Read data from Slave output file
    let received_data = read_data_from_file(&slave_output_file, 10)?;

    // Step 4 - Verify data matches
    if test_data != received_data {
        log::error!("❌ Data mismatch!");
        log::error!("  Expected: {:?}", test_data);
        log::error!("  Received: {:?}", received_data);

        // Cleanup
        master.kill()?;
        master.wait()?;
        slave.kill()?;
        slave.wait()?;
        let _ = std::fs::remove_file(&master_data_file);
        let _ = std::fs::remove_file(&slave_output_file);

        return Err(anyhow!("Data verification failed"));
    }

    log::info!("✅ Data verified successfully!");

    // Cleanup
    master.kill()?;
    master.wait()?;
    slave.kill()?;
    slave.wait()?;
    let _ = std::fs::remove_file(&master_data_file);
    let _ = std::fs::remove_file(&slave_output_file);

    log::info!("✅ Test 03 Holding Registers Mode completed successfully");
    Ok(())
}

/// Test 04: Input Registers mode (0x0030, length 10)
///
/// Tests communication with input registers.
/// Slave listens and provides input register data (initialized to zeros),
/// another Slave polls and reads it.
/// Note: slave-listen initializes storage to zeros, so this test verifies
/// zero values are correctly communicated.
pub async fn test_single_station_input_registers() -> Result<()> {
    log::info!("🧪 Starting CLI Single-Station Test: 04 Input Registers Mode");

    let ports = vcom_matchers_with_ports(DEFAULT_PORT1, DEFAULT_PORT2);
    let station_id = 1;
    let start_address = 0x0030;
    let register_count = 10;
    let register_mode = "input";

    // Expected data: all zeros (slave-listen initializes storage to zeros)
    let expected_data: Vec<u16> = vec![0; register_count as usize];
    log::info!("🎲 Expected data (zeros): {:?}", expected_data);

    // Create temporary file for poller output
    let temp_dir = std::env::temp_dir();
    let poller_output_file = temp_dir.join("cli_e2e_poller_input_output.json");

    // Clean up any existing files
    let _ = std::fs::remove_file(&poller_output_file);

    // Step 1 - Spawn Slave Listener on port1 (provides data - starts with zeros)
    let mut listener = spawn_cli_slave_listener_no_data(
        &ports.port1_name,
        station_id,
        register_mode,
        start_address,
        register_count,
    )?;

    sleep_seconds(2).await;

    // Check if Listener is still running
    if let Some(status) = listener.try_wait()? {
        let stderr = if let Some(mut stderr) = listener.stderr.take() {
            let mut buf = String::new();
            use std::io::Read;
            stderr.read_to_string(&mut buf).unwrap_or_default();
            buf
        } else {
            String::new()
        };

        return Err(anyhow!(
            "Listener exited prematurely with status {}: {}",
            status,
            stderr
        ));
    }

    // Step 2 - Spawn Slave Poller on port2 (reads data)
    let mut poller = spawn_cli_slave(
        &ports.port2_name,
        station_id,
        register_mode,
        start_address,
        register_count,
        Some(&format!("file:{}", poller_output_file.display())),
    )?;

    sleep_seconds(3).await;

    // Step 3 - Read data from Poller and verify
    let received_data = read_data_from_file(&poller_output_file, 10)?;
    if expected_data != received_data {
        log::error!("❌ Data mismatch!");
        log::error!("  Expected: {:?}", expected_data);
        log::error!("  Received: {:?}", received_data);

        // Cleanup
        listener.kill()?;
        listener.wait()?;
        poller.kill()?;
        poller.wait()?;
        let _ = std::fs::remove_file(&poller_output_file);

        return Err(anyhow!("Data verification failed"));
    }
    log::info!("✅ Data verified successfully!");

    // Cleanup
    listener.kill()?;
    listener.wait()?;
    poller.kill()?;
    poller.wait()?;
    let _ = std::fs::remove_file(&poller_output_file);

    log::info!("✅ Test 04 Input Registers Mode completed successfully");
    Ok(())
}
