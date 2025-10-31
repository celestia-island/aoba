use anyhow::{anyhow, Result};

use super::super::{
    config::{RegisterMode, StationConfig},
    navigation::{configure_tui_station, navigate_to_modbus_panel, setup_tui_test},
    status_paths::wait_for_station_count,
};
use super::cli::{send_data_from_cli_master, verify_master_data, verify_slave_data};
use aoba_ci_utils::*;

/// Run a complete single-station Master test with TUI Master and CLI Slave.
///
/// # Purpose
///
/// This is a **high-level test orchestrator** that validates the complete Modbus
/// Master workflow using the shared navigation, status, and CLI helpers:
/// 1. Launch the TUI and configure a Master station via fine-grained status checks
/// 2. Wait for the TUI-managed CLI subprocess to report readiness
/// 3. Issue a CLI `--slave-poll` health check against the runtime data source
/// 4. Assert that the returned payload matches the expected register snapshot
///
/// This function tests the **TUI → CLI communication path** where the TUI acts
/// as Master and initiates read operations.
///
/// ## Execution Plan (Aligned with TUI Internals)
///
/// 1. **Initialization & Page Transition**: Call `setup_tui_test`, which wraps the
///    keyboard handlers in `src/tui/ui/pages/entry` and `config_panel`, press `Enter`
///    to enter the configuration view, and confirm `Page::ConfigPanel` in the status dump via
///    `wait_for_tui_page("ConfigPanel")`.
/// 2. **Enter Modbus Dashboard**: Execute `navigate_to_modbus_panel`, which relies on
///    `aoba_ci_utils::navigate_to_vcom` and `src/tui/ui/pages/modbus_panel/input/navigation.rs`
///    (`handle_enter_action`) while double-checking both terminal frames and the status tree to
///    ensure we land on `Page::ModbusDashboard`.
/// 3. **Ensure Connection Mode**: `configure_tui_station` first invokes `ensure_connection_mode`
///    (see `station/connection.rs`), using the `ModbusDashboardCursor::ModbusMode` branch so that
///    the runtime `ModbusConnectionMode` matches `StationConfig::is_master()`.
/// 4. **Edit Station Fields**: Sequentially call `configure_station_id`, `configure_register_type`,
///    `configure_start_address`, and `configure_register_count` (implemented in
///    `station/configure.rs`, backed by `src/tui/ui/pages/modbus_panel/input/editing.rs`), with
///    every step validated through `execute_with_status_checks` on the status JSON.
/// 5. **Save & Enable Port**: Trigger `Ctrl+S` via `save_configuration_and_verify`, which flows
///    through `navigation.rs::handle_save_config` → `UiToCore::ToggleRuntime` →
///    `src/tui/subprocess.rs::start_subprocess`, while simultaneously watching
///    `wait_for_port_enabled`/`verify_port_enabled` to confirm both the status file and the title bar
///    (`src/tui/ui/title.rs`).
/// 6. **Confirm CLI Subprocess & Recover if Needed**: Read `/tmp/ci_cli_*_status.json` with
///    `wait_for_cli_status`; on timeout, run `scripts/socat_init.sh` to reset ports and retry.
///    Verify `CliMode` resolves to `MasterProvide` for Master flows or `SlavePoll` for Slave flows.
/// 7. **Master Data Verification**: For Master scenarios, call `verify_master_data` to launch the
///    CLI `--slave-poll`, ensuring the synthesized data source from the subprocess is externally
///    reachable (default expectation: zeroed registers).
/// 8. **Slave Data Verification**: For Slave scenarios, first call `send_data_from_cli_master` to
///    provide deterministic values, then run `verify_slave_data` against the TUI status tree to
///    confirm the configuration and write-back metadata.
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
///     c1 -->|Response · test data| t5
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
/// ## Stage 1: Determine Expected Data
/// - Default expectation uses zeroed values that mirror the freshly created data source
/// - Future enhancements may inject randomized snapshots for regression coverage
///
/// ## Stage 2: Setup TUI Master
/// - Call `setup_tui_test(port1, port2)` to initialize environment
/// - Call `navigate_to_modbus_panel` to reach Modbus dashboard
/// - Call `configure_tui_station` with test data to create Master station
///
/// ## Stage 3: Wait for CLI Subprocess
/// - Use `wait_for_station_count` to ensure the station is committed in the status dump
/// - Call `wait_for_cli_status` so the CLI `master-provide-persist` helper reports readiness
/// - Allow the runtime a brief grace period to finish IPC handshakes
///
/// ## Stage 4: Verify Data Path
/// - Call `verify_master_data` to run `aoba --slave-poll` against `port2`
/// - Inspect the JSON output and ensure the provided values match expectations
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
///     register_values: None,
/// };
///
/// run_single_station_master_test("COM3", "COM4", master_config).await?;
/// // Test configures a Master station and verifies the CLI subprocess answers a poll
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
/// // Test configures a Coils station and confirms the CLI subprocess responds
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
/// - **Symptom**: `configure_tui_station` fails during field edits or save
/// - **Cause**: Navigation drift, register edit timeout, or values not persisted
/// - **Solution**: Increase edit timeouts, ensure register counts align with expectations
///
/// ## CLI Subprocess Readiness Failure
/// - **Symptom**: `wait_for_cli_status` times out or fails to locate the helper dump
/// - **Cause**: Debug CI flag missing, subprocess startup delay, or virtual port conflicts
/// - **Solution**: Confirm `--debug-ci-e2e-test` is enabled, rerun `scripts/socat_init.sh`, review TUI logs

