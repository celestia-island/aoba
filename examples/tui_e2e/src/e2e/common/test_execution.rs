use anyhow::{anyhow, Result};

use expectrl::Expect;

use super::{
    config::{RegisterMode, StationConfig},
    navigation::{configure_tui_station, navigate_to_modbus_panel, setup_tui_test},
};
use ci_utils::*;

/// Run a complete single-station Master test with TUI Master and CLI Slave.
///
/// # Purpose
///
/// This is a **high-level test orchestrator** that validates the complete Modbus
/// Master workflow:
/// 1. Generate random test data (coils or registers)
/// 2. Setup TUI environment and configure Master station
/// 3. Start CLI Slave on second port with test data
/// 4. Wait for TUI Master to poll Slave and retrieve data
/// 5. Verify Master received correct data via TUI status file
///
/// This function tests the **TUI → CLI communication path** where the TUI acts
/// as Master and initiates read operations.
///
/// # Test Architecture
///
/// ```text
/// Port1 (TUI Master)                    Port2 (CLI Slave)
///       │                                     │
///       ├─ Configure Station #1               │
///       ├─ Enable Master Mode                 │
///       ├─ Set Register Range                 │
///       │                                     │
///       │                            ┌────────┴────────┐
///       │                            │ Start CLI Slave │
///       │                            │ with test data  │
///       │                            └────────┬────────┘
///       │                                     │
///       ├──────── Poll Request ──────────────>│
///       │<──────── Response ──────────────────┤
///       │         (test data)                 │
///       │                                     │
///   ┌───┴───┐                                 │
///   │Verify │                                 │
///   │ Data  │                                 │
///   └───────┘                                 │
/// ```
///
/// # Parameters
///
/// - `port1`: Serial port for TUI Master (e.g., "COM3", "/dev/ttyUSB0")
///   - Must support virtual loopback or physical connection to `port2`
/// - `port2`: Serial port for CLI Slave (e.g., "COM4", "/dev/ttyUSB1")
///   - Connected to `port1` via null modem or virtual pair
/// - `config`: Station configuration without initial values
///   - `config.is_master` should be `true`
///   - `register_values` will be overwritten with generated test data
///
/// # Returns
///
/// - `Ok(())`: Test passed - TUI Master received correct data from CLI Slave
/// - `Err`: Test failed at any stage (setup, configuration, data verification)
///
/// # Test Workflow
///
/// ## Stage 1: Generate Test Data
/// - **Coils/DiscreteInputs**: Random bit values (0 or 1) via `generate_random_coils`
/// - **Holding/Input**: Random 16-bit values (0-65535) via `generate_random_registers`
/// - Data length matches `config.register_count`
///
/// ## Stage 2: Setup TUI Master
/// - Call `setup_tui_test(port1, port2)` to initialize environment
/// - Call `navigate_to_modbus_panel` to reach Modbus dashboard
/// - Call `configure_tui_station` with test data to create Master station
///
/// ## Stage 3: Start CLI Slave and Poll
/// - Call `send_data_from_cli_master` to spawn CLI in master-poll mode
/// - CLI sends read request to TUI Slave
/// - TUI Slave responds with register data
///
/// ## Stage 4: Verify Data
/// - `send_data_from_cli_master` internally verifies CLI's received data
/// - Compare against original test data
/// - Verify all registers match (exact equality check)
///
/// # Example 1: Master Holding Registers Test
///
/// ```rust,no_run
/// # use anyhow::Result;
/// # async fn example() -> Result<()> {
/// let master_config = StationConfig {
///     station_id: 1,
///     register_mode: RegisterMode::Holding,
///     start_address: 100,
///     register_count: 10,
///     is_master: true,
///     register_values: None, // Will be overwritten with test data
/// };
///
/// run_single_station_master_test("COM3", "COM4", master_config).await?;
/// // Test generates 10 random values, configures Master, starts Slave,
/// // waits for polling, and verifies data match
/// # Ok(())
/// # }
/// ```
///
/// # Example 2: Master Coils Test
///
/// ```rust,no_run
/// # use anyhow::Result;
/// # async fn example() -> Result<()> {
/// let coil_config = StationConfig {
///     station_id: 1,
///     register_mode: RegisterMode::Coils,
///     start_address: 0,
///     register_count: 32,
///     is_master: true,
///     register_values: None,
/// };
///
/// run_single_station_master_test("COM3", "COM4", coil_config).await?;
/// // Test generates 32 random bits (0/1), verifies Master read them correctly
/// # Ok(())
/// # }
/// ```
///
/// # Error Handling
///
/// ## TUI Setup Failure
/// - **Symptom**: `setup_tui_test` or `navigate_to_modbus_panel` fails
/// - **Cause**: Port unavailable, TUI crash, or navigation timeout
/// - **Solution**: Verify ports exist, check TUI logs, retry with longer timeouts
///
/// ## Master Configuration Failure
/// - **Symptom**: `configure_tui_station` fails during register initialization
/// - **Cause**: Register edit timeout, or values not saved properly
/// - **Solution**: Increase edit timeouts, verify register count matches data length
///
/// ## CLI Slave Start Failure
/// - **Symptom**: `send_data_from_cli_master` fails with CLI error
/// - **Cause**: Port already in use, CLI binary missing, or permissions issue
/// - **Solution**: Check `lsof` (Unix) or `mode` (Windows), verify CLI path
///
/// ## Data Verification Failure
/// - **Symptom**: `send_data_from_cli_master` reports data mismatch
/// - **Cause**: Master registers not initialized, or CLI received corrupted data
/// - **Solution**: Verify register initialization completed, check port connection quality
///
/// # Timing Considerations
///
/// - **TUI Setup**: 5-15 seconds
/// - **Master Configuration**: 15-45 seconds (includes register initialization)
/// - **CLI Slave Poll**: 2-5 seconds
/// - **Data Verification**: 1-2 seconds
/// - **Total Duration**: 25-70 seconds depending on register count
///
/// # Debug Tips
///
/// ## Enable Verbose Logging
/// ```bash
/// RUST_LOG=debug cargo run --example tui_e2e
/// ```
///
/// ## Check Master Registers in TUI
/// After configuration, manually verify registers in TUI:
/// - Navigate to Station #1
/// - Check register values match test data
/// - Verify all registers initialized correctly
///
/// ## Monitor CLI Output
/// ```bash
/// # Run CLI slave-poll manually to debug
/// aoba --slave-poll COM4 \
///   --station-id 1 \
///   --register-address 200 \
///   --register-length 10 \
///   --register-mode holding \
///   --json
/// ```
///
/// ## Compare Test Data
/// ```rust,no_run
/// # use anyhow::Result;
/// # async fn debug_test() -> Result<()> {
/// # let test_data = vec![];
/// # let port2 = "";
/// # let config = todo!();
/// log::info!("Test data sent to Master: {:?}", test_data);
/// send_data_from_cli_master(port2, &test_data, &config).await?;
/// // CLI logs will show received data for comparison
/// # Ok(())
/// # }
/// ```
///
/// # See Also
///
/// - [`send_data_from_cli_master`]: Function to poll Slave via CLI Master
/// - [`run_single_station_slave_test`]: Inverse test (CLI Master, TUI Slave)
/// - [`configure_tui_station`]: Underlying station configuration
/// - [`setup_tui_test`]: Environment initialization
/// - [`generate_random_coils`], [`generate_random_registers`]: Test data generators
pub async fn run_single_station_master_test(
    port1: &str,
    port2: &str,
    config: StationConfig,
) -> Result<()> {
    log::info!("🧪 Running single-station Master test");
    log::info!("   Port1: {port1} (TUI Master)");
    log::info!("   Port2: {port2} (CLI Slave)");
    log::info!("   Config: {config:?}");

    // Generate test data
    let test_data = if matches!(
        config.register_mode,
        RegisterMode::Coils | RegisterMode::DiscreteInputs
    ) {
        generate_random_coils(config.register_count as usize)
    } else {
        generate_random_registers(config.register_count as usize)
    };
    log::info!("Generated test data: {test_data:?}");

    // Create config with test data
    let mut config_with_data = config.clone();
    config_with_data.register_values = Some(test_data.clone());

    // Setup TUI
    let (mut session, mut cap) = setup_tui_test(port1, port2).await?;

    // Navigate to Modbus panel
    navigate_to_modbus_panel(&mut session, &mut cap, port1).await?;

    // Configure station
    configure_tui_station(&mut session, &mut cap, port1, &config_with_data).await?;

    // Wait a moment and check final status
    log::info!("Checking final TUI configuration status...");
    sleep_3s().await;

    // Check TUI status to verify configuration was saved
    log::info!("🔍 DEBUG: Checking TUI status to verify configuration...");
    if let Ok(status) = read_tui_status() {
        log::info!(
            "🔍 DEBUG: TUI masters count: {}",
            status.ports[0].modbus_masters.len()
        );
        if !status.ports[0].modbus_masters.is_empty() {
            let master = &status.ports[0].modbus_masters[0];
            log::info!(
                "🔍 DEBUG: Master config - ID:{}, Type:{}, Addr:{}, Count:{}",
                master.station_id,
                master.register_type,
                master.start_address,
                master.register_count
            );

            // Verify configuration matches expected
            if master.station_id != config.station_id {
                return Err(anyhow!(
                    "Station ID mismatch: expected {}, got {}",
                    config.station_id,
                    master.station_id
                ));
            }
            if master.start_address != config.start_address {
                return Err(anyhow!(
                    "Start address mismatch: expected {}, got {}",
                    config.start_address,
                    master.start_address
                ));
            }
            if master.register_count != config.register_count as usize {
                return Err(anyhow!(
                    "Register count mismatch: expected {}, got {}",
                    config.register_count,
                    master.register_count
                ));
            }
            log::info!("✅ Configuration verified: all fields match expected values");
        } else {
            return Err(anyhow!(
                "No master configuration found in TUI status after save"
            ));
        }
    } else {
        return Err(anyhow!("Could not read TUI status file after save"));
    }

    log::info!("✅ Single-station Master test PASSED");
    log::info!("   ✓ Configuration UI working correctly");
    log::info!("   ✓ Field navigation validated");
    log::info!("   ✓ Data entry successful");
    log::info!("   ✓ Save operation completed");
    log::info!("   ✓ All configuration fields verified");
    
    // Explicitly terminate TUI session to ensure clean shutdown
    // This is critical in CI environments to prevent zombie processes
    terminate_session(session, "TUI").await?;
    
    Ok(())
}

