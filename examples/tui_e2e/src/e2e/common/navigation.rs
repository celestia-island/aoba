use anyhow::{anyhow, Result};

use expectrl::Expect;

use super::{
    config::StationConfig,
    retry::execute_transaction_with_retry,
    station::{
        configure_register_count, configure_register_type, configure_start_address,
        configure_station_id, create_station, ensure_connection_mode, focus_create_station_button,
        focus_station, initialize_slave_registers, save_configuration_and_verify,
    },
    validation::check_station_config,
};
use ci_utils::*;

fn is_default_master_station(station: &TuiModbusMaster) -> bool {
    station.station_id == 1
        && station.register_type == "Holding"
        && station.start_address == 0
        && station.register_count <= 1
}

fn is_default_slave_station(station: &TuiModbusSlave) -> bool {
    station.station_id == 1
        && station.register_type == "Holding"
        && station.start_address == 0
        && station.register_count <= 1
}

/// Setup TUI test environment with initialized session and terminal capture.
///
/// # Purpose
///
/// This is the **primary initialization function** for all TUI E2E tests. It:
/// 1. Validates serial port availability
/// 2. Spawns the TUI process in debug CI mode **with `--no-config-cache`**
/// 3. Waits for TUI initialization (3 seconds + page detection)
/// 4. Navigates from Entry page to ConfigPanel
/// 5. Returns ready-to-use session and capture objects
///
/// # Configuration Cache Handling
///
/// TUI is started with `--no-config-cache` flag, which disables loading and saving
/// of `aoba_tui_config.json`. This ensures each test starts with a completely clean
/// state without interference from previous test runs. No manual cache cleanup is needed.
///
/// # Parameters
///
/// - `port1`: Primary serial port name (e.g., "COM3", "/dev/ttyUSB0")
///   - Must exist and be accessible
///   - Used for main Modbus operations in tests
/// - `_port2`: Secondary port (currently unused, reserved for future multi-port tests)
///   - Prefix `_` indicates intentional non-use
///
/// # Returns
///
/// - `Ok((session, capture))`: Tuple of initialized TUI session and terminal capture
///   - `session`: `impl Expect` - Expectrl session for sending commands and reading output
///   - `capture`: `TerminalCapture` - Screen capture tool configured with Small size (80x24)
/// - `Err`: Port doesn't exist, TUI spawn failed, or initialization timeout
///
/// # Timing Behavior
///
/// - **TUI Spawn**: Immediate
/// - **Initial Wait**: 3 seconds (hard-coded for TUI startup)
/// - **Entry Page Wait**: Up to 10 seconds (via `wait_for_tui_page`)
/// - **ConfigPanel Navigation**: 1 second sleep after Enter key
/// - **ConfigPanel Wait**: Up to 10 seconds (via `wait_for_tui_page`)
/// - **Total Duration**: ~5-15 seconds depending on system performance
///
/// # Example: Basic Usage
///
/// ```rust,no_run
/// # use anyhow::Result;
/// # async fn example() -> Result<()> {
/// let (mut session, mut cap) = setup_tui_test("COM3", "COM4").await?;
///
/// // Session is now on ConfigPanel, ready for operations
/// // Example: Navigate to a port
/// navigate_to_modbus_panel(&mut session, &mut cap, "COM3").await?;
/// # Ok(())
/// # }
/// ```
///
/// # Example: Full Test Workflow
///
/// ```rust,no_run
/// # use anyhow::Result;
/// # async fn test_modbus_station() -> Result<()> {
/// use ci_utils::CursorAction;
///
/// // Step 1: Initialize TUI environment
/// let (mut session, mut cap) = setup_tui_test("COM3", "COM4").await?;
///
/// // Step 2: Navigate to Modbus panel for COM3
/// navigate_to_modbus_panel(&mut session, &mut cap, "COM3").await?;
///
/// // Step 3: Configure a Master station
/// let master_config = StationConfig {
///     station_id: 1,
///     register_mode: RegisterMode::Holding,
///     start_address: 100,
///     register_count: 10,
///     is_master: true,
///     register_values: None,
/// };
/// configure_tui_station(&mut session, &mut cap, "COM3", &master_config).await?;
///
/// // Step 4: Perform test operations...
/// # Ok(())
/// # }
/// ```
///
/// # Error Handling
///
/// This function can fail at several stages:
///
/// - **Port Validation**: `"Port {port1} does not exist"`
///   - Check port name is correct and device is connected
///   - Use `list_ports()` CLI command to verify available ports
///
/// - **TUI Spawn Failure**: `spawn_expect_process` error
///   - Verify AOBA binary is built and in PATH
///   - Check permissions for terminal access
///
/// - **Entry Page Timeout**: `wait_for_tui_page` timeout after 10 seconds
///   - TUI may be stuck or slow to start
///   - Check system resources (CPU, memory)
///   - Review TUI logs for startup errors
///
/// - **ConfigPanel Navigation**: Unexpected screen state
///   - TUI may have shown error dialog or unexpected page
///   - Capture screenshot to debug navigation state
///
/// # Debug Tips
///
/// ## TUI Not Starting
/// ```bash
/// # Verify AOBA is built and accessible
/// cargo build --release
/// ./target/release/aoba --version
///
/// # Check for port conflicts
/// lsof /dev/ttyUSB0  # Unix
/// mode COM3          # Windows
/// ```
///
/// ## Timing Issues
/// If tests fail intermittently, adjust waits:
/// - Increase initial sleep from 3 to 5 seconds for slow systems
/// - Increase `wait_for_tui_page` timeout from 10 to 20 seconds
/// - Add extra sleeps after navigation actions
///
/// ## Capture Debugging
/// ```rust,no_run
/// # use anyhow::Result;
/// # async fn debug_setup() -> Result<()> {
/// # let mut session = todo!();
/// # let mut cap = todo!();
/// // After setup, capture screen to verify state
/// let screen = cap.capture(&mut session, "after_setup").await?;
/// println!("Current screen:\n{}", screen);
/// # Ok(())
/// # }
/// ```
///
/// # See Also
///
/// - [`navigate_to_modbus_panel`]: Next step after setup to enter port-specific Modbus panel
/// - [`configure_tui_station`]: Configure station after reaching Modbus panel
/// - [`wait_for_tui_page`]: Underlying page detection function
/// - [`spawn_expect_process`]: Low-level process spawning (from ci_utils)
pub async fn setup_tui_test(port1: &str, _port2: &str) -> Result<(impl Expect, TerminalCapture)> {
    log::info!("🔧 Setting up TUI test environment for port {port1}");

    // Verify port exists
    if !port_exists(port1) {
        return Err(anyhow!("Port {port1} does not exist"));
    }

    // Spawn TUI with debug mode and no-config-cache enabled
    // The --no-config-cache flag prevents TUI from loading/saving aoba_tui_config.json
    // This ensures each test starts with a clean state
    log::info!("Starting TUI in debug mode with --no-config-cache...");
    let mut tui_session =
        spawn_expect_process(&["--tui", "--debug-ci-e2e-test", "--no-config-cache"])?;
    let mut tui_cap = TerminalCapture::with_size(TerminalSize::Small);

    // Wait for TUI to initialize
    sleep_3s().await;

    // Wait for TUI to reach Entry page
    log::info!("Waiting for TUI Entry page...");
    wait_for_tui_page("Entry", 10, None).await?;

    // Navigate to ConfigPanel
    log::info!("Navigating to ConfigPanel...");
    let actions = vec![CursorAction::PressEnter, CursorAction::Sleep1s];
    execute_cursor_actions(
        &mut tui_session,
        &mut tui_cap,
        &actions,
        "enter_config_panel",
    )
    .await?;

    // Wait for ConfigPanel page
    wait_for_tui_page("ConfigPanel", 10, None).await?;

    log::info!("✅ TUI test environment ready");
    Ok((tui_session, tui_cap))
}

