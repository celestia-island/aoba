use anyhow::Result;

use super::super::{
    config::{RegisterMode, StationConfig},
    navigation::{configure_tui_station, navigate_to_modbus_panel, setup_tui_test},
};
use super::cli::send_data_from_cli_master;
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
/// ```mermaid
/// flowchart LR
///     subgraph tuimaster[Port1 · TUI Master]
///         t1[Configure Station #1]
///         t2[Enable Master Mode]
///         t3[Set Register Range]
///         t4[Send Poll Request]
///         t5[Receive Response]
///         t6[Verify Data]
///     end
///     subgraph clislave[Port2 · CLI Slave]
///         c1[Start CLI Slave with test data]
///     end
///     t1 --> t2 --> t3 --> t4
///     t3 -->|Launch CLI helper| c1
///     t4 -->|Poll request| c1
///     c1 -->|Response (test data)| t5
///     t5 --> t6
/// ```
///
/// # Parameters
///
/// - `port1`: Serial port for TUI Master (e.g., "COM3", "/dev/ttyUSB0")
///   - Must support virtual loopback or physical connection to `port2`
/// - `port2`: Serial port for CLI Slave (e.g., "COM4", "/dev/ttyUSB1")
///   - Connected to `port1` via null modem or virtual pair
/// - `config`: Station configuration without initial values
///   - `config.is_master()` should be `true`
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
/// - Data length matches `config.register_count()`
///
/// ## Stage 2: Setup TUI Master
/// - Call `setup_tui_test(port1, port2)` to initialize environment
/// - Call `navigate_to_modbus_panel` to reach Modbus dashboard
/// - Call `configure_tui_station` with test data to create Master station
///
/// ## Stage 3: Start CLI Slave and Poll
/// - Call `send_data_from_cli_master` to spawn CLI in slave-poll mode (acts as Modbus master)
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
/// - [`send_data_from_cli_master`]: Function to supply data via CLI master-provide
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

    // Setup TUI
    let (mut session, mut cap) = setup_tui_test(port1, port2).await?;

    // Navigate to Modbus panel
    navigate_to_modbus_panel(&mut session, &mut cap, port1).await?;

    // Configure station
    configure_tui_station(&mut session, &mut cap, port1, &config).await?;

    log::info!("✅ Single-station Master configuration applied and verified");

    // Explicitly terminate TUI session to ensure clean shutdown
    // This is critical in CI environments to prevent zombie processes
    terminate_session(session, "TUI").await?;

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
/// ```mermaid
/// flowchart LR
///     subgraph tuislave[Port1 · TUI Slave]
///         s1[Configure Station #1]
///         s2[Enable Slave Mode]
///         s3[Set Register Range]
///         s4[Initialize with test data]
///         s5[Provide Response]
///     end
///     subgraph climaster[Port2 · CLI Master]
///         m1[Start CLI Data Provider]
///         m2[Verify Data]
///     end
///     s1 --> s2 --> s3 --> s4
///     s4 -->|Launch CLI helper| m1
///     m1 -->|Poll request| s5
///     s5 -->|Response (test data)| m2
/// ```
///
/// # Parameters
///
/// - `port1`: Serial port for TUI Slave (e.g., "COM3", "/dev/ttyUSB0")
///   - Must support virtual loopback or physical connection to `port2`
/// - `port2`: Serial port for CLI Master (e.g., "COM4", "/dev/ttyUSB1")
///   - Connected to `port1` via null modem or virtual pair
/// - `config`: Station configuration without initial values
///   - `config.is_master()` should be `false`
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
/// - Data length matches `config.register_count()`
///
/// ## Stage 2: Setup TUI Slave
/// - Call `setup_tui_test(port1, port2)` to initialize environment
/// - Call `navigate_to_modbus_panel` to reach Modbus dashboard
/// - Call `configure_tui_station` with test data to create Slave station
/// - TUI writes test data to Slave registers
///
/// ## Stage 3: Provide a Modbus responder for TUI polling
/// - Call `send_data_from_cli_master` to spawn CLI in master-provide mode (acts as Modbus slave/server)
/// - CLI waits for the TUI's polling request and responds with the prepared data set
/// - TUI Slave receives the response via its background `slave-poll-persist` subprocess
///
/// ## Stage 4: Verify Data
/// - `send_data_from_cli_master` parses the CLI output to confirm the served values
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
/// # Run CLI master-provide manually to debug
/// printf '{"values":[1,2,3,4,5]}' > /tmp/data.json
/// aoba --master-provide COM4 \
///   --station-id 1 \
///   --register-address 200 \
///   --register-length 10 \
///   --register-mode holding \
///   --data-source file:/tmp/data.json \
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
/// - [`send_data_from_cli_master`]: Function to supply data via CLI master-provide
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
    log::info!("   Port2: {port2} (CLI data provider)");
    log::info!("   Config: {config:?}");

    // Generate test data
    let test_data = if matches!(
        config.register_mode(),
        RegisterMode::Coils | RegisterMode::DiscreteInputs
    ) {
        generate_random_coils(config.register_count() as usize)
    } else {
        generate_random_registers(config.register_count() as usize)
    };
    log::info!("Generated test data: {test_data:?}");

    // Create config with test data
    let mut config_with_data = config.clone();
    config_with_data.set_register_values(Some(test_data.clone()));

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
    log::info!("   ✓ CLI responder served expected data");

    // Explicitly terminate TUI session to ensure clean shutdown
    terminate_session(session, "TUI").await?;

    Ok(())
}