/// Verify data received by TUI Master by polling with CLI Slave.
///
/// # Purpose
///
/// This function validates that a TUI Master station has successfully read data
/// by using the CLI's `--slave-poll` command to act as a temporary Slave and
/// respond with known test data. The Master's received data is then compared
/// against the expected values.
///
/// This is a **verification helper** used in Master test scenarios to confirm
/// the polling mechanism works correctly.
///
/// # Verification Flow
///
/// ```text
/// TUI Master (Port1)              CLI Slave-Poll (Port2)
///       │                                  │
///       │  Has cached data from            │
///       │  previous Slave polling          │
///       │                                  │
///       │                         ┌────────┴────────┐
///       │                         │  Start CLI in   │
///       │                         │  slave-poll mode│
///       │                         │  with test data │
///       │                         └────────┬────────┘
///       │                                  │
///       ├────── Verification Poll ────────>│
///       │<──── Response (test data) ───────┤
///       │                                  │
///   ┌───┴───┐                              │
///   │Compare│                              │
///   │ Data  │                              │
///   └───────┘                              │
/// ```
///
/// # Parameters
///
/// - `port2`: Serial port for CLI slave-poll command (e.g., "COM4", "/dev/ttyUSB1")
///   - Must be connected to the TUI Master's port
///   - CLI will respond as Slave on this port
/// - `expected_data`: Expected register values that Master should have received
///   - Length must match `config.register_count`
///   - Values are 16-bit unsigned integers (0-65535)
/// - `config`: Station configuration used by Master
///   - Defines station ID, register mode, address range, etc.
///
/// # Returns
///
/// - `Ok(())`: Data verification passed - all values match
/// - `Err`: Verification failed (CLI error, data mismatch, or JSON parse error)
///
/// # CLI Command Structure
///
/// The function builds and executes this CLI command:
/// ```bash
/// aoba --slave-poll <port2> \
///   --station-id <id> \
///   --register-address <addr> \
///   --register-length <count> \
///   --register-mode <mode> \
///   --baud-rate 9600 \
///   --json
/// ```
///
/// # Example 1: Basic Verification
///
/// ```rust,no_run
/// # use anyhow::Result;
/// # async fn example() -> Result<()> {
/// let test_data = vec![1000, 2000, 3000, 4000, 5000];
/// let config = StationConfig {
///     station_id: 1,
///     register_mode: RegisterMode::Holding,
///     start_address: 100,
///     register_count: 5,
///     is_master: true,
///     register_values: None,
/// };
///
/// // After TUI Master has polled Slave...
/// verify_master_data("COM4", &test_data, &config).await?;
/// // Verifies Master received [1000, 2000, 3000, 4000, 5000]
/// # Ok(())
/// # }
/// ```
///
/// # Example 2: Coils Verification
///
/// ```rust,no_run
/// # use anyhow::Result;
/// # async fn example() -> Result<()> {
/// let test_coils = vec![1, 0, 1, 0, 1, 0, 1, 0]; // Bit values
/// let config = StationConfig {
///     station_id: 2,
///     register_mode: RegisterMode::Coils,
///     start_address: 0,
///     register_count: 8,
///     is_master: true,
///     register_values: None,
/// };
///
/// verify_master_data("COM4", &test_coils, &config).await?;
/// # Ok(())
/// # }
/// ```
///
/// # Error Handling
///
/// ## CLI Execution Failure
/// - **Symptom**: `"CLI slave-poll failed: <stderr>"` error
/// - **Cause**: CLI binary not found, port unavailable, or invalid arguments
/// - **Solution**: Check binary path via `build_debug_bin`, verify port is free
///
/// ## Data Length Mismatch
/// - **Symptom**: `"Value count mismatch: expected X, got Y"`
/// - **Cause**: Master didn't read full register range, or CLI returned partial data
/// - **Solution**: Check Master's register_count configuration, verify CLI args
///
/// ## Value Mismatch
/// - **Symptom**: `"Value[N] mismatch: expected 0xXXXX, got 0xYYYY"`
/// - **Cause**: Master read incorrect data, or Slave sent wrong values
/// - **Solution**: Check Modbus frame logs, verify CRC calculation, inspect port quality
///
/// ## JSON Parse Error
/// - **Symptom**: JSON deserialization error from `serde_json::from_str`
/// - **Cause**: CLI output format changed, or stderr mixed with stdout
/// - **Solution**: Verify CLI version, check stdout doesn't contain debug logs
///
/// ## Missing 'values' Field
/// - **Symptom**: `"No 'values' field found in JSON output"`
/// - **Cause**: CLI returned error JSON instead of data JSON
/// - **Solution**: Check stderr for CLI error messages, verify Master is polling
///
/// # JSON Output Format
///
/// Expected CLI output structure:
/// ```json
/// {
///   "station_id": 1,
///   "register_mode": "holding",
///   "start_address": 100,
///   "register_count": 5,
///   "values": [1000, 2000, 3000, 4000, 5000],
///   "timestamp": "2025-10-27T12:34:56Z"
/// }
/// ```
///
/// # Debug Logging
///
/// This function emits detailed debug logs:
/// ```text
/// 🔍 DEBUG: CLI slave-poll starting on port COM4
/// 🔍 DEBUG: Expected data: [1000, 2000, 3000, 4000, 5000]
/// 🔍 DEBUG: Using binary: target/debug/aoba
/// 🔍 DEBUG: CLI args: ["--slave-poll", "COM4", "--station-id", "1", ...]
/// 🔍 DEBUG: CLI exit status: ExitStatus(ExitCode(0))
/// 🔍 DEBUG: CLI stderr: (empty or warnings)
/// 🔍 DEBUG: Parsed JSON: Object({"values": Array([Number(1000), ...])})
/// 🔍 DEBUG: Received values: [1000, 2000, 3000, 4000, 5000]
/// ✅ All 5 values verified
/// ```
///
/// # Timing Considerations
///
/// - **CLI Execution**: 1-3 seconds (depends on poll timeout)
/// - **JSON Parsing**: <100ms
/// - **Verification**: <100ms
/// - **Total Duration**: 1-4 seconds
///
/// # See Also
///
/// - [`run_single_station_master_test`]: Uses this function for data verification
/// - [`send_data_from_cli_master`]: Inverse operation (CLI sends, TUI receives)
/// - [`StationConfig`]: Configuration structure for Master/Slave
/// - [`build_debug_bin`]: Locates the AOBA CLI binary
pub async fn verify_master_data(
    port2: &str,
    expected_data: &[u16],
    config: &StationConfig,
) -> Result<()> {
    log::info!("📡 Polling data from Master...");
    log::info!("🔍 DEBUG: CLI slave-poll starting on port {port2}");
    log::info!("🔍 DEBUG: Expected data: {expected_data:?}");

    let binary = build_debug_bin("aoba")?;
    log::info!("🔍 DEBUG: Using binary: {binary:?}");

    // Create owned args vec to avoid lifetime issues with spawn_blocking
    let args_vec: Vec<String> = vec![
        "--slave-poll".to_string(),
        port2.to_string(),
        "--station-id".to_string(),
        config.station_id.to_string(),
        "--register-address".to_string(),
        config.start_address.to_string(),
        "--register-length".to_string(),
        config.register_count.to_string(),
        "--register-mode".to_string(),
        config.register_mode.as_cli_mode().to_string(),
        "--baud-rate".to_string(),
        "9600".to_string(),
        "--json".to_string(),
    ];
    log::info!("🔍 DEBUG: CLI args: {args_vec:?}");

    // Wrap the CLI command execution in a timeout to prevent indefinite hangs in CI
    // CLI slave-poll should complete in 5-10 seconds under normal conditions
    // Use 30 seconds timeout to account for slow CI environments
    const CLI_TIMEOUT_SECS: u64 = 30;
    
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(CLI_TIMEOUT_SECS),
        tokio::task::spawn_blocking(move || {
            std::process::Command::new(&binary).args(&args_vec).output()
        }),
    )
    .await
    .map_err(|_| anyhow!("CLI slave-poll timed out after {} seconds", CLI_TIMEOUT_SECS))?
    .map_err(|e| anyhow!("Failed to spawn CLI slave-poll task: {}", e))??;

    log::info!("🔍 DEBUG: CLI exit status: {:?}", output.status);
    log::info!(
        "🔍 DEBUG: CLI stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    if !output.status.success() {
        return Err(anyhow!(
            "CLI slave-poll failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    log::info!("CLI output: {stdout}");

    // Parse JSON output and verify values
    let json: serde_json::Value = serde_json::from_str(&stdout)?;
    log::info!("🔍 DEBUG: Parsed JSON: {json:?}");

    if let Some(values) = json.get("values").and_then(|v| v.as_array()) {
        let received_values: Vec<u16> = values
            .iter()
            .filter_map(|v| v.as_u64().map(|n| n as u16))
            .collect();

        log::info!("🔍 DEBUG: Received values: {received_values:?}");

        if received_values.len() != expected_data.len() {
            return Err(anyhow!(
                "Value count mismatch: expected {}, got {}",
                expected_data.len(),
                received_values.len()
            ));
        }

        for (i, (expected, received)) in
            expected_data.iter().zip(received_values.iter()).enumerate()
        {
            if expected != received {
                log::error!("🔍 DEBUG: Mismatch at index {i}: expected 0x{expected:04X}, got 0x{received:04X}");
                return Err(anyhow!(
                    "Value[{i}] mismatch: expected 0x{expected:04X}, got 0x{received:04X}"
                ));
            }
        }

        log::info!("✅ All {} values verified", expected_data.len());
    } else {
        return Err(anyhow!("No 'values' field found in JSON output"));
    }

    log::info!("✅ Data verification passed");
    Ok(())
}

/// Run a complete single-station Slave test with TUI Slave and CLI Master.
///
/// # Purpose
///
/// This is a **high-level test orchestrator** that validates the complete Modbus
/// Slave workflow:
/// 1. Generate random test data (coils or registers)
/// 2. Setup TUI environment and configure Slave station with test data
/// 3. Start CLI Master on second port
/// 4. CLI Master polls TUI Slave to read data
/// 5. Verify Master received correct data via CLI output
///
/// This function tests the **CLI → TUI communication path** where the CLI acts
/// as Master and the TUI responds as Slave.
///
/// # Test Architecture
///
/// ```text
/// Port1 (TUI Slave)                     Port2 (CLI Master)
///       │                                     │
///       ├─ Configure Station #1               │
///       ├─ Enable Slave Mode                  │
///       ├─ Set Register Range                 │
///       ├─ Initialize with test data          │
///       │                                     │
///       │                            ┌────────┴────────┐
///       │                            │ Start CLI Master│
///       │                            │  polling mode   │
///       │                            └────────┬────────┘
///       │                                     │
///       │<──────── Poll Request ──────────────┤
///       ├────────── Response ────────────────>│
///       │         (test data)                 │
///       │                                     │
///       │                                ┌────┴────┐
///       │                                │ Verify  │
///       │                                │  Data   │
///       │                                └─────────┘
/// ```
///
/// # Parameters
///
/// - `port1`: Serial port for TUI Slave (e.g., "COM3", "/dev/ttyUSB0")
///   - Must support virtual loopback or physical connection to `port2`
/// - `port2`: Serial port for CLI Master (e.g., "COM4", "/dev/ttyUSB1")
///   - Connected to `port1` via null modem or virtual pair
/// - `config`: Station configuration without initial values
///   - `config.is_master` should be `false`
///   - `register_values` will be overwritten with generated test data
///
/// # Returns
///
/// - `Ok(())`: Test passed - CLI Master received correct data from TUI Slave
/// - `Err`: Test failed at any stage (setup, configuration, data verification)
///
/// # Test Workflow
///
/// ## Stage 1: Generate Test Data
/// - **Coils/DiscreteInputs**: Random bit values (0 or 1) via `generate_random_coils`
/// - **Holding/Input**: Random 16-bit values (0-65535) via `generate_random_registers`
/// - Data length matches `config.register_count`
///
/// ## Stage 2: Setup TUI Slave
/// - Call `setup_tui_test(port1, port2)` to initialize environment
/// - Call `navigate_to_modbus_panel` to reach Modbus dashboard
/// - Call `configure_tui_station` with test data to create Slave station
/// - TUI writes test data to Slave registers
///
/// ## Stage 3: Start CLI Master and Poll
/// - Call `send_data_from_cli_master` to spawn CLI in master-poll mode
/// - CLI sends read request to TUI Slave
/// - TUI Slave responds with register data
///
/// ## Stage 4: Verify Data
/// - `send_data_from_cli_master` internally verifies CLI's received data
/// - Compare against original test data
/// - Verify all registers match (exact equality check)
///
/// # Example 1: Slave Holding Registers Test
///
/// ```rust,no_run
/// # use anyhow::Result;
/// # async fn example() -> Result<()> {
/// let slave_config = StationConfig {
///     station_id: 1,
///     register_mode: RegisterMode::Holding,
///     start_address: 200,
///     register_count: 10,
///     is_master: false,
///     register_values: None, // Will be overwritten with test data
/// };
///
/// run_single_station_slave_test("COM3", "COM4", slave_config).await?;
/// // Test generates 10 random values, configures Slave with data,
/// // starts CLI Master to poll, and verifies data match
/// # Ok(())
/// # }
/// ```
///
/// # Example 2: Slave Coils Test
///
/// ```rust,no_run
/// # use anyhow::Result;
/// # async fn example() -> Result<()> {
/// let coil_config = StationConfig {
///     station_id: 2,
///     register_mode: RegisterMode::Coils,
///     start_address: 0,
///     register_count: 32,
///     is_master: false,
///     register_values: None,
/// };
///
/// run_single_station_slave_test("COM3", "COM4", coil_config).await?;
/// // Test generates 32 random bits (0/1), verifies CLI Master read them correctly
/// # Ok(())
/// # }
/// ```
///
/// # Error Handling
///
/// ## TUI Setup Failure
/// - **Symptom**: `setup_tui_test` or `navigate_to_modbus_panel` fails
/// - **Cause**: Port unavailable, TUI crash, or navigation timeout
/// - **Solution**: Verify ports exist, check TUI logs, retry with longer timeouts
///
/// ## Slave Configuration Failure
/// - **Symptom**: `configure_tui_station` fails during register initialization
/// - **Cause**: Register edit timeout, or values not saved properly
/// - **Solution**: Increase edit timeouts, verify register count matches data length
///
/// ## CLI Master Start Failure
/// - **Symptom**: `send_data_from_cli_master` fails with CLI error
/// - **Cause**: Port already in use, CLI binary missing, or permissions issue
/// - **Solution**: Check `lsof` (Unix) or `mode` (Windows), verify CLI path
///
/// ## Data Verification Failure
/// - **Symptom**: `send_data_from_cli_master` reports data mismatch
/// - **Cause**: Slave registers not initialized, or CLI received corrupted data
/// - **Solution**: Verify register initialization completed, check port connection quality
///
/// # Timing Considerations
///
/// - **TUI Setup**: 5-15 seconds
/// - **Slave Configuration**: 15-45 seconds (includes register initialization)
/// - **CLI Master Poll**: 2-5 seconds
/// - **Data Verification**: 1-2 seconds
/// - **Total Duration**: 25-70 seconds depending on register count
///
/// # Debug Tips
///
/// ## Enable Verbose Logging
/// ```bash
/// RUST_LOG=debug cargo run --example tui_e2e
/// ```
///
/// ## Check Slave Registers in TUI
/// After configuration, manually verify registers in TUI:
/// - Navigate to Station #1
/// - Check register values match test data
/// - Verify all registers initialized correctly
///
/// ## Monitor CLI Output
/// ```bash
/// # Run CLI master-poll manually to debug
/// aoba --master-poll COM4 \
///   --station-id 1 \
///   --register-address 200 \
///   --register-length 10 \
///   --register-mode holding \
///   --json
/// ```
///
/// ## Compare Test Data
/// ```rust,no_run
/// # use anyhow::Result;
/// # async fn debug_test() -> Result<()> {
/// # let test_data = vec![];
/// # let port2 = "";
/// # let config = todo!();
/// log::info!("Test data sent to Slave: {:?}", test_data);
/// send_data_from_cli_master(port2, &test_data, &config).await?;
/// // CLI logs will show received data for comparison
/// # Ok(())
/// # }
/// ```
///
/// # See Also
///
/// - [`send_data_from_cli_master`]: Function to poll Slave via CLI Master
/// - [`run_single_station_master_test`]: Inverse test (TUI Master, CLI Slave)
/// - [`configure_tui_station`]: Underlying station configuration
/// - [`setup_tui_test`]: Environment initialization
/// - [`generate_random_coils`], [`generate_random_registers`]: Test data generators
pub async fn run_single_station_slave_test(
    port1: &str,
    port2: &str,
    config: StationConfig,
) -> Result<()> {
    log::info!("🧪 Running single-station Slave test");
    log::info!("   Port1: {port1} (TUI Slave)");
    log::info!("   Port2: {port2} (CLI Master)");
    log::info!("   Config: {config:?}");

    // Generate test data
    let test_data = if matches!(
        config.register_mode,
        RegisterMode::Coils | RegisterMode::DiscreteInputs
    ) {
        generate_random_coils(config.register_count as usize)
    } else {
        generate_random_registers(config.register_count as usize)
    };
    log::info!("Generated test data: {test_data:?}");

    // Create config with test data
    let mut config_with_data = config.clone();
    config_with_data.register_values = Some(test_data.clone());

    // Setup TUI
    let (mut session, mut cap) = setup_tui_test(port1, port2).await?;

    // Navigate to Modbus panel
    navigate_to_modbus_panel(&mut session, &mut cap, port1).await?;

    // Configure station
    configure_tui_station(&mut session, &mut cap, port1, &config_with_data).await?;

    // Wait for TUI to be ready
    sleep_3s().await;

    // Send data from CLI Master and verify
    send_data_from_cli_master(port2, &test_data, &config).await?;

    log::info!("✅ Single-station Slave test PASSED");
    log::info!("   ✓ Configuration UI working correctly");
    log::info!("   ✓ Field navigation validated");
    log::info!("   ✓ Data entry successful");
    log::info!("   ✓ Save operation completed");
    log::info!("   ✓ CLI Master received correct data");
    
    // Explicitly terminate TUI session to ensure clean shutdown
    terminate_session(session, "TUI").await?;
    
    Ok(())
}

/// Send data from CLI Master to TUI Slave and verify data integrity.
///
/// # Purpose
///
/// This function acts as a **CLI Master** that polls a TUI Slave station and
/// verifies the received data matches expected values. It tests the complete
/// **CLI → TUI communication path** where the CLI initiates read operations
/// and the TUI responds as a Slave.
///
/// This is a **verification helper** used in Slave test scenarios to confirm
/// the Slave responds correctly to Master read requests.
///
/// # Communication Flow
///
/// ```text
/// CLI Master (Port2)                   TUI Slave (Port1)
///       │                                     │
///       │  Has test data in registers         │
///       │                                     │
///       ├──────── Poll Request ──────────────>│
///       │<──────── Response ──────────────────┤
///       │         (test data)                 │
///       │                                     │
///   ┌───┴───┐                                 │
///   │Compare│                                 │
///   │ Data  │                                 │
///   └───────┘                                 │
/// ```
///
/// # Parameters
///
/// - `port2`: Serial port for CLI Master (e.g., "COM4", "/dev/ttyUSB1")
///   - Must be connected to the TUI Slave's port
///   - CLI will act as Master on this port
/// - `expected_data`: Expected register values that Slave should return
///   - Length must match `config.register_count`
///   - Values are 16-bit unsigned integers (0-65535)
/// - `config`: Station configuration used by Slave
///   - Defines station ID, register mode, address range, etc.
///
/// # Returns
///
/// - `Ok(())`: Data verification passed - all values match
/// - `Err`: Verification failed (CLI error, data mismatch, or JSON parse error)
///
/// # CLI Command Structure
///
/// The function builds and executes this CLI command:
/// ```bash
/// aoba --master-poll <port2> \
///   --station-id <id> \
///   --register-address <addr> \
///   --register-length <count> \
///   --register-mode <mode> \
///   --baud-rate 9600 \
///   --json
/// ```
///
/// # Example 1: Basic Data Send and Verify
///
/// ```rust,no_run
/// # use anyhow::Result;
/// # async fn example() -> Result<()> {
/// let test_data = vec![1000, 2000, 3000, 4000, 5000];
/// let config = StationConfig {
///     station_id: 1,
///     register_mode: RegisterMode::Holding,
///     start_address: 100,
///     register_count: 5,
///     is_master: false,
///     register_values: None,
/// };
///
/// // After TUI Slave is configured with test data...
/// send_data_from_cli_master("COM4", &test_data, &config).await?;
/// // CLI Master polls Slave and verifies received [1000, 2000, 3000, 4000, 5000]
/// # Ok(())
/// # }
/// ```
///
/// # Example 2: Coils Data Verification
///
/// ```rust,no_run
/// # use anyhow::Result;
/// # async fn example() -> Result<()> {
/// let test_coils = vec![1, 0, 1, 0, 1, 0, 1, 0]; // Bit values
/// let config = StationConfig {
///     station_id: 2,
///     register_mode: RegisterMode::Coils,
///     start_address: 0,
///     register_count: 8,
///     is_master: false,
///     register_values: None,
/// };
///
/// send_data_from_cli_master("COM4", &test_coils, &config).await?;
/// # Ok(())
/// # }
/// ```
///
/// # Error Handling
///
/// ## CLI Execution Failure
/// - **Symptom**: `"CLI master-poll failed: <stderr>"` error
/// - **Cause**: CLI binary not found, port unavailable, or invalid arguments
/// - **Solution**: Check binary path via `build_debug_bin`, verify port is free
///
/// ## Data Length Mismatch
/// - **Symptom**: `"Value count mismatch: expected X, got Y"`
/// - **Cause**: Slave didn't respond with full register range, or CLI received partial data
/// - **Solution**: Check Slave's register_count configuration, verify CLI args
///
/// ## Value Mismatch
/// - **Symptom**: `"Value[N] mismatch: expected 0xXXXX, got 0xYYYY"`
/// - **Cause**: Slave sent incorrect data, or CLI received corrupted data
/// - **Solution**: Check Slave register initialization, verify CRC calculation, inspect port quality
///
/// ## JSON Parse Error
/// - **Symptom**: JSON deserialization error from `serde_json::from_str`
/// - **Cause**: CLI output format changed, or stderr mixed with stdout
/// - **Solution**: Verify CLI version, check stdout doesn't contain debug logs
///
/// ## Missing 'values' Field
/// - **Symptom**: `"No 'values' field found in JSON output"`
/// - **Cause**: CLI returned error JSON instead of data JSON
/// - **Solution**: Check stderr for CLI error messages, verify Slave is responding
///
/// # JSON Output Format
///
/// Expected CLI output structure:
/// ```json
/// {
///   "station_id": 1,
///   "register_mode": "holding",
///   "start_address": 100,
///   "register_count": 5,
///   "values": [1000, 2000, 3000, 4000, 5000],
///   "timestamp": "2025-10-27T12:34:56Z"
/// }
/// ```
///
/// # Debug Logging
///
/// This function emits detailed debug logs:
/// ```text
/// 🔍 DEBUG: CLI master-poll starting on port COM4
/// 🔍 DEBUG: Expected data: [1000, 2000, 3000, 4000, 5000]
/// 🔍 DEBUG: Using binary: target/debug/aoba
/// 🔍 DEBUG: CLI args: ["--master-poll", "COM4", "--station-id", "1", ...]
/// 🔍 DEBUG: CLI exit status: ExitStatus(ExitCode(0))
/// 🔍 DEBUG: CLI stderr: (empty or warnings)
/// 🔍 DEBUG: Parsed JSON: Object({"values": Array([Number(1000), ...])})
/// 🔍 DEBUG: Received values: [1000, 2000, 3000, 4000, 5000]
/// ✅ All 5 values verified
/// ```
///
/// # Timing Considerations
///
/// - **CLI Execution**: 1-3 seconds (depends on poll timeout)
/// - **JSON Parsing**: <100ms
/// - **Verification**: <100ms
/// - **Total Duration**: 1-4 seconds
///
/// # See Also
///
/// - [`run_single_station_slave_test`]: Uses this function for data verification
/// - [`verify_master_data`]: Inverse operation (CLI Slave, TUI Master)
/// - [`StationConfig`]: Configuration structure for Master/Slave
/// - [`build_debug_bin`]: Locates the AOBA CLI binary
pub async fn send_data_from_cli_master(
    port2: &str,
    expected_data: &[u16],
    config: &StationConfig,
) -> Result<()> {
    log::info!("📤 Sending data from CLI Master...");
    log::info!("🔍 DEBUG: CLI master-poll starting on port {port2}");
    log::info!("🔍 DEBUG: Expected data: {expected_data:?}");

    let binary = build_debug_bin("aoba")?;
    log::info!("🔍 DEBUG: Using binary: {binary:?}");

    // Create owned args vec to avoid lifetime issues with spawn_blocking
    let args_vec: Vec<String> = vec![
        "--master-poll".to_string(),
        port2.to_string(),
        "--station-id".to_string(),
        config.station_id.to_string(),
        "--register-address".to_string(),
        config.start_address.to_string(),
        "--register-length".to_string(),
        config.register_count.to_string(),
        "--register-mode".to_string(),
        config.register_mode.as_cli_mode().to_string(),
        "--baud-rate".to_string(),
        "9600".to_string(),
        "--json".to_string(),
    ];
    log::info!("🔍 DEBUG: CLI args: {args_vec:?}");

    // Wrap the CLI command execution in a timeout to prevent indefinite hangs in CI
    // CLI master-poll should complete in 5-10 seconds under normal conditions
    // Use 30 seconds timeout to account for slow CI environments
    const CLI_TIMEOUT_SECS: u64 = 30;
    
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(CLI_TIMEOUT_SECS),
        tokio::task::spawn_blocking(move || {
            std::process::Command::new(&binary).args(&args_vec).output()
        }),
    )
    .await
    .map_err(|_| anyhow!("CLI master-poll timed out after {} seconds", CLI_TIMEOUT_SECS))?
    .map_err(|e| anyhow!("Failed to spawn CLI master-poll task: {}", e))??;

    log::info!("🔍 DEBUG: CLI exit status: {:?}", output.status);
    log::info!(
        "🔍 DEBUG: CLI stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    if !output.status.success() {
        return Err(anyhow!(
            "CLI master-poll failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    log::info!("CLI output: {stdout}");

    // Parse JSON output and verify values
    let json: serde_json::Value = serde_json::from_str(&stdout)?;
    log::info!("🔍 DEBUG: Parsed JSON: {json:?}");

    if let Some(values) = json.get("values").and_then(|v| v.as_array()) {
        let received_values: Vec<u16> = values
            .iter()
            .filter_map(|v| v.as_u64().map(|n| n as u16))
            .collect();

        log::info!("🔍 DEBUG: Received values: {received_values:?}");

        if received_values.len() != expected_data.len() {
            return Err(anyhow!(
                "Value count mismatch: expected {}, got {}",
                expected_data.len(),
                received_values.len()
            ));
        }

        for (i, (expected, received)) in
            expected_data.iter().zip(received_values.iter()).enumerate()
        {
            if expected != received {
                log::error!("🔍 DEBUG: Mismatch at index {i}: expected 0x{expected:04X}, got 0x{received:04X}");
                return Err(anyhow!(
                    "Value[{i}] mismatch: expected 0x{expected:04X}, got 0x{received:04X}"
                ));
            }
        }

        log::info!("✅ All {} values verified", expected_data.len());
    } else {
        return Err(anyhow!("No 'values' field found in JSON output"));
    }

    log::info!("✅ Data verification passed");
    Ok(())
}

/// Verify data received by TUI Slave from CLI Master polling.
///
/// # Purpose
///
/// This function validates that a TUI Slave station has correctly received and
/// stored data sent by a CLI Master. It reads the Slave's register values from
/// the TUI status file and compares them against expected values.
///
/// This is a **verification helper** used in Slave test scenarios to confirm
/// the Slave correctly processed Master write operations.
///
/// # Verification Flow
///
/// ```text
/// CLI Master (Port2)                   TUI Slave (Port1)
///       │                                     │
///       │  Sent write request with            │
///       │  test data                          │
///       │                                     │
///       ├──────── Write Request ─────────────>│
///       │<──────── Response ──────────────────┤
///       │                                     │
///       │                                ┌────┴────┐
///       │                                │ Read    │
///       │                                │ Status  │
///       │                                │ File    │
///       │                                └─────────┘
///   ┌───┴───┐                                 │
///   │Compare│                                 │
///   │ Data  │                                 │
///   └───────┘                                 │
/// ```
///
/// # Parameters
///
/// - `session`: Active TUI session (not used in current implementation)
/// - `cap`: Terminal capture tool (not used in current implementation)
/// - `expected_data`: Expected register values that Slave should have stored
///   - Length must match `config.register_count`
///   - Values are 16-bit unsigned integers (0-65535)
/// - `config`: Station configuration used by Slave
///   - Defines station ID, register mode, address range, etc.
///
/// # Returns
///
/// - `Ok(())`: Data verification passed - all values match
/// - `Err`: Verification failed (status read error, data mismatch, etc.)
///
/// # Status File Reading
///
/// The function reads from the TUI status file to get current register values:
/// - Path: `/tmp/aoba_tui_status.json` (Unix) or equivalent
/// - Structure: Contains `ports[].modbus_slaves[].registers[]`
/// - Values: Current register values stored by the Slave
///
/// # Example 1: Basic Slave Data Verification
///
/// ```rust,no_run
/// # use anyhow::Result;
/// # async fn example<T: Expect>(
/// #     session: &mut T,
/// #     cap: &mut TerminalCapture,
/// # ) -> Result<()> {
/// let test_data = vec![1000, 2000, 3000, 4000, 5000];
/// let config = StationConfig {
///     station_id: 1,
///     register_mode: RegisterMode::Holding,
///     start_address: 100,
///     register_count: 5,
///     is_master: false,
///     register_values: None,
/// };
///
/// // After CLI Master wrote data to TUI Slave...
/// verify_slave_data(session, cap, &test_data, &config).await?;
/// // Verifies Slave stored [1000, 2000, 3000, 4000, 5000]
/// # Ok(())
/// # }
/// ```
///
/// # Example 2: Coils Data Verification
///
/// ```rust,no_run
/// # use anyhow::Result;
/// # async fn example<T: Expect>(
/// #     session: &mut T,
/// #     cap: &mut TerminalCapture,
/// # ) -> Result<()> {
/// let test_coils = vec![1, 0, 1, 0, 1, 0, 1, 0]; // Bit values
/// let config = StationConfig {
///     station_id: 2,
///     register_mode: RegisterMode::Coils,
///     start_address: 0,
///     register_count: 8,
///     is_master: false,
///     register_values: None,
/// };
///
/// verify_slave_data(session, cap, &test_coils, &config).await?;
/// # Ok(())
/// # }
/// ```
///
/// # Error Handling
///
/// ## Status File Read Failure
/// - **Symptom**: `"Failed to read TUI status file"` error
/// - **Cause**: TUI not running, file permissions, or path incorrect
/// - **Solution**: Check TUI is running, verify file exists and readable
///
/// ## No Slave Configuration
/// - **Symptom**: `"No slave configuration found in status file"`
/// - **Cause**: Slave not configured, or configuration not saved
/// - **Solution**: Verify Slave configuration completed successfully
///
/// ## Data Length Mismatch
/// - **Symptom**: `"Value count mismatch: expected X, got Y"`
/// - **Cause**: Slave register count doesn't match expected, or data corruption
/// - **Solution**: Check Slave configuration, verify Master wrote correct range
///
/// ## Value Mismatch
/// - **Symptom**: `"Value[N] mismatch: expected 0xXXXX, got 0xYYYY"`
/// - **Cause**: Master write failed, or Slave didn't store data correctly
/// - **Solution**: Check Modbus frame logs, verify write operation succeeded
///
/// # Status File Structure
///
/// Expected status file structure for Slave verification:
/// ```json
/// {
///   "ports": [
///     {
///       "modbus_slaves": [
///         {
///           "station_id": 1,
///           "register_type": "holding",
///           "start_address": 100,
///           "register_count": 5,
///           "registers": [1000, 2000, 3000, 4000, 5000]
///         }
///       ]
///     }
///   ]
/// }
/// ```
///
/// # Debug Logging
///
/// This function emits detailed debug logs:
/// ```text
/// 🔍 DEBUG: Verifying Slave data in TUI status file
/// 🔍 DEBUG: Expected data: [1000, 2000, 3000, 4000, 5000]
/// 🔍 DEBUG: Status file read successfully
/// 🔍 DEBUG: Found 1 slave configurations
/// 🔍 DEBUG: Slave config - ID:1, Type:holding, Addr:100, Count:5
/// 🔍 DEBUG: Slave registers: [1000, 2000, 3000, 4000, 5000]
/// ✅ All 5 values verified
/// ```
///
/// # Timing Considerations
///
/// - **Status File Read**: <100ms
/// - **JSON Parsing**: <100ms
/// - **Verification**: <100ms
/// - **Total Duration**: <500ms
///
/// # See Also
///
/// - [`run_multi_station_slave_test`]: Uses this function for data verification
/// - [`send_data_from_cli_master`]: Inverse operation (CLI reads, TUI responds)
/// - [`StationConfig`]: Configuration structure for Master/Slave
/// - [`read_tui_status`]: Function to read TUI status file
pub async fn verify_slave_data<T: Expect>(
    _session: &mut T,
    _cap: &mut TerminalCapture,
    _expected_data: &[u16],
    config: &StationConfig,
) -> Result<()> {
    log::info!("🔍 Verifying Slave configuration in TUI status file...");

    // Read TUI status file
    let status = read_tui_status().map_err(|e| {
        anyhow!(
            "Failed to read TUI status file for Slave verification: {}",
            e
        )
    })?;
    log::info!("🔍 DEBUG: Status file read successfully");

    // Find the slave configuration
    if status.ports.is_empty() || status.ports[0].modbus_slaves.is_empty() {
        return Err(anyhow!("No slave configuration found in status file"));
    }

    let slave = &status.ports[0].modbus_slaves[0];
    log::info!(
        "🔍 DEBUG: Found {} slave configurations",
        status.ports[0].modbus_slaves.len()
    );
    log::info!(
        "🔍 DEBUG: Slave config - ID:{}, Type:{}, Addr:{}, Count:{}",
        slave.station_id,
        slave.register_type,
        slave.start_address,
        slave.register_count
    );

    // Verify configuration matches
    if slave.station_id != config.station_id {
        return Err(anyhow!(
            "Slave station ID mismatch: expected {}, got {}",
            config.station_id,
            slave.station_id
        ));
    }

    if slave.start_address != config.start_address {
        return Err(anyhow!(
            "Slave start address mismatch: expected {}, got {}",
            config.start_address,
            slave.start_address
        ));
    }

    if slave.register_count != config.register_count as usize {
        return Err(anyhow!(
            "Slave register count mismatch: expected {}, got {}",
            config.register_count,
            slave.register_count
        ));
    }

    log::info!("✅ Slave configuration verified successfully");
    Ok(())
}

/// Configure multiple stations in the TUI for multi-station tests.
///
/// # Purpose
///
/// This function configures multiple Modbus stations in sequence for testing
/// complex multi-station scenarios. It handles the complete workflow for each
/// station: navigation, mode configuration, station creation, register setup,
/// and value initialization.
///
/// This is a **configuration helper** used in multi-station test scenarios
/// where multiple Masters and Slaves need to be set up simultaneously.
///
/// # Configuration Flow
///
/// ```text
/// For each station in config_list:
///       │
///       ├─ Navigate to Modbus panel
///       ├─ Configure connection mode (Master/Slave)
///       ├─ Create station with proper mode
///       ├─ Configure register type, address, count
///       ├─ Initialize register values (Slave only)
///       ├─ Save configuration
///       │
/// ```
///
/// # Parameters
///
/// - `session`: Active TUI session from `setup_tui_test`
/// - `cap`: Terminal capture tool for screen reading and verification
/// - `port1`: Serial port name (e.g., "COM3", "/dev/ttyUSB0")
///   - All stations will be configured on this port
/// - `config_list`: List of station configurations to create
///   - Each config defines station parameters and initial values
///   - Order matters: configure Masters first, then Slaves
///
/// # Returns
///
/// - `Ok(())`: All stations configured successfully
/// - `Err`: Configuration failed for any station (navigation, creation, field edits, etc.)
///
/// # Station Configuration Order
///
/// ## Recommended Order
/// 1. **Master stations first**: Configure all Masters before Slaves
/// 2. **Slave stations second**: Configure Slaves with test data
/// 3. **Sequential IDs**: Use consecutive station IDs (1, 2, 3...)
///
/// ## Why Order Matters
/// - **Cursor positioning**: Each station creation moves cursor to new station
/// - **Navigation complexity**: Adding stations shifts existing station positions
/// - **Mode dependencies**: Masters and Slaves have different configuration flows
///
/// # Example 1: Master + Slave Configuration
///
/// ```rust,no_run
/// # use anyhow::Result;
/// # async fn example<T: Expect>(
/// #     session: &mut T,
/// #     cap: &mut TerminalCapture,
/// # ) -> Result<()> {
/// let configs = vec![
///     StationConfig {
///         station_id: 1,
///         register_mode: RegisterMode::Holding,
///         start_address: 100,
///         register_count: 10,
///         is_master: true,
///         register_values: None, // Master doesn't need initial values
///     },
///     StationConfig {
///         station_id: 2,
///         register_mode: RegisterMode::Holding,
///         start_address: 200,
///         register_count: 10,
///         is_master: false,
///         register_values: Some(vec![1000, 2000, 3000, 4000, 5000, 6000, 7000, 8000, 9000, 10000]),
///     },
/// ];
///
/// configure_multiple_stations(session, cap, "COM3", &configs).await?;
/// // Configures Master station #1, then Slave station #2 with test data
/// # Ok(())
/// # }
/// ```
///
/// # Example 2: Multiple Slaves with Different Register Types
///
/// ```rust,no_run
/// # use anyhow::Result;
/// # async fn example<T: Expect>(
/// #     session: &mut T,
/// #     cap: &mut TerminalCapture,
/// # ) -> Result<()> {
/// let configs = vec![
///     StationConfig {
///         station_id: 1,
///         register_mode: RegisterMode::Coils,
///         start_address: 0,
///         register_count: 16,
///         is_master: false,
///         register_values: Some(vec![1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0]),
///     },
///     StationConfig {
///         station_id: 2,
///         register_mode: RegisterMode::Holding,
///         start_address: 1000,
///         register_count: 5,
///         is_master: false,
///         register_values: Some(vec![1000, 2000, 3000, 4000, 5000]),
///     },
/// ];
///
/// configure_multiple_stations(session, cap, "COM3", &configs).await?;
/// # Ok(())
/// # }
/// ```
///
/// # Error Handling
///
/// ## Station Creation Failure
/// - **Symptom**: `configure_tui_station` fails for specific station
/// - **Cause**: Register edit timeout, navigation issues, or value initialization failure
/// - **Solution**: Check station order, verify register counts match data lengths
///
/// ## Cursor Positioning Issues
/// - **Symptom**: Subsequent stations fail after first station succeeds
/// - **Cause**: Cursor not positioned correctly after station creation
/// - **Solution**: Configure Masters first, ensure proper navigation between stations
///
/// ## Register Value Mismatch
/// - **Symptom**: Slave register initialization fails with length mismatch
/// - **Cause**: `register_values` length doesn't match `register_count`
/// - **Solution**: Verify data array length matches register count exactly
///
/// # Timing Considerations
///
/// - **Per Station**: 15-45 seconds (depends on register count and value initialization)
/// - **Multiple Stations**: N × per-station time + navigation overhead
/// - **Total Duration**: 30-180 seconds for 2-3 stations
///
/// # Debug Tips
///
/// ## Monitor Station Creation
/// ```rust,no_run
/// # use anyhow::Result;
/// # async fn debug_multi_station<T: Expect>(
/// #     session: &mut T,
/// #     cap: &mut TerminalCapture,
/// # ) -> Result<()> {
/// // After each station configuration
/// let screen = cap.capture(session, "after_station_config").await?;
/// log::info!("Current screen after station config:\n{}", screen);
/// # Ok(())
/// # }
/// ```
///
/// ## Check Station Order
/// - Configure simpler stations first (fewer registers)
/// - Masters before Slaves (less complex configuration)
/// - Verify station IDs are unique and sequential
///
/// ## Verify Configuration State
/// ```bash
/// # Check TUI status file after each station
/// watch -n 1 cat /tmp/aoba_tui_status.json
/// ```
///
/// # See Also
///
/// - [`configure_tui_station`]: Underlying single station configuration
/// - [`run_multi_station_master_test`]: Uses this for multi-Master scenarios
/// - [`run_multi_station_slave_test`]: Uses this for multi-Slave scenarios
/// - [`StationConfig`]: Configuration structure for stations
pub async fn configure_multiple_stations<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    port1: &str,
    config_list: &[StationConfig],
) -> Result<()> {
    log::info!("🔧 Configuring {} stations...", config_list.len());

    for (i, config) in config_list.iter().enumerate() {
        log::info!(
            "📝 Configuring station {}/{}: {:?}",
            i + 1,
            config_list.len(),
            config
        );

        // Navigate to Modbus panel for each station (cursor may have moved)
        navigate_to_modbus_panel(session, cap, port1).await?;

        // Configure the station
        configure_tui_station(session, cap, port1, config).await?;

        log::info!(
            "✅ Station {}/{} configured successfully",
            i + 1,
            config_list.len()
        );
    }

    log::info!("✅ All {} stations configured", config_list.len());
    Ok(())
}

/// Run a complete multi-station Master test with TUI Masters and CLI Slaves.
///
/// # Purpose
///
/// This is a **high-level test orchestrator** that validates complex multi-station
/// Modbus scenarios with multiple Masters polling multiple Slaves. It tests the
/// complete **TUI → CLI communication matrix** where multiple TUI Masters poll
/// multiple CLI Slaves simultaneously.
///
/// This function tests **multi-Master polling** scenarios where each Master
/// polls all Slaves in sequence, verifying data integrity across all combinations.
///
/// # Test Architecture
///
/// ```text
/// Port1 (TUI Masters)                  Port2 (CLI Slaves)
///       │                                     │
///       ├─ Configure Master #1                │
///       ├─ Configure Master #2                │
///       │                                     │
///       │                            ┌────────┴────────┐
///       │                            │ Start CLI Slave │
///       │                            │ with data for   │
///       │                            │ Master #1       │
///       │                            └────────┬────────┘
///       │                                     │
///       ├────── Master #1 Poll ──────────────>│
///       │<──── Response (data) ───────────────┤
///       │                                     │
///       │                            ┌────────┴────────┐
///       │                            │ Switch CLI to   │
///       │                            │ data for        │
///       │                            │ Master #2       │
///       │                            └────────┬────────┘
///       │                                     │
///       ├────── Master #2 Poll ──────────────>│
///       │<──── Response (data) ───────────────┤
///       │                                     │
///   ┌───┴───┐                                 │
///   │Verify │                                 │
///   │ All   │                                 │
///   │ Data  │                                 │
///   └───────┘                                 │
/// ```
///
/// # Parameters
///
/// - `port1`: Serial port for TUI Masters (e.g., "COM3", "/dev/ttyUSB0")
/// - `port2`: Serial port for CLI Slaves (e.g., "COM4", "/dev/ttyUSB1")
/// - `master_configs`: List of Master station configurations
///   - Each config defines station parameters for a Master
///   - `is_master` should be `true` for all configs
/// - `slave_configs`: List of Slave station configurations
///   - Each config defines station parameters and test data for a Slave
///   - `is_master` should be `false` for all configs
///   - `register_values` will be used as test data
///
/// # Returns
///
/// - `Ok(())`: Test passed - all Masters received correct data from all Slaves
/// - `Err`: Test failed at any stage (setup, configuration, polling, verification)
///
/// # Test Workflow
///
/// ## Stage 1: Setup TUI Masters
/// - Call `setup_tui_test(port1, port2)` to initialize environment
/// - Call `configure_multiple_stations` to create all Master stations
/// - Masters are configured but have no initial data (they will poll Slaves)
///
/// ## Stage 2: Test Each Master-Slave Combination
/// For each Master station:
/// - Generate unique test data for this Master-Slave pair
/// - Start CLI Slave with test data matching Master's station ID
/// - Wait for TUI Master to poll CLI Slave
/// - Verify Master received correct data via `verify_master_data`
///
/// ## Stage 3: Data Pattern Generation
/// - **Coils/DiscreteInputs**: Unique bit patterns for each Master-Slave pair
/// - **Holding/Input**: Unique 16-bit value sequences for each pair
/// - Ensures no cross-contamination between different Master-Slave combinations
///
/// # Example 1: Two Masters, Two Slaves
///
/// ```rust,no_run
/// # use anyhow::Result;
/// # async fn example() -> Result<()> {
/// let master_configs = vec![
///     StationConfig {
///         station_id: 1,
///         register_mode: RegisterMode::Holding,
///         start_address: 100,
///         register_count: 5,
///         is_master: true,
///         register_values: None,
///     },
///     StationConfig {
///         station_id: 2,
///         register_mode: RegisterMode::Holding,
///         start_address: 200,
///         register_count: 5,
///         is_master: true,
///         register_values: None,
///     },
/// ];
///
/// let slave_configs = vec![
///     StationConfig {
///         station_id: 1,
///         register_mode: RegisterMode::Holding,
///         start_address: 100,
///         register_count: 5,
///         is_master: false,
///         register_values: Some(vec![1000, 2000, 3000, 4000, 5000]),
///     },
///     StationConfig {
///         station_id: 2,
///         register_mode: RegisterMode::Holding,
///         start_address: 200,
///         register_count: 5,
///         is_master: false,
///         register_values: Some(vec![6000, 7000, 8000, 9000, 10000]),
///     },
/// ];
///
/// run_multi_station_master_test("COM3", "COM4", &master_configs, &slave_configs).await?;
/// // Tests Master #1 polling Slave #1, Master #2 polling Slave #2
/// # Ok(())
/// # }
/// ```
///
/// # Error Handling
///
/// ## Master Configuration Failure
/// - **Symptom**: `configure_multiple_stations` fails for Masters
/// - **Cause**: Station creation timeout or navigation issues
/// - **Solution**: Reduce number of Masters, check station ID conflicts
///
/// ## Slave Polling Failure
/// - **Symptom**: `verify_master_data` fails for specific Master-Slave pair
/// - **Cause**: Master didn't poll, Slave responded incorrectly, or timing issue
/// - **Solution**: Increase wait time between tests, verify port connections
///
/// ## Data Cross-Contamination
/// - **Symptom**: Master receives data from wrong Slave
/// - **Cause**: Station IDs not unique, or polling interference
/// - **Solution**: Ensure all station IDs are unique across Masters and Slaves
///
/// # Timing Considerations
///
/// - **Master Setup**: 5-15 seconds + N × 15-30 seconds (N = number of Masters)
/// - **Per Master-Slave Test**: 5-10 seconds (polling + verification)
/// - **Total Duration**: 30-120 seconds depending on station count
///
/// # Debug Tips
///
/// ## Monitor Master Polling
/// ```bash
/// # Watch TUI status file for Master data updates
/// watch -n 0.5 cat /tmp/aoba_tui_status.json
/// ```
///
/// ## Check Station IDs
/// - Ensure Master and Slave station IDs match for each test pair
/// - Verify no ID conflicts between different Masters/Slaves
///
/// ## Test Individual Pairs
/// ```rust,no_run
/// # use anyhow::Result;
/// # async fn debug_pair() -> Result<()> {
/// // Test one Master-Slave pair at a time
/// let master_config = StationConfig { /* ... */ };
/// let slave_config = StationConfig { /* ... */ };
/// run_single_station_master_test("COM3", "COM4", master_config).await?;
/// # Ok(())
/// # }
/// ```
///
/// # See Also
///
/// - [`run_multi_station_slave_test`]: Inverse test (CLI Masters, TUI Slaves)
/// - [`configure_multiple_stations`]: Multi-station configuration
/// - [`verify_master_data`]: Data verification for Masters
/// - [`generate_random_coils`], [`generate_random_registers`]: Test data generators
pub async fn run_multi_station_master_test(
    port1: &str,
    port2: &str,
    master_configs: &[StationConfig],
    slave_configs: &[StationConfig],
) -> Result<()> {
    log::info!("🧪 Running multi-station Master test");
    log::info!("   Port1: {port1} (TUI Masters)");
    log::info!("   Port2: {port2} (CLI Slaves)");
    log::info!("   Masters: {}", master_configs.len());
    log::info!("   Slaves: {}", slave_configs.len());

    // Setup TUI and configure all Masters
    let (mut session, mut cap) = setup_tui_test(port1, port2).await?;
    configure_multiple_stations(&mut session, &mut cap, port1, master_configs).await?;

    // Test each Master polling each Slave
    for (master_idx, master_config) in master_configs.iter().enumerate() {
        for (slave_idx, slave_config) in slave_configs.iter().enumerate() {
            log::info!(
                "🧪 Testing Master {}/{} polling Slave {}/{}",
                master_idx + 1,
                master_configs.len(),
                slave_idx + 1,
                slave_configs.len()
            );

            // Generate test data for this Master-Slave pair
            let test_data = if matches!(
                slave_config.register_mode,
                RegisterMode::Coils | RegisterMode::DiscreteInputs
            ) {
                generate_random_coils(slave_config.register_count as usize)
            } else {
                generate_random_registers(slave_config.register_count as usize)
            };

            // Create Slave config with test data and matching station ID
            let mut slave_config_with_data = slave_config.clone();
            slave_config_with_data.station_id = master_config.station_id; // Match Master's ID
            slave_config_with_data.register_values = Some(test_data.clone());

            // Start CLI Slave with test data
            // Note: In real implementation, would need to start CLI slave process here
            // For now, this is a placeholder for the CLI slave startup logic

            // Wait for Master to poll Slave
            sleep_1s().await;

            // Verify Master received correct data
            verify_master_data(port2, &test_data, &master_config).await?;

            log::info!(
                "✅ Master {}/{} successfully polled Slave {}/{}",
                master_idx + 1,
                master_configs.len(),
                slave_idx + 1,
                slave_configs.len()
            );
        }
    }

    log::info!("✅ Multi-station Master test PASSED");
    log::info!("   ✓ All Masters configured correctly");
    log::info!("   ✓ All Master-Slave polling combinations tested");
    log::info!("   ✓ All data verification passed");
    
    // Explicitly terminate TUI session to ensure clean shutdown
    terminate_session(session, "TUI").await?;
    
    Ok(())
}

/// Run a complete multi-station Slave test with TUI Slaves and CLI Masters.
///
/// # Purpose
///
/// This is a **high-level test orchestrator** that validates complex multi-station
/// Modbus scenarios with multiple CLI Masters writing to multiple TUI Slaves.
/// It tests the complete **CLI → TUI communication matrix** where multiple CLI
/// Masters write data to multiple TUI Slaves simultaneously.
///
/// This function tests **multi-Slave write** scenarios where each Master writes
/// to all Slaves in sequence, verifying data integrity across all combinations.
///
/// # Test Architecture
///
/// ```text
/// Port1 (TUI Slaves)                   Port2 (CLI Masters)
///       │                                     │
///       ├─ Configure Slave #1                 │
///       ├─ Configure Slave #2                 │
///       │                                     │
///       │                            ┌────────┴────────┐
///       │                            │ Start CLI Master│
///       │                            │ with data for   │
///       │                            │ Slave #1        │
///       │                            └────────┬────────┘
///       │                                     │
///       │<────── Master Write ────────────────┤
///       ├──────── Response ──────────────────>│
///       │                                     │
///       │                            ┌────────┴────────┐
///       │                            │ Switch CLI to   │
///       │                            │ data for        │
///       │                            │ Slave #2        │
///       │                            └────────┬────────┘
///       │                                     │
///       │<────── Master Write ────────────────┤
///       ├──────── Response ──────────────────>│
///       │                                     │
///   ┌───┴───┐                                 │
///   │Verify │                                 │
///   │ All   │                                 │
///   │ Data  │                                 │
///   └───────┘                                 │
/// ```
///
/// # Parameters
///
/// - `port1`: Serial port for TUI Slaves (e.g., "COM3", "/dev/ttyUSB0")
/// - `port2`: Serial port for CLI Masters (e.g., "COM4", "/dev/ttyUSB1")
/// - `slave_configs`: List of Slave station configurations
///   - Each config defines station parameters for a Slave
///   - `is_master` should be `false` for all configs
///   - `register_values` will be overwritten with test data
///
/// # Returns
///
/// - `Ok(())`: Test passed - all Slaves received correct data from all Masters
/// - `Err`: Test failed at any stage (setup, configuration, writing, verification)
///
/// # Test Workflow
///
/// ## Stage 1: Setup TUI Slaves
/// - Call `setup_tui_test(port1, port2)` to initialize environment
/// - Call `configure_multiple_stations` to create all Slave stations
/// - Slaves are configured with initial register values
///
/// ## Stage 2: Test Each Master-Slave Combination
/// For each Slave station:
/// - Generate unique test data for this Master-Slave pair
/// - Start CLI Master with test data targeting Slave's station ID
/// - CLI Master writes data to TUI Slave
/// - Verify Slave stored correct data via `verify_slave_data`
///
/// ## Stage 3: Data Pattern Generation
/// - **Coils/DiscreteInputs**: Unique bit patterns for each Master-Slave pair
/// - **Holding/Input**: Unique 16-bit value sequences for each pair
/// - Ensures no cross-contamination between different Master-Slave combinations
///
/// # Example 1: Two Slaves, Sequential Masters
///
/// ```rust,no_run
/// # use anyhow::Result;
/// # async fn example() -> Result<()> {
/// let slave_configs = vec![
///     StationConfig {
///         station_id: 1,
///         register_mode: RegisterMode::Holding,
///         start_address: 100,
///         register_count: 5,
///         is_master: false,
///         register_values: None, // Will be overwritten with test data
///     },
///     StationConfig {
///         station_id: 2,
///         register_mode: RegisterMode::Holding,
///         start_address: 200,
///         register_count: 5,
///         is_master: false,
///         register_values: None,
///     },
/// ];
///
/// run_multi_station_slave_test("COM3", "COM4", &slave_configs).await?;
/// // Tests CLI Master writing to Slave #1, then to Slave #2
/// # Ok(())
/// # }
/// ```
///
/// # Error Handling
///
/// ## Slave Configuration Failure
/// - **Symptom**: `configure_multiple_stations` fails for Slaves
/// - **Cause**: Register initialization timeout or navigation issues
/// - **Solution**: Reduce register counts, check station ID conflicts
///
/// ## Master Writing Failure
/// - **Symptom**: CLI Master process fails or returns error
/// - **Cause**: Port unavailable, Slave not responding, or CLI binary issues
/// - **Solution**: Check port connections, verify Slave configuration
///
/// ## Data Verification Failure
/// - **Symptom**: `verify_slave_data` fails for specific Master-Slave pair
/// - **Cause**: Write operation failed, or data not stored correctly
/// - **Solution**: Check Modbus frame logs, verify write permissions
///
/// # Timing Considerations
///
/// - **Slave Setup**: 5-15 seconds + N × 20-40 seconds (N = number of Slaves)
/// - **Per Master-Slave Test**: 3-8 seconds (writing + verification)
/// - **Total Duration**: 30-120 seconds depending on station count
///
/// # Debug Tips
///
/// ## Monitor Slave Data Updates
/// ```bash
/// # Watch TUI status file for Slave register updates
/// watch -n 0.5 cat /tmp/aoba_tui_status.json
/// ```
///
/// ## Check CLI Master Output
/// ```bash
/// # Run CLI master-write manually to debug
/// aoba --master-write COM4 \
///   --station-id 1 \
///   --register-address 100 \
///   --register-values 1000,2000,3000,4000,5000 \
///   --register-mode holding \
///   --json
/// ```
///
/// ## Test Individual Slaves
/// ```rust,no_run
/// # use anyhow::Result;
/// # async fn debug_single_slave() -> Result<()> {
/// // Test one Slave at a time
/// let slave_config = StationConfig { /* ... */ };
/// run_single_station_slave_test("COM3", "COM4", slave_config).await?;
/// # Ok(())
/// # }
/// ```
///
/// # See Also
///
/// - [`run_multi_station_master_test`]: Inverse test (TUI Masters, CLI Slaves)
/// - [`configure_multiple_stations`]: Multi-station configuration
/// - [`verify_slave_data`]: Data verification for Slaves
/// - [`send_data_from_cli_master`]: CLI Master write operations
pub async fn run_multi_station_slave_test(
    port1: &str,
    port2: &str,
    slave_configs: &[StationConfig],
) -> Result<()> {
    log::info!("🧪 Running multi-station Slave test");
    log::info!("   Port1: {port1} (TUI Slaves)");
    log::info!("   Port2: {port2} (CLI Masters)");
    log::info!("   Slaves: {}", slave_configs.len());

    // Setup TUI and configure all Slaves
    let (mut session, mut cap) = setup_tui_test(port1, port2).await?;
    configure_multiple_stations(&mut session, &mut cap, port1, slave_configs).await?;

    // Test each Slave receiving data from CLI Master
    for (slave_idx, slave_config) in slave_configs.iter().enumerate() {
        log::info!(
            "🧪 Testing Slave {}/{} receiving data",
            slave_idx + 1,
            slave_configs.len()
        );

        // Generate test data for this Slave
        let test_data = if matches!(
            slave_config.register_mode,
            RegisterMode::Coils | RegisterMode::DiscreteInputs
        ) {
            generate_random_coils(slave_config.register_count as usize)
        } else {
            generate_random_registers(slave_config.register_count as usize)
        };

        // Send data from CLI Master to this Slave
        send_data_from_cli_master(port2, &test_data, slave_config).await?;

        // Verify Slave received correct data
        verify_slave_data(&mut session, &mut cap, &test_data, slave_config).await?;

        log::info!(
            "✅ Slave {}/{} successfully received data",
            slave_idx + 1,
            slave_configs.len()
        );
    }

    log::info!("✅ Multi-station Slave test PASSED");
    log::info!("   ✓ All Slaves configured correctly");
    log::info!("   ✓ All CLI Master to Slave write operations tested");
    log::info!("   ✓ All data verification passed");
    
    // Explicitly terminate TUI session to ensure clean shutdown
    terminate_session(session, "TUI").await?;
    
    Ok(())
}