/// ## CLI Health Check Failure
/// - **Symptom**: `verify_master_data` fails with CLI error or value mismatch
/// - **Cause**: CLI binary missing, subprocess not yet started, or serial contention
/// - **Solution**: Confirm the status snapshot lists the station, rerun after
///   `scripts/socat_init.sh` to rebuild virtual ports, inspect CLI stderr for permission errors
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
/// - [`verify_master_data`]: CLI `--slave-poll` health check for Master subprocesses
/// - [`send_data_from_cli_master`]: Function to supply data via CLI master-provide (Slave tests)
/// - [`run_single_station_slave_test`]: Inverse test (CLI Master, TUI Slave)
/// - [`configure_tui_station`]: Underlying station configuration
/// - [`setup_tui_test`]: Environment initialization
pub async fn run_single_station_master_test(
    port1: &str,
    port2: &str,
    config: StationConfig,
    screenshot_ctx: &ScreenshotContext,
) -> Result<()> {
    use super::super::screenshot_integration::*;
    
    log::info!("🧪 Running single-station Master test");
    log::info!("   Port1: {port1} (TUI Master)");
    log::info!("   Port2: {port2} (CLI Slave)");
    log::info!("   Config: {config:?}");

    reset_snapshot_placeholders();

    // Setup TUI and ensure we are fully inside ConfigPanel before proceeding.
    let (mut session, mut cap) = setup_tui_test(port1, port2, Some(screenshot_ctx)).await?;

    // Navigate to Modbus panel and confirm dashboard activation.
    navigate_to_modbus_panel(&mut session, &mut cap, port1).await?;
    wait_for_tui_page("ModbusDashboard", 10, None).await?;
    
    // Screenshot: After entering Modbus panel
    screenshot_after_modbus_panel(&mut session, &mut cap, port1, Some(screenshot_ctx)).await?;

    // Configure the target station using reusable workflow helpers.
    configure_tui_station(&mut session, &mut cap, port1, &config).await?;

    // Station count and configuration must be visible in the status dump.
    wait_for_station_count(port1, true, 1, 10).await?;
    wait_for_modbus_config(port1, true, config.station_id(), 10, None).await?;
    
    // Screenshot: After configuring station
    screenshot_after_station_config(
        &mut session,
        &mut cap,
        port1,
        config.station_id(),
        config.register_mode(),
        config.start_address(),
        config.register_count() as usize,
        config.is_master(),
        Some(screenshot_ctx),
    ).await?;

    // Persisted configuration should enable the port; verify status JSON and UI indicator.
    wait_for_port_enabled(port1, 20, Some(500)).await?;
    verify_port_enabled(&mut session, &mut cap, "master_port_enabled").await?;
    
    // Screenshot: After port is enabled
    screenshot_after_port_enabled(
        &mut session,
        &mut cap,
        port1,
        config.station_id(),
        config.register_mode(),
        config.start_address(),
        config.register_count() as usize,
        config.is_master(),
        Some(screenshot_ctx),
    ).await?;

    // Ensure the managed CLI subprocess is running in MasterProvide mode, retrying with socat reset when needed.
    let cli_status = wait_for_cli_status_with_recovery(port1, 15, Some(500)).await?;
    if cli_status.mode != CliMode::MasterProvide {
        return Err(anyhow!(
            "CLI subprocess for {port1} expected MasterProvide but observed {:?}",
            cli_status.mode
        ));
    }
    if cli_status.station_id != config.station_id() {
        return Err(anyhow!(
            "CLI station id mismatch: expected {}, got {}",
            config.station_id(),
            cli_status.station_id
        ));
    }
    if cli_status.register_length != config.register_count() {
        return Err(anyhow!(
            "CLI register length mismatch: expected {}, got {}",
            config.register_count(),
            cli_status.register_length
        ));
    }
    if cli_status.register_address != config.start_address() {
        return Err(anyhow!(
            "CLI register address mismatch: expected {}, got {}",
            config.start_address(),
            cli_status.register_address
        ));
    }
    if cli_status.register_mode != config.register_mode() {
        return Err(anyhow!(
            "CLI register mode mismatch: expected {:?}, got {:?}",
            config.register_mode(),
            cli_status.register_mode
        ));
    }

    log::info!("✅ CLI master-provide subprocess reported ready");

    // Allow subprocess to settle before issuing poll.
    sleep_3s().await;

    // Initial master data set defaults to zeros until operators push updates.
    let expected_data = vec![0u16; config.register_count() as usize];

    // Poll MasterProvide helper via CLI slave-poll to ensure runtime responds.
    verify_master_data(port2, &expected_data, &config).await?;

    log::info!("✅ Single-station Master runtime responded to CLI poll");

    // Explicitly terminate TUI session to ensure clean shutdown.
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
/// - TUI writes test data to Slave registers and publishes status updates via `wait_for_station_count`
/// - Confirm the background `slave-poll-persist` subprocess is running with `wait_for_cli_status`
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

/// ## CLI Subprocess Readiness Failure
/// - **Symptom**: `wait_for_cli_status` times out waiting for the background helper
/// - **Cause**: Debug CI mode disabled, subprocess crash, or virtual port misconfiguration
/// - **Solution**: Confirm `--debug-ci-e2e-test` is set, rerun `scripts/socat_init.sh`, inspect TUI logs
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
    screenshot_ctx: &ScreenshotContext,
) -> Result<()> {
    log::info!("🧪 Running single-station Slave test");
    log::info!("   Port1: {port1} (TUI Slave)");
    log::info!("   Port2: {port2} (CLI data provider)");
    log::info!("   Config: {config:?}");

    reset_snapshot_placeholders();

    // Ensure virtual serial ports are initialized and not left in a busy state from previous runs.
    if reset_virtual_serial_ports().await? {
        sleep_1s().await;
    }

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

    match config.register_mode() {
        RegisterMode::Coils | RegisterMode::DiscreteInputs => {
            register_snapshot_switch_values(&test_data);
        }
        RegisterMode::Holding | RegisterMode::Input => {
            register_snapshot_hex_values(&test_data);
        }
    }

    // Create config with test data
    let mut config_with_data = config.clone();
    config_with_data.set_register_values(Some(test_data.clone()));

    // Setup TUI and confirm ConfigPanel is ready for interaction.
    let (mut session, mut cap) = setup_tui_test(port1, port2, Some(screenshot_ctx)).await?;

    // Navigate to Modbus panel and guarantee dashboard context.
    navigate_to_modbus_panel(&mut session, &mut cap, port1).await?;
    wait_for_tui_page("ModbusDashboard", 10, None).await?;

    // Configure the Slave station with generated data.
    configure_tui_station(&mut session, &mut cap, port1, &config_with_data).await?;

    // Verify station presence and configuration in the status dump.
    wait_for_station_count(port1, false, 1, 10).await?;
    wait_for_modbus_config(port1, false, config_with_data.station_id(), 10, None).await?;

    // Saving configuration should enable the port; verify JSON status and visual indicator.
    wait_for_port_enabled(port1, 20, Some(500)).await?;
    verify_port_enabled(&mut session, &mut cap, "slave_port_enabled").await?;

    // Ensure the CLI helper is in SlavePoll mode, retrying with port reset if needed.
    let cli_status = wait_for_cli_status_with_recovery(port1, 15, Some(500)).await?;
    if cli_status.mode != CliMode::SlavePoll {
        return Err(anyhow!(
            "CLI subprocess for {port1} expected SlavePoll but observed {:?}",
            cli_status.mode
        ));
    }
    if cli_status.station_id != config_with_data.station_id() {
        return Err(anyhow!(
            "CLI station id mismatch: expected {}, got {}",
            config_with_data.station_id(),
            cli_status.station_id
        ));
    }
    if cli_status.register_length != config_with_data.register_count() {
        return Err(anyhow!(
            "CLI register length mismatch: expected {}, got {}",
            config_with_data.register_count(),
            cli_status.register_length
        ));
    }
    if cli_status.register_address != config_with_data.start_address() {
        return Err(anyhow!(
            "CLI register address mismatch: expected {}, got {}",
            config_with_data.start_address(),
            cli_status.register_address
        ));
    }
    if cli_status.register_mode != config_with_data.register_mode() {
        return Err(anyhow!(
            "CLI register mode mismatch: expected {:?}, got {:?}",
            config_with_data.register_mode(),
            cli_status.register_mode
        ));
    }

    log::info!("✅ CLI slave-poll subprocess reported ready");

    // Allow slave poller to settle.
    sleep_3s().await;

    // Send data from CLI Master and verify the TUI slave consumed it.
    send_data_from_cli_master(port2, &test_data, &config).await?;
    verify_slave_data(&mut session, &mut cap, &test_data, &config_with_data).await?;

    log::info!("✅ Single-station Slave test PASSED");
    log::info!("   ✓ Configuration UI working correctly");
    log::info!("   ✓ Field navigation validated");
    log::info!("   ✓ Data entry successful");
    log::info!("   ✓ Save operation completed");
    log::info!("   ✓ CLI responder served expected data");

    // Explicitly terminate TUI session to ensure clean shutdown.
    terminate_session(session, "TUI").await?;

    Ok(())
}