/// Navigate to specific serial port and enter its Modbus panel.
///
/// # Purpose
///
/// This is the **second initialization step** after `setup_tui_test`. It navigates
/// from the ConfigPanel (showing all serial ports) to a specific port's Modbus
/// dashboard, where stations can be created and configured.
///
/// # Navigation Flow
///
/// ```text
/// Entry Page
///   ↓ [PressEnter]
/// ConfigPanel (COM Ports list)
///   ↓ [Navigate to target port]
/// ConfigPanel (port selected)
///   ↓ [PressEnter]
/// ModbusDashboard (Modbus station management)
/// ```
///
/// # Parameters
///
/// - `session`: Active TUI session from `setup_tui_test`
/// - `cap`: Terminal capture tool from `setup_tui_test`
/// - `port1`: Target serial port name (e.g., "COM3", "/dev/ttyUSB0")
///   - Must be visible in ConfigPanel port list
///   - Port should be available (not in use by other process)
///
/// # Returns
///
/// - `Ok(())`: Successfully navigated to ModbusDashboard for the target port
/// - `Err`: Navigation failed (port not found, timeout, or unexpected state)
///
/// # Transaction Retry Stages
///
/// This function uses three **transaction retry stages** with safe rollback:
///
/// ## Stage 1: Entry → ConfigPanel
/// - **Actions**: Press Enter, wait 1.5s
/// - **Verification**: Screen contains "Configuration" or "Serial" (not "Welcome")
/// - **Rollback**: None (no reset needed from Entry)
/// - **Retry Logic**: Up to 3 attempts if still on Entry page
///
/// ## Stage 2: Navigate to Target Port
/// - **Delegated**: `navigate_to_vcom(session, cap, port1)`
/// - **Purpose**: Move cursor to specific port in the list
/// - **See**: Helper function handles Up/Down arrow navigation
///
/// ## Stage 3: Enter Modbus Panel
/// - **Actions**: Press Enter, wait 1.5s
/// - **Verification**: Screen contains "Station" or "Create" (entered ModbusDashboard)
/// - **Rollback**: Press Escape to return to ConfigPanel
/// - **Reset Navigation**: Wait 500ms before retry
/// - **Retry Logic**: Up to 3 attempts if still on ConfigPanel
///
/// # Example 1: Basic Usage After Setup
///
/// ```rust,no_run
/// # use anyhow::Result;
/// # async fn example() -> Result<()> {
/// // Step 1: Initialize TUI
/// let (mut session, mut cap) = setup_tui_test("COM3", "COM4").await?;
///
/// // Step 2: Navigate to Modbus panel for COM3
/// navigate_to_modbus_panel(&mut session, &mut cap, "COM3").await?;
///
/// // Now ready for station operations
/// # Ok(())
/// # }
/// ```
///
/// # Example 2: Full Workflow with Station Creation
///
/// ```rust,no_run
/// # use anyhow::Result;
/// # async fn test_station_creation() -> Result<()> {
/// use ci_utils::CursorAction;
///
/// // Initialize environment
/// let (mut session, mut cap) = setup_tui_test("COM3", "COM4").await?;
///
/// // Navigate to Modbus panel
/// navigate_to_modbus_panel(&mut session, &mut cap, "COM3").await?;
///
/// // Verify we're on ModbusDashboard
/// let screen = cap.capture(&mut session, "verify_dashboard").await?;
/// assert!(screen.contains("Station") || screen.contains("Create"));
///
/// // Create a Master station
/// let master_config = StationConfig {
///     station_id: 1,
///     register_mode: RegisterMode::Holding,
///     start_address: 100,
///     register_count: 10,
///     is_master: true,
///     register_values: None,
/// };
/// configure_tui_station(&mut session, &mut cap, "COM3", &master_config).await?;
/// # Ok(())
/// # }
/// ```
///
/// # Error Handling
///
/// ## Port Not Found
/// - **Symptom**: `navigate_to_vcom` fails with "Port not found" error
/// - **Cause**: Port name doesn't match any entry in ConfigPanel list
/// - **Solution**: Verify port name spelling, check port is connected and visible to OS
///
/// ## Entry → ConfigPanel Timeout
/// - **Symptom**: Transaction fails after 3 attempts, still seeing "Welcome" or "Press Enter"
/// - **Cause**: TUI not responding to Enter key, or slow page transition
/// - **Solution**: Increase sleep duration from 1500ms to 2000ms, check TUI logs
///
/// ## ConfigPanel → ModbusDashboard Timeout
/// - **Symptom**: Transaction fails after 3 attempts, still seeing "Serial" or port list
/// - **Cause**: Wrong port selected, or Modbus panel initialization slow
/// - **Solution**: Verify port name, check `navigate_to_vcom` selected correct port
///
/// ## Verification Timeout
/// - **Symptom**: `wait_for_tui_page("ModbusDashboard", 10, None)` times out
/// - **Cause**: Page state file not updated, or unexpected page reached
/// - **Solution**: Check TUI debug logs, verify status file path and permissions
///
/// # Timing Considerations
///
/// - **Entry → ConfigPanel**: 1.5s sleep + verification
/// - **Port Navigation**: Variable (depends on port position in list)
/// - **ConfigPanel → ModbusDashboard**: 1.5s sleep + verification
/// - **Final Verification**: Up to 10s timeout via `wait_for_tui_page`
/// - **Total Duration**: ~3-15 seconds depending on port position and system performance
///
/// # Debug Tips
///
/// ## Capture Navigation States
/// ```rust,no_run
/// # use anyhow::Result;
/// # async fn debug_navigation() -> Result<()> {
/// # let mut session = todo!();
/// # let mut cap = todo!();
/// // Before navigation
/// let before = cap.capture(&mut session, "before_nav").await?;
/// println!("Before:\n{}", before);
///
/// navigate_to_modbus_panel(&mut session, &mut cap, "COM3").await?;
///
/// // After navigation
/// let after = cap.capture(&mut session, "after_nav").await?;
/// println!("After:\n{}", after);
/// # Ok(())
/// # }
/// ```
///
/// ## Check Port Visibility
/// ```bash
/// # List available ports using AOBA CLI
/// aoba list-ports
///
/// # Expected output shows COM3 in the list
/// # COM1 - USB Serial Device
/// # COM3 - Virtual COM Port  <-- Target port
/// # COM5 - Bluetooth Device
/// ```
///
/// # See Also
///
/// - [`setup_tui_test`]: Prerequisite initialization step
/// - [`configure_tui_station`]: Next step to configure stations after navigation
/// - [`navigate_to_vcom`]: Helper for port selection in ConfigPanel
/// - [`execute_transaction_with_retry`]: Underlying transaction mechanism
/// - [`wait_for_tui_page`]: Page state verification
pub async fn navigate_to_modbus_panel<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    port1: &str,
) -> Result<()> {
    log::info!("🗺️  Navigating to port {port1} and entering Modbus panel...");

    // Step 1: Navigate to Entry -> ConfigPanel with retry
    execute_transaction_with_retry(
        session,
        cap,
        "entry_to_config_panel",
        &[CursorAction::PressEnter, CursorAction::Sleep3s],
        |screen| {
            if screen.contains("Welcome") || screen.contains("Press Enter") {
                Ok(false) // Still on Entry page
            } else if screen.contains("Configuration") || screen.contains("Serial") {
                Ok(true) // Successfully on ConfigPanel
            } else {
                Ok(false)
            }
        },
        None,
        &[], // No reset needed for Entry page
    )
    .await?;

    // Step 2: Navigate to the specific port with retry
    navigate_to_vcom(session, cap, port1).await?;

    // Step 3: Enter Modbus panel with retry
    execute_transaction_with_retry(
        session,
        cap,
        "enter_modbus_panel",
        &[CursorAction::PressEnter, CursorAction::Sleep3s],
        |screen| {
            if screen.contains("Station") || screen.contains("Create") {
                Ok(true) // Successfully in Modbus panel
            } else if screen.contains("Serial") {
                Ok(false) // Still in ConfigPanel
            } else {
                Ok(false)
            }
        },
        Some(&[CursorAction::PressEscape, CursorAction::Sleep1s]),
        &[CursorAction::Sleep1s], // Reset: just wait
    )
    .await?;

    // Verify we're in ModbusDashboard via status file
    wait_for_tui_page("ModbusDashboard", 10, None).await?;

    log::info!("✅ Successfully entered Modbus panel");
    Ok(())
}

