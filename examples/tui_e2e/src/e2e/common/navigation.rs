use anyhow::{anyhow, Result};

use expectrl::Expect;
use regex::Regex;

use super::{
    config::StationConfig,
    retry::{execute_field_edit_with_retry, execute_transaction_with_retry},
};
use ci_utils::*;

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
    _port1: &str,
    config: &StationConfig,
) -> Result<()> {
    const MAX_NUMERIC_FIELD_DIGITS: usize = 5;
    log::info!("⚙️  Configuring TUI station: {config:?}");

    // Phase 0: Ensure cursor is at AddLine (Create Station button)
    // After navigate_to_modbus_panel, cursor position is undefined
    // We must explicitly navigate to AddLine before starting configuration
    log::info!("Phase 0: Resetting cursor to AddLine (Create Station button)...");
    let actions = vec![CursorAction::PressCtrlPageUp, CursorAction::Sleep1s];
    execute_cursor_actions(session, cap, &actions, "reset_to_addline").await?;

    // Verify cursor is at Create Station button
    let screen = cap.capture(session, "verify_at_create_station").await?;
    if !screen.contains("Create Station") {
        return Err(anyhow!(
            "Expected to be at Create Station button after Ctrl+PgUp, but not found"
        ));
    }
    log::info!("✅ Cursor positioned at Create Station button");

    // Phase 1: Configure connection mode (Master/Slave) FIRST, before creating station
    // This ensures the station is created with the correct mode from the start
    log::info!(
        "Phase 1: Configuring connection mode: {}",
        if config.is_master { "Master" } else { "Slave" }
    );

    // Navigate from AddLine (Create Station) to Connection Mode field
    // Connection Mode is the field right after Create Station
    let actions = vec![
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 1,
        },
        CursorAction::Sleep1s,
    ];
    execute_cursor_actions(session, cap, &actions, "move_to_connection_mode").await?;

    // Switch to Slave if needed (default is Master)
    if !config.is_master {
        log::info!("Switching from Master to Slave mode...");

        // Enter edit mode for Connection Mode selector
        let actions = vec![CursorAction::PressEnter, CursorAction::Sleep1s];
        execute_cursor_actions(session, cap, &actions, "enter_connection_mode_edit").await?;

        // Press Right arrow to switch from Master (index 0) to Slave (index 1)
        let actions = vec![
            CursorAction::PressArrow {
                direction: ArrowKey::Right,
                count: 1,
            },
            CursorAction::Sleep1s,
        ];
        execute_cursor_actions(session, cap, &actions, "switch_to_slave_index").await?;

        // Confirm the selection by pressing Enter
        let actions = vec![CursorAction::PressEnter, CursorAction::Sleep3s];
        execute_cursor_actions(session, cap, &actions, "confirm_slave_mode").await?;

        // Capture milestone: Mode switched to Slave
        log::info!("📸 Milestone: Mode switched to Slave");
        let screen = cap.capture(session, "milestone_mode_slave").await?;
        log::info!("Terminal snapshot:\n{screen}");

        // CRITICAL: Verify the mode was actually switched to Slave
        // This verification checks the terminal display to ensure "Slave" is visible
        // on the Connection Mode line specifically
        log::info!("Verifying Connection Mode was switched to Slave...");
        let pattern = Regex::new(r"Connection Mode.*Slave")?;
        let actions = vec![CursorAction::MatchPattern {
            pattern,
            description: "Connection Mode line should show 'Slave'".to_string(),
            line_range: None,
            col_range: None,
            retry_action: None,
        }];
        execute_cursor_actions(session, cap, &actions, "verify_slave_mode").await?;
        log::info!("✅ Connection Mode verified as Slave (UI display)");

        // ADDITIONAL: Wait longer for internal state to update
        // The UI might show "Slave" before the internal state is fully committed
        sleep_3s().await;

        // Reset to top after mode change to ensure known cursor position
        let actions = vec![CursorAction::PressCtrlPageUp, CursorAction::Sleep1s];
        execute_cursor_actions(session, cap, &actions, "reset_to_top_after_slave").await?;
    } else {
        // For Master mode, also reset to top for consistency
        let actions = vec![CursorAction::PressCtrlPageUp, CursorAction::Sleep1s];
        execute_cursor_actions(session, cap, &actions, "reset_to_top_master").await?;
    }

    // Phase 2: Create station AFTER mode is configured
    log::info!("Creating station...");

    // First check if station already exists (may happen in retry scenarios)
    let screen = cap.capture(session, "check_existing_station").await?;
    let station_exists = screen.contains("#1");

    if !station_exists {
        execute_transaction_with_retry(
            session,
            cap,
            "create_station",
            &[
                CursorAction::PressEnter,
                CursorAction::Sleep3s,
                // DO NOT press Ctrl+PgUp here!
                // The TUI automatically moves cursor to the new station's StationId field
                // after creation, which is exactly where we want to be for Phase 3
            ],
            |screen| {
                // Verify station #1 was created AND cursor is at Station ID field
                // Look for both "#1" (station created) and "Station ID" field
                if screen.contains("#1") && screen.contains("Station ID") {
                    Ok(true)
                } else {
                    log::debug!("Create station verification: looking for '#1' and 'Station ID'");
                    Ok(false)
                }
            },
            Some(&[
                // Custom rollback: reset to top (AddLine) to retry creation
                CursorAction::PressCtrlPageUp,
                CursorAction::Sleep1s,
            ]),
            &[CursorAction::PressCtrlPageUp, CursorAction::Sleep1s],
        )
        .await?;
    } else {
        log::info!("Station #1 already exists, navigating to it...");
        // Navigate to existing station's StationId field
        execute_cursor_actions(
            session,
            cap,
            &[
                CursorAction::PressCtrlPageUp,
                CursorAction::Sleep1s,
                CursorAction::PressPageDown,
                CursorAction::Sleep1s,
                CursorAction::PressArrow {
                    direction: ArrowKey::Down,
                    count: 1,
                },
                CursorAction::Sleep1s,
            ],
            "navigate_to_existing_station",
        )
        .await?;
    }

    // Phase 3: Verify we're at Station ID field
    // After station creation, TUI automatically positions cursor at StationId field
    // No additional navigation needed!
    log::info!("Verifying cursor is at Station ID field...");

    let screen = cap.capture(session, "verify_at_station_id").await?;
    if !screen.contains("Station ID") {
        return Err(anyhow!(
            "Expected to be at Station ID field after station creation, but field not found"
        ));
    }

    log::info!("📸 Milestone: At Station ID field");

    // Phase 4: Configure Station ID (field 0) with enhanced transaction retry
    // Cursor should now be on Station ID field
    log::info!("Configuring Station ID: {}", config.station_id);

    let station_id_actions = vec![
        CursorAction::PressEnter, // Enter edit mode
        CursorAction::Sleep1s,    // Increased wait for edit mode
        CursorAction::PressCtrlA, // Select all
        CursorAction::Sleep1s,
        CursorAction::PressBackspace, // Clear
        CursorAction::Sleep1s,
        CursorAction::TypeString(config.station_id.to_string()), // Type new value
        CursorAction::Sleep1s,                                   // Wait for typing to complete
        CursorAction::PressEnter,                                // Confirm
        CursorAction::Sleep3s, // Wait longer for commit and UI update
    ];

    let reset_to_station_id = vec![
        CursorAction::PressCtrlPageUp,
        CursorAction::Sleep1s,
        CursorAction::PressPageDown,
        CursorAction::Sleep1s,
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 1,
        },
        CursorAction::Sleep1s,
    ];

    // Edit Station ID field WITHOUT moving to next field
    // We'll verify the value directly instead of checking next field position
    execute_field_edit_with_retry(
        session,
        cap,
        "station_id",
        &station_id_actions,
        true, // Check not in edit mode
        None, // Don't verify next field, just verify we're not in edit mode
        &reset_to_station_id,
    )
    .await?;

    // Now explicitly move to Register Type field
    log::info!("Moving to Register Type field...");
    execute_cursor_actions(
        session,
        cap,
        &[
            CursorAction::PressArrow {
                direction: ArrowKey::Down,
                count: 1,
            },
            CursorAction::Sleep1s,
        ],
        "move_to_register_type",
    )
    .await?;

    log::info!("📸 Milestone: Station ID configured");

    // Phase 5: Configure Register Type (field 1) with transaction retry
    log::info!("Configuring Register Type: {:?}", config.register_mode);
    let (direction, count) = config.register_mode.arrow_from_default();

    if count > 0 {
        // Need to change from default (Holding) to another type
        execute_transaction_with_retry(
            session,
            cap,
            "configure_register_type",
            &[
                CursorAction::PressEnter,
                CursorAction::Sleep1s,
                CursorAction::PressArrow { direction, count },
                CursorAction::Sleep1s,
                CursorAction::PressEnter,
                CursorAction::Sleep3s,
                CursorAction::PressArrow {
                    direction: ArrowKey::Down,
                    count: 1,
                },
                CursorAction::Sleep1s,
            ],
            |screen| {
                // Verify we moved to next field (Start Address)
                let has_address_field = screen.contains("Address") || screen.contains("0x");
                Ok(has_address_field)
            },
            Some(&[CursorAction::PressEscape, CursorAction::Sleep1s]),
            &[
                CursorAction::PressCtrlPageUp,
                CursorAction::Sleep1s,
                CursorAction::PressPageDown,
                CursorAction::Sleep1s,
                CursorAction::PressArrow {
                    direction: ArrowKey::Down,
                    count: 2,
                },
                CursorAction::Sleep1s,
            ],
        )
        .await?;
    } else {
        // Using default (Holding), just move to next field with retry
        execute_transaction_with_retry(
            session,
            cap,
            "skip_default_register_type",
            &[
                CursorAction::PressArrow {
                    direction: ArrowKey::Down,
                    count: 1,
                },
                CursorAction::Sleep1s,
            ],
            |screen| {
                let has_address_field = screen.contains("Address") || screen.contains("0x");
                Ok(has_address_field)
            },
            None,
            &[
                CursorAction::PressCtrlPageUp,
                CursorAction::Sleep1s,
                CursorAction::PressPageDown,
                CursorAction::Sleep1s,
                CursorAction::PressArrow {
                    direction: ArrowKey::Down,
                    count: 1,
                },
                CursorAction::Sleep1s,
            ],
        )
        .await?;
    }

    log::info!("📸 Milestone: Register Type configured");

    // Phase 6: Configure Start Address (field 2) with transaction retry
    log::info!("Configuring Start Address: 0x{:04X}", config.start_address);

    let reset_to_start_address = vec![
        CursorAction::PressCtrlPageUp,
        CursorAction::Sleep1s,
        CursorAction::PressPageDown,
        CursorAction::Sleep1s,
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 3,
        },
        CursorAction::Sleep1s,
    ];

    // Always re-center the cursor onto Start Address before attempting edits. This keeps
    // subsequent operations deterministic even if prior navigation left us on Register
    // Type or other fields.
    execute_cursor_actions(
        session,
        cap,
        &reset_to_start_address,
        "focus_start_address_initial",
    )
    .await?;
    ensure_cursor_on_start_address(session, cap).await?;

    if config.start_address == 0 {
        log::info!("Start Address already 0 - skipping edit");
        let screen = cap.capture(session, "verify_start_address_default").await?;
        if !screen.contains("> Start Address") {
            return Err(anyhow!(
                "Cursor not positioned at Start Address when attempting to skip edit"
            ));
        }
        execute_cursor_actions(
            session,
            cap,
            &[
                CursorAction::PressArrow {
                    direction: ArrowKey::Down,
                    count: 1,
                },
                CursorAction::Sleep1s,
            ],
            "move_to_register_length_after_start_skip",
        )
        .await?;
    } else {
        // Start Address should be entered in decimal format. Clear existing digits via
        // limited backspace attempts to avoid control-key sequences misbehaving in CI.
        let mut start_address_actions = vec![
            CursorAction::PressEnter,
            CursorAction::Sleep1s,
            CursorAction::PressCtrlA,
            CursorAction::Sleep1s,
        ];
        for _ in 0..MAX_NUMERIC_FIELD_DIGITS {
            start_address_actions.push(CursorAction::PressBackspace);
        }
        start_address_actions.extend_from_slice(&[
            CursorAction::Sleep1s,
            CursorAction::TypeString(config.start_address.to_string()),
            CursorAction::Sleep1s,
            CursorAction::PressEnter,
            CursorAction::Sleep3s,
            CursorAction::PressArrow {
                direction: ArrowKey::Down,
                count: 1,
            },
            CursorAction::Sleep1s,
        ]);

        let expected_start_pattern = format!(
            "Start Address         0x{:04X} ({})",
            config.start_address, config.start_address
        );

        execute_field_edit_with_retry(
            session,
            cap,
            "start_address",
            &start_address_actions,
            true,
            Some(expected_start_pattern.as_str()),
            &reset_to_start_address,
        )
        .await?;
    }

    log::info!("📸 Milestone: Start Address configured");

    // Phase 7: Configure Register Count (field 3) with transaction retry
    log::info!("Configuring Register Count: {}", config.register_count);

    let mut register_count_actions = vec![
        CursorAction::PressEnter,
        CursorAction::Sleep3s,
        CursorAction::PressCtrlA,
        CursorAction::Sleep1s,
    ];
    for _ in 0..MAX_NUMERIC_FIELD_DIGITS {
        register_count_actions.push(CursorAction::PressBackspace);
    }
    register_count_actions.extend_from_slice(&[
        CursorAction::Sleep1s,
        CursorAction::TypeString(config.register_count.to_string()),
        CursorAction::Sleep1s,
        CursorAction::PressEnter,
        CursorAction::Sleep3s, // Wait for register grid to render (reduced from 3000ms)
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 1,
        },
        CursorAction::Sleep1s,
    ]);

    let reset_to_register_count = vec![
        CursorAction::PressCtrlPageUp,
        CursorAction::Sleep1s,
        CursorAction::PressPageDown,
        CursorAction::Sleep1s,
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 4,
        },
        CursorAction::Sleep1s,
    ];

    // After setting Register Count we expect the display to reflect the new value
    // and the cursor to move into the register grid so further edits can proceed.

    let expected_register_count_pattern = format!(
        "Register Length       0x{:04X} ({})",
        config.register_count, config.register_count
    );

    execute_field_edit_with_retry(
        session,
        cap,
        "register_count",
        &register_count_actions,
        true, // Must NOT be in edit mode
        Some(expected_register_count_pattern.as_str()),
        &reset_to_register_count,
    )
    .await?;

    log::info!("📸 Milestone: Register Count configured");

    // Phase 8: Configure register values if provided (Slave stations only)
    if let Some(values) = &config.register_values {
        if !config.is_master {
            log::info!("Phase 8: Configuring {} register values...", values.len());

            // IMPORTANT: After Register Count edit + Enter, cursor is at Register Length field
            // We need to navigate to the register list below it
            // The UI shows register address lines, each with multiple register value fields
            log::info!("Navigating to first register value field...");

            // Navigation steps:
            // 1. Down: Move from "Register Length" to first register address line
            //    After Down, cursor should already be at the first register value field
            let actions = vec![
                CursorAction::PressArrow {
                    direction: ArrowKey::Down,
                    count: 1,
                },
                CursorAction::Sleep1s,
            ];
            execute_cursor_actions(session, cap, &actions, "nav_to_first_register_value").await?;

            // Now configure each register value
            // TUI displays registers in rows with 4 values per row
            // Navigation: Right arrow moves to next register on same row
            // After last register on a row, we're at the end - need Down to go to next row's first register
            for (reg_idx, &value) in values.iter().enumerate() {
                log::info!(
                    "  Setting register {} to 0x{:04X} ({})",
                    reg_idx,
                    value,
                    value
                );

                // Edit the current register value
                let actions = vec![
                    CursorAction::PressEnter, // Enter edit mode
                    CursorAction::Sleep1s,
                    CursorAction::PressCtrlA, // Select all
                    CursorAction::Sleep1s,
                    CursorAction::PressBackspace, // Clear
                    CursorAction::Sleep1s,
                    // NOTE: TUI register fields accept hexadecimal input
                    // Format as "0xXXXX" for proper hex interpretation
                    CursorAction::TypeString(format!("0x{:04X}", value)),
                    CursorAction::Sleep1s,
                    CursorAction::PressEnter, // Confirm
                    CursorAction::Sleep1s,    // Wait for commit
                ];
                execute_cursor_actions(
                    session,
                    cap,
                    &actions,
                    &format!("set_register_{}", reg_idx),
                )
                .await?;

                // After Enter, cursor stays at the same register field
                // We need to manually navigate to next register
                if reg_idx < values.len() - 1 {
                    // Not the last register - move to next
                    // Check if we need to move to next row (every 4 registers)
                    if (reg_idx + 1) % 4 == 0 {
                        // Moving to next row: Down then move back to first column
                        // Actually, Down + Left*3 to get to first register of next row
                        log::info!("    Moving to next register row...");
                        let actions = vec![
                            CursorAction::PressArrow {
                                direction: ArrowKey::Down,
                                count: 1,
                            },
                            CursorAction::Sleep1s,
                            // After Down, cursor should be at address field of next row
                            // Press Right once to get to first register value
                            CursorAction::PressArrow {
                                direction: ArrowKey::Right,
                                count: 1,
                            },
                            CursorAction::Sleep1s,
                        ];
                        execute_cursor_actions(
                            session,
                            cap,
                            &actions,
                            &format!("nav_to_next_row_{}", reg_idx + 1),
                        )
                        .await?;
                    } else {
                        // Same row - just move Right to next register
                        log::info!("    Moving to next register on same row...");
                        let actions = vec![
                            CursorAction::PressArrow {
                                direction: ArrowKey::Right,
                                count: 1,
                            },
                            CursorAction::Sleep1s,
                        ];
                        execute_cursor_actions(
                            session,
                            cap,
                            &actions,
                            &format!("nav_to_next_register_{}", reg_idx + 1),
                        )
                        .await?;
                    }
                }
            }

            log::info!("✅ All {} register values configured", values.len());

            log::info!("📸 Milestone: Register values configured");
        } else {
            log::info!(
                "Phase 8: Skipping register value configuration (Master stations don't have initial values)"
            );
        }
    } else {
        log::info!("Phase 8: No register values provided - using defaults");
    }

    // Phase 9: Save configuration with Ctrl+S (port will auto-enable)
    log::info!("Saving configuration with Ctrl+S...");

    // Save configuration - don't verify screen as old error messages may persist
    // Instead, verify via status file in next phase
    let save_actions = vec![
        CursorAction::PressCtrlPageUp,
        CursorAction::Sleep1s,
        CursorAction::PressCtrlS,
        CursorAction::Sleep3s, // Wait longer for save and port enable
    ];

    execute_cursor_actions(session, cap, &save_actions, "save_configuration").await?;

    log::info!("📸 Milestone: Configuration saved");

    // Phase 10: Verify configuration via status file
    // According to CLAUDE.md, Ctrl+S saves config and auto-enables port
    log::info!("Verifying configuration was saved to status file...");

    // Wait for status file to be updated
    sleep_3s().await;

    // Check if configuration exists in status file
    let status = read_tui_status().map_err(|e| {
        anyhow!(
            "Failed to read TUI status file after Ctrl+S: {}. \
             This indicates the configuration may not have been saved.",
            e
        )
    })?;

    // Verify we have the port
    if status.ports.is_empty() {
        return Err(anyhow!(
            "No ports found in TUI status after Ctrl+S. \
             Configuration save may have failed."
        ));
    }

    let port_status = &status.ports[0];
    log::info!(
        "Port status: enabled={}, masters={}, slaves={}",
        port_status.enabled,
        port_status.modbus_masters.len(),
        port_status.modbus_slaves.len()
    );

    // Verify station configuration exists
    if config.is_master {
        if port_status.modbus_masters.is_empty() {
            return Err(anyhow!(
                "No master configuration found in status file after Ctrl+S. \
                 Configuration save failed."
            ));
        }
        log::info!("✅ Master configuration found in status file");
    } else {
        if port_status.modbus_slaves.is_empty() {
            return Err(anyhow!(
                "No slave configuration found in status file after Ctrl+S. \
                 Configuration save failed."
            ));
        }
        log::info!("✅ Slave configuration found in status file");
    }

    // Phase 11: Wait for CLI subprocess to start (for Master mode)
    // The TUI spawns a CLI subprocess which creates its own status file
    if config.is_master {
        log::info!("Waiting for CLI Master subprocess to start...");
        sleep_3s().await;

        // Check if CLI status file exists
        let cli_status_path = format!(
            "/tmp/ci_cli_{}_status.json",
            _port1.trim_start_matches("/tmp/")
        );
        log::info!("Checking for CLI status file: {cli_status_path}");

        // Wait up to 10 seconds for CLI status file to appear
        let mut found = false;
        for i in 1..=20 {
            if std::path::Path::new(&cli_status_path).exists() {
                log::info!(
                    "✅ CLI subprocess status file found after {}s",
                    i as f32 * 0.5
                );
                found = true;
                break;
            }
            sleep_1s().await;
        }

        if !found {
            log::warn!("⚠️  CLI subprocess status file not found, but continuing...");
            log::warn!("    This may be normal if subprocess hasn't written status yet");
        }
    }

    log::info!("📸 Milestone: Port enabled and running");

    log::info!("✅ Station configuration completed - saved and enabled");
    Ok(())
}

