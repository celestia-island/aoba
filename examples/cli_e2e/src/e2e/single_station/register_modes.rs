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
            "--debug-ci-e2e-test",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    log::info!(
        "✅ Spawned CLI Master: mode={}, addr=0x{:04X}, count={}",
        register_mode,
        start_address,
        register_count
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
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    log::info!(
        "✅ Spawned CLI Slave: mode={}, addr=0x{:04X}, count={}",
        register_mode,
        start_address,
        register_count
    );

    Ok(child)
}

/// Helper to send data to a CLI process via stdin
fn send_data_to_cli(child: &mut std::process::Child, data: &[u16]) -> Result<()> {
    if let Some(stdin) = child.stdin.as_mut() {
        // Send data as JSON array
        let json_data = serde_json::to_string(data)?;
        use std::io::Write;
        writeln!(stdin, "{}", json_data)?;
        stdin.flush()?;
        log::info!("📤 Sent data to CLI: {:?}", data);
        Ok(())
    } else {
        Err(anyhow!("Failed to get stdin handle for CLI process"))
    }
}

/// Helper to read data from a CLI process stdout
fn read_data_from_cli(child: &mut std::process::Child) -> Result<Vec<u16>> {
    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        // Read first line of JSON output
        if let Some(Ok(line)) = lines.next() {
            let data: Vec<u16> = serde_json::from_str(&line)?;
            log::info!("📥 Received data from CLI: {:?}", data);
            return Ok(data);
        }
    }

    Err(anyhow!("Failed to read data from CLI stdout"))
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

    // TODO: Step 1 - Spawn Master process on port1
    let mut master = spawn_cli_master(
        &ports.port1_name,
        station_id,
        register_mode,
        start_address,
        register_count,
    )?;

    // Give Master time to initialize
    sleep_seconds(2).await;

    // Check if Master is still running
    if let Some(status) = master.try_wait()? {
        return Err(anyhow!("Master exited prematurely with status {}", status));
    }

    // TODO: Step 2 - Send data to Master via stdin
    send_data_to_cli(&mut master, &test_data)?;

    // Give Master time to process data
    sleep_seconds(1).await;

    // TODO: Step 3 - Spawn Slave process on port2
    let mut slave = spawn_cli_slave(
        &ports.port2_name,
        station_id,
        register_mode,
        start_address,
        register_count,
    )?;

    // Give Slave time to initialize and read data
    sleep_seconds(3).await;

    // TODO: Step 4 - Read data from Slave stdout
    let received_data = read_data_from_cli(&mut slave)?;

    // TODO: Step 5 - Verify data matches
    if test_data != received_data {
        log::error!("❌ Data mismatch!");
        log::error!("  Expected: {:?}", test_data);
        log::error!("  Received: {:?}", received_data);
        return Err(anyhow!("Data verification failed"));
    }

    log::info!("✅ Data verified successfully!");

    // Cleanup
    master.kill()?;
    master.wait()?;
    slave.kill()?;
    slave.wait()?;

    log::info!("✅ Test 01 Coils Mode completed successfully");
    Ok(())
}