/// Configure a single Modbus station in the TUI with complete transaction safety.
///
/// # Purpose
///
/// This is the **core configuration function** for TUI E2E tests. It orchestrates
/// the complete station setup workflow:
/// 1. Configure connection mode (Master/Slave) **before** creating station
/// 2. Create the station with proper mode from the start
/// 3. Navigate to station configuration page
/// 4. Configure register mode (Coils/DiscreteInputs/Holding/Input)
/// 5. Set start address and register count
/// 6. Initialize register values for Slave stations (if provided)
/// 7. Save configuration and exit
///
/// Each phase uses **transaction retry** with safe rollback checkpoints to ensure
/// reliability even in unstable test environments.
///
/// # Why Configure Mode First?
///
/// The TUI implementation has a critical quirk: **station mode (Master/Slave) must
/// be set BEFORE creating the station**, otherwise the mode change won't take effect
/// properly. This function follows the correct workflow to avoid mode mismatch issues.
///
/// # Parameters
///
/// - `session`: Active TUI session from `setup_tui_test` / `navigate_to_modbus_panel`
/// - `cap`: Terminal capture tool for screen reading and verification
/// - `_port1`: Port name (currently unused, kept for API compatibility)
///   - Prefix `_` indicates intentional non-use
///   - May be used in future for multi-port validation
/// - `config`: Station configuration containing all parameters
///   - See [`StationConfig`] for field descriptions
///
/// # Returns
///
/// - `Ok(())`: Station successfully configured and ready for use
/// - `Err`: Configuration failed at any phase (mode switch, creation, field edits, etc.)
///
/// # Configuration Workflow
///
/// ## Phase 1: Set Connection Mode (Master/Slave)
///
/// - **Prerequisite**: Cursor at "Create Station" button (top of ModbusDashboard)
/// - **Actions**: Navigate Down to "Connection Mode", press Right arrow to toggle Slave
/// - **Verification**: UI displays "Connection Mode Slave" via regex pattern matching
/// - **Wait**: 2s after mode change for internal state to stabilize
/// - **Reset**: Ctrl+PageUp to return to top
///
/// ## Phase 2: Create Station
///
/// - **Duplicate Prevention**: Check if station #1 already exists before creation
/// - **Actions**: Press Enter on "Create Station", wait 2s, reset to top
/// - **Verification**: Screen contains "#1" or "Station 1"
/// - **Rollback**: Ctrl+PageUp (no Escape needed, custom rollback strategy)
/// - **Retry**: Up to 3 attempts with 1s delay
///
/// ## Phase 3: Enter Station Configuration
///
/// - **Actions**: Press Down to select station #1, press Enter, wait 2s
/// - **Verification**: Screen contains "Station ID" or "Register Mode"
/// - **Rollback**: Press Escape to return to dashboard
/// - **Retry**: Up to 3 attempts
///
/// ## Phase 4: Configure Register Mode
///
/// - **Target**: "Register Mode" field
/// - **Actions**: Arrow keys to select mode (Coils/DiscreteInputs/Holding/Input)
/// - **Retry**: Each field edit has 3-attempt retry with EditMode detection
/// - **Verification**: Selected mode displayed in field
///
/// ## Phase 5: Set Start Address
///
/// - **Target**: "Start Address" field
/// - **Actions**: Type address value (e.g., "100"), Tab to next field
/// - **Verification**: Address value visible in field
/// - **Rollback**: Escape if stuck in edit mode
///
/// ## Phase 6: Set Register Count
///
/// - **Target**: "Count" field
/// - **Actions**: Type count value (e.g., "10"), Tab/Enter to confirm
/// - **Verification**: Count value visible in field
///
/// ## Phase 7: Initialize Register Values (Slave Only)
///
/// - **Condition**: `config.register_values.is_some()` and `!config.is_master`
/// - **Actions**: Navigate to register list, edit each register with provided values
/// - **Verification**: Each register shows correct value after edit
/// - **Retry**: Per-register retry on edit failures
///
/// ## Phase 8: Save and Exit
///
/// - **Actions**: Press Escape multiple times to exit configuration
/// - **Verification**: Return to ModbusDashboard
/// - **Final Checkpoint**: Verify station #1 visible in dashboard
///
/// # Example 1: Master Station with Holding Registers
///
/// ```rust,no_run
/// # use anyhow::Result;
/// # async fn example() -> Result<()> {
/// use ci_utils::CursorAction;
///
/// // Setup environment
/// let (mut session, mut cap) = setup_tui_test("COM3", "COM4").await?;
/// navigate_to_modbus_panel(&mut session, &mut cap, "COM3").await?;
///
/// // Configure Master station
/// let master_config = StationConfig {
///     station_id: 1,
///     register_mode: RegisterMode::Holding,
///     start_address: 100,
///     register_count: 10,
///     is_master: true,
///     register_values: None, // Master doesn't need initial values
/// };
///
/// configure_tui_station(&mut session, &mut cap, "COM3", &master_config).await?;
///
/// // Station ready for read operations
/// # Ok(())
/// # }
/// ```
///
/// # Example 2: Slave Station with Pre-populated Values
///
/// ```rust,no_run
/// # use anyhow::Result;
/// # async fn example() -> Result<()> {
/// // Setup environment
/// let (mut session, mut cap) = setup_tui_test("COM3", "COM4").await?;
/// navigate_to_modbus_panel(&mut session, &mut cap, "COM3").await?;
///
/// // Configure Slave station with initial values
/// let slave_config = StationConfig {
///     station_id: 2,
///     register_mode: RegisterMode::Holding,
///     start_address: 200,
///     register_count: 5,
///     is_master: false,
///     register_values: Some(vec![1000, 2000, 3000, 4000, 5000]),
/// };
///
/// configure_tui_station(&mut session, &mut cap, "COM3", &slave_config).await?;
///
/// // Slave station ready, registers contain [1000, 2000, 3000, 4000, 5000]
/// # Ok(())
/// # }
/// ```
///
/// # Example 3: Coils Configuration
///
/// ```rust,no_run
/// # use anyhow::Result;
/// # async fn example() -> Result<()> {
/// // Coils are bit-based registers (0 or 1)
/// let coil_config = StationConfig {
///     station_id: 1,
///     register_mode: RegisterMode::Coils,
///     start_address: 0,
///     register_count: 16,
///     is_master: false,
///     register_values: Some(vec![1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0]),
/// };
///
/// let (mut session, mut cap) = setup_tui_test("COM3", "COM4").await?;
/// navigate_to_modbus_panel(&mut session, &mut cap, "COM3").await?;
/// configure_tui_station(&mut session, &mut cap, "COM3", &coil_config).await?;
/// # Ok(())
/// # }
/// ```
///
/// # Transaction Safety Features
///
/// ## Duplicate Prevention
/// - Checks if station #1 already exists before attempting creation
/// - Prevents retry loops from creating multiple stations
///
/// ## Mode Verification
/// - Uses regex pattern `r"Connection Mode\s+Slave"` to verify UI state
/// - Waits 2s after mode change for internal state stabilization
/// - Prevents mode mismatch bugs that occur when mode set after creation
///
/// ## Edit Retry with Rollback
/// - Each field edit uses `execute_field_edit_with_retry`
/// - EditMode detection + pattern matching for dual verification
/// - Escape-based rollback if stuck in edit state
///
/// ## Register Value Initialization
/// - Only for Slave stations with `register_values.is_some()`
/// - Each register edit has independent retry logic
/// - Verifies value after each edit before proceeding
///
/// # Error Handling
///
/// ## Mode Switch Failure
/// - **Symptom**: Regex pattern match fails, can't find "Connection Mode Slave"
/// - **Cause**: Arrow key didn't toggle mode, or UI rendering slow
/// - **Solution**: Increase sleep from 2s to 3s, verify arrow key sent correctly
///
/// ## Station Creation Failure
/// - **Symptom**: After 3 attempts, station #1 still not visible
/// - **Cause**: Enter key not recognized, or station initialization slow
/// - **Solution**: Check TUI logs for errors, increase wait from 2s to 3s
///
/// ## Field Edit Timeout
/// - **Symptom**: `execute_field_edit_with_retry` fails after 3 attempts
/// - **Cause**: Wrong field pattern, or edit mode not detected
/// - **Solution**: Verify field pattern matches screen text exactly, check EditMode detection
///
/// ## Register Initialization Failure
/// - **Symptom**: Some registers retain default values instead of provided values
/// - **Cause**: Edit failed mid-way, or values not saved properly
/// - **Solution**: Verify register count matches values array length, check save confirmation
///
/// # Timing Considerations
///
/// - **Mode Switch**: 2s wait for internal state stabilization (critical!)
/// - **Station Creation**: 2s wait for initialization
/// - **Field Navigation**: 500ms between cursor moves
/// - **Field Edit**: Variable based on retry attempts (up to 3x with 1s delays)
/// - **Register Init**: N * field_edit_time for N registers
/// - **Total Duration**: 10-60 seconds depending on configuration complexity
///
/// # See Also
///
/// - [`StationConfig`]: Configuration structure with all parameters
/// - [`RegisterMode`]: Enum for register types (Coils, Holdings, Inputs, etc.)
/// - [`execute_field_edit_with_retry`]: Underlying field editing with retry
/// - [`execute_transaction_with_retry`]: Underlying transaction mechanism
/// - [`setup_tui_test`]: Prerequisite environment initialization
/// - [`navigate_to_modbus_panel`]: Prerequisite navigation to Modbus panel
pub async fn configure_tui_station<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    port1: &str,
    config: &StationConfig,
) -> Result<()> {
    log::info!("⚙️  Configuring TUI station: {config:?}");

    let mut status = read_tui_status()?;
    if status.ports.iter().all(|p| p.name != port1) {
        return Err(anyhow!("Port {} not found in TUI status", port1));
    }

    ensure_connection_mode(session, cap, config.is_master).await?;

    status = read_tui_status()?;

    let port = status
        .ports
        .iter()
        .find(|p| p.name == port1)
        .ok_or_else(|| anyhow!("Port {} not found in TUI status", port1))?;

    let (reuse_existing, existing_index) = if config.is_master {
        match port.modbus_masters.len() {
            0 => (false, None),
            1 if is_default_master_station(&port.modbus_masters[0]) => {
                log::info!("♻️  Reusing initial master station at index 0");
                (true, Some(0))
            }
            _ => (false, None),
        }
    } else {
        match port.modbus_slaves.len() {
            0 => (false, None),
            1 if is_default_slave_station(&port.modbus_slaves[0]) => {
                log::info!("♻️  Reusing initial slave station at index 0");
                (true, Some(0))
            }
            _ => (false, None),
        }
    };

    if !reuse_existing {
        focus_create_station_button(session, cap).await?;
    }

    let station_index = if reuse_existing {
        existing_index.unwrap()
    } else {
        create_station(session, cap, port1, config.is_master).await?
    };

    focus_station(session, cap, port1, station_index, config.is_master).await?;

    configure_station_id(
        session,
        cap,
        port1,
        station_index,
        config.station_id,
        config.is_master,
    )
    .await?;

    configure_register_type(
        session,
        cap,
        port1,
        station_index,
        config.register_mode,
        config.is_master,
    )
    .await?;

    configure_start_address(
        session,
        cap,
        port1,
        station_index,
        config.start_address,
        config.is_master,
    )
    .await?;

    configure_register_count(
        session,
        cap,
        port1,
        station_index,
        config.register_count,
        config.is_master,
    )
    .await?;

    if !config.is_master {
        if let Some(values) = &config.register_values {
            initialize_slave_registers(session, cap, values, config.register_mode).await?;
        }
    }

    if !config.is_master {
        ensure_connection_mode(session, cap, config.is_master).await?;
    }

    save_configuration_and_verify(session, cap, port1).await?;

    let final_checks = check_station_config(
        port1,
        station_index,
        config.is_master,
        config.station_id,
        config.register_mode.status_value(),
        config.start_address,
        config.register_count,
    );
    execute_cursor_actions(session, cap, &final_checks, "verify_station_config").await?;

    log::info!("✅ Station configured and verified");
    Ok(())
}
