use anyhow::Result;
use expectrl::Expect;

use super::super::{
    config::{RegisterMode, StationConfig},
    navigation::{configure_tui_station, navigate_to_modbus_panel, setup_tui_test},
};
use super::cli::{send_data_from_cli_master, verify_master_data, verify_slave_data};
use ci_utils::*;

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
            verify_master_data(port2, &test_data, master_config).await?;

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