/// Test 02: Discrete Inputs / Writable Coils mode (0x0010, length 10)
///
/// Tests bidirectional communication with discrete input/coil registers.
/// First verifies Master -> Slave, then tests Slave -> Master write.
pub async fn test_single_station_discrete_inputs() -> Result<()> {
    log::info!("🧪 Starting CLI Single-Station Test: 02 Discrete Inputs/Writable Coils Mode");

    let ports = vcom_matchers_with_ports(DEFAULT_PORT1, DEFAULT_PORT2);
    let station_id = 1;
    let start_address = 0x0010;
    let register_count = 10;
    let register_mode = "discrete_inputs";

    // Generate test data
    let test_data = generate_random_coils(register_count as usize);
    log::info!("🎲 Initial test data: {:?}", test_data);

    // TODO: Step 1 - Spawn Master process on port1
    let mut master = spawn_cli_master(
        &ports.port1_name,
        station_id,
        register_mode,
        start_address,
        register_count,
    )?;

    sleep_seconds(2).await;

    // TODO: Step 2 - Send data to Master
    send_data_to_cli(&mut master, &test_data)?;
    sleep_seconds(1).await;

    // TODO: Step 3 - Spawn Slave process on port2
    let mut slave = spawn_cli_slave(
        &ports.port2_name,
        station_id,
        register_mode,
        start_address,
        register_count,
    )?;

    sleep_seconds(3).await;

    // TODO: Step 4 - Read data from Slave and verify
    let received_data = read_data_from_cli(&mut slave)?;
    if test_data != received_data {
        log::error!("❌ Phase 1 (Master->Slave) data mismatch!");
        return Err(anyhow!("Phase 1 verification failed"));
    }
    log::info!("✅ Phase 1 (Master->Slave) verified");

    // TODO: Step 5 - Test bidirectional write (Slave sends write command to Master)
    let write_data = generate_random_coils(register_count as usize);
    log::info!("🎲 Write test data: {:?}", write_data);
    send_data_to_cli(&mut slave, &write_data)?;
    sleep_seconds(2).await;

    // TODO: Step 6 - Verify Master received and updated values
    let master_updated_data = read_data_from_cli(&mut master)?;
    if write_data != master_updated_data {
        log::error!("❌ Phase 2 (Slave->Master write) data mismatch!");
        return Err(anyhow!("Phase 2 verification failed"));
    }
    log::info!("✅ Phase 2 (Slave->Master write) verified");

    // Cleanup
    master.kill()?;
    master.wait()?;
    slave.kill()?;
    slave.wait()?;

    log::info!("✅ Test 02 Discrete Inputs/Writable Coils Mode completed successfully");
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

    // TODO: Step 1 - Spawn Master process on port1
    let mut master = spawn_cli_master(
        &ports.port1_name,
        station_id,
        register_mode,
        start_address,
        register_count,
    )?;

    sleep_seconds(2).await;

    // TODO: Step 2 - Send data to Master via stdin
    send_data_to_cli(&mut master, &test_data)?;
    sleep_seconds(1).await;

    // TODO: Step 3 - Spawn Slave process on port2
    let mut slave = spawn_cli_slave(
        &ports.port2_name,
        station_id,
        register_mode,
        start_address,
        register_count,
    )?;

    sleep_seconds(3).await;

    // TODO: Step 4 - Read data from Slave stdout
    let received_data = read_data_from_cli(&mut slave)?;

    // TODO: Step 5 - Verify data matches
    if test_data != received_data {
        log::error!("❌ Data mismatch!");
        log::error!("  Expected: {:?}", test_data);
        log::error!("  Received: {:?}", received_data);
        return Err(anyhow!("Data verification failed"));
    }

    log::info!("✅ Data verified successfully!");

    // Cleanup
    master.kill()?;
    master.wait()?;
    slave.kill()?;
    slave.wait()?;

    log::info!("✅ Test 03 Holding Registers Mode completed successfully");
    Ok(())
}

/// Test 04: Input Registers / Writable Registers mode (0x0030, length 10)
///
/// Tests bidirectional communication with input registers.
/// First verifies Master -> Slave, then tests Slave -> Master write.
pub async fn test_single_station_input_registers() -> Result<()> {
    log::info!("🧪 Starting CLI Single-Station Test: 04 Input Registers/Writable Registers Mode");

    let ports = vcom_matchers_with_ports(DEFAULT_PORT1, DEFAULT_PORT2);
    let station_id = 1;
    let start_address = 0x0030;
    let register_count = 10;
    let register_mode = "input";

    // Generate test data
    let test_data = generate_random_registers(register_count as usize);
    log::info!("🎲 Initial test data: {:?}", test_data);

    // TODO: Step 1 - Spawn Master process on port1
    let mut master = spawn_cli_master(
        &ports.port1_name,
        station_id,
        register_mode,
        start_address,
        register_count,
    )?;

    sleep_seconds(2).await;

    // TODO: Step 2 - Send data to Master
    send_data_to_cli(&mut master, &test_data)?;
    sleep_seconds(1).await;

    // TODO: Step 3 - Spawn Slave process on port2
    let mut slave = spawn_cli_slave(
        &ports.port2_name,
        station_id,
        register_mode,
        start_address,
        register_count,
    )?;

    sleep_seconds(3).await;

    // TODO: Step 4 - Read data from Slave and verify
    let received_data = read_data_from_cli(&mut slave)?;
    if test_data != received_data {
        log::error!("❌ Phase 1 (Master->Slave) data mismatch!");
        return Err(anyhow!("Phase 1 verification failed"));
    }
    log::info!("✅ Phase 1 (Master->Slave) verified");

    // TODO: Step 5 - Test bidirectional write (Slave sends write command to Master)
    let write_data = generate_random_registers(register_count as usize);
    log::info!("🎲 Write test data: {:?}", write_data);
    send_data_to_cli(&mut slave, &write_data)?;
    sleep_seconds(2).await;

    // TODO: Step 6 - Verify Master received and updated values
    let master_updated_data = read_data_from_cli(&mut master)?;
    if write_data != master_updated_data {
        log::error!("❌ Phase 2 (Slave->Master write) data mismatch!");
        return Err(anyhow!("Phase 2 verification failed"));
    }
    log::info!("✅ Phase 2 (Slave->Master write) verified");

    // Cleanup
    master.kill()?;
    master.wait()?;
    slave.kill()?;
    slave.wait()?;

    log::info!("✅ Test 04 Input Registers/Writable Registers Mode completed successfully");
    Ok(())
}