async fn ensure_cursor_on_start_address<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
) -> Result<()> {
    const MAX_FOCUS_ATTEMPTS: usize = 5;

    for attempt in 0..MAX_FOCUS_ATTEMPTS {
        let capture_name = format!("focus_start_address_attempt_{}", attempt + 1);
        let screen = cap.capture(session, &capture_name).await?;

        if screen.contains("> Start Address") {
            return Ok(());
        }

        let (actions, label): (Vec<CursorAction>, &str) = if screen.contains("> Register Type") {
            (
                vec![
                    CursorAction::PressArrow {
                        direction: ArrowKey::Down,
                        count: 1,
                    },
                    CursorAction::Sleep1s,
                ],
                "focus_start_address_from_register_type",
            )
        } else if screen.contains("> Register Length") {
            (
                vec![
                    CursorAction::PressArrow {
                        direction: ArrowKey::Up,
                        count: 1,
                    },
                    CursorAction::Sleep1s,
                ],
                "focus_start_address_from_register_length",
            )
        } else {
            (
                vec![
                    CursorAction::PressArrow {
                        direction: ArrowKey::Down,
                        count: 1,
                    },
                    CursorAction::Sleep1s,
                ],
                "focus_start_address_general_adjust",
            )
        };

        execute_cursor_actions(session, cap, &actions, label).await?;
    }

    Err(anyhow!(
        "Failed to focus Start Address field after {MAX_FOCUS_ATTEMPTS} attempts"
    ))
}