async fn wait_for_cli_status_with_recovery(
    port_name: &str,
    timeout_secs: u64,
    retry_interval_ms: Option<u64>,
) -> Result<CliStatus> {
    match wait_for_cli_status(port_name, timeout_secs, retry_interval_ms).await {
        Ok(status) => Ok(status),
        Err(err) => {
            log::warn!(
                "⚠️  CLI status wait failed for {port_name}: {err}. Attempting port reset via socat_init.sh"
            );
            let original_err = err;
            if reset_virtual_serial_ports().await? {
                sleep_1s().await;
                wait_for_cli_status(port_name, timeout_secs, retry_interval_ms)
                    .await
                    .map_err(|retry_err| {
                        anyhow!(
                            "CLI status unavailable after port reset: {retry_err} (original: {original_err})"
                        )
                    })
            } else {
                Err(original_err)
            }
        }
    }
}

async fn reset_virtual_serial_ports() -> Result<bool> {
    #[cfg(not(unix))]
    {
        log::info!(
            "ℹ️  Skipping socat_init.sh reset because virtual serial ports are not supported on this platform"
        );
        return Ok(false);
    }

    #[cfg(unix)]
    {
        use std::{path::PathBuf, process::Command};

        let script_candidates = [
            PathBuf::from("scripts/socat_init.sh"),
            PathBuf::from("../../scripts/socat_init.sh"),
        ];

        let script_path = script_candidates
            .into_iter()
            .find(|candidate| candidate.exists());

        let Some(path) = script_path else {
            log::warn!("⚠️  socat_init.sh not found; skipping port reset");
            return Ok(false);
        };

        log::info!(
            "🔁 Running socat_init.sh to reset virtual ports: {}",
            path.display()
        );

        let output = tokio::task::spawn_blocking(move || {
            Command::new("bash")
                .arg(&path)
                .arg("--mode")
                .arg("tui")
                .output()
        })
        .await??;

        if output.status.success() {
            log::info!("✅ socat_init.sh completed successfully");
            Ok(true)
        } else {
            log::warn!(
                "⚠️  socat_init.sh failed (status {}):\nstdout: {}\nstderr: {}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            Ok(false)
        }
    }
}
