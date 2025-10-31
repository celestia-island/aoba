# Copilot Instructions for Layered TUI/CLI Testing

## Post-Migration Package Layout

- `packages/tui`: Owns the terminal UI, global state management, and TUI-driven IPC front-ends
- `packages/cli`: Hosts CLI binaries, command dispatch, and long-running Modbus workers
- `packages/protocol`: Shared IPC definitions, status schemas, and Modbus transport primitives
- `packages/ci_utils`: Shared test harness utilities used by every layered test suite

All four crates remain in the top-level workspace. Shared dependencies must be declared in the root `Cargo.toml`; individual package manifests only add crate-specific extras.

## Test Suite Segmentation

- `TUI UI E2E` (deprecated, being merged): Simulates user input and consumes a mocked TUI global state layer. Validates key-to-state transitions without touching IPC. Being merged into `TUI E2E`.
- `TUI E2E`: Exercises TUI global state against mocked TUI→CLI IPC endpoints, asserting command emission and inbound register diffs. Implemented in `examples/tui_e2e`.
- `CLI E2E` (new): Covers bidirectional IPC between the virtualized TUI transport and the live CLI runtime, plus legacy stdio scenarios moved from the original CLI suite. Implemented in `examples/cli_e2e`.
- `CLI Tests` (renamed from legacy CLI E2E): Pure CLI logic checks that do not rely on IPC or persistent stdio loops.

Current CI workflows mirror this split: `e2e-tests-cli.yml` executes the CLI matrices, `e2e-tests-tui.yml` drives the TUI E2E test suites. Each workflow builds its associated example crate alongside the primary package before executing modules.

The remaining sections in this document focus on the `TUI E2E` testing layer, where status monitoring continues to be the primary verification strategy. Refer to each suite's README for layer-specific conventions as they are created.

## Workspace Dependency Policy

- Declare shared versions once in the root `Cargo.toml`
- Re-export workspace members through `[workspace.dependencies]`
- Only add `[dependencies]` entries inside a package when the crate is the sole consumer

## TUI E2E Testing with Status Monitoring

### Overview

The TUI E2E testing framework has been refactored to support two complementary testing approaches:

1. **TUI UI E2E** (deprecated, being merged): Pure UI element testing using simulated terminal
   - Uses terminal screen capture and pattern matching
   - Validates UI rendering, layout, and visual elements
   - Example: Checking if configuration fields show editing brackets `[value]`
   - Being merged into TUI E2E with screenshot verification

2. **TUI E2E**: Testing using status tree monitoring and screenshot verification
   - Reads global status from JSON dumps
   - Validates application state and behavior
   - Supports screenshot-based UI verification
   - Example: Checking if port is enabled, modbus stations are configured

### Debug Mode Activation

#### For TUI Processes

Start TUI with the `--debug-ci-e2e-test` flag:

```bash
cargo run --package aoba -- --tui --debug-ci-e2e-test
```

This will create `/tmp/ci_tui_status.json` with periodic status dumps (every 500ms).

**For E2E tests**, also add `--no-config-cache` to prevent configuration persistence:

```bash
cargo run --package aoba -- --tui --debug-ci-e2e-test --no-config-cache
```

The `--no-config-cache` flag disables loading and saving of `aoba_tui_config.json`,
ensuring each test starts with a clean state without interference from previous runs.
This is **automatically used** by the TUI E2E test framework in `setup_tui_test()`.

#### For CLI Subprocesses

CLI subprocesses automatically inherit debug mode when spawned by a TUI process in debug mode. The `--debug-ci-e2e-test` flag is injected automatically.

Manual CLI invocation:

```bash
cargo run --package aoba -- --slave-listen-persist /tmp/vcom1 --debug-ci-e2e-test
```

This will create `/tmp/ci_cli_vcom1_status.json` with periodic status dumps (uses port basename, e.g., "/tmp/vcom1" -> "vcom1").

### Note: Running commands on Windows (non-CI)

If you run these commands on a local Windows machine (not in CI) and you use WSL (Windows Subsystem for Linux), we recommend wrapping commands that must run in a Unix-like environment with `wsl bash -lc '...'` so paths and temporary file locations (for example `/tmp`) are resolved correctly inside WSL.

For example:

```bash
# Start TUI in debug mode inside WSL
wsl bash -lc 'cargo run --package aoba -- --tui --debug-ci-e2e-test'

# Manually start CLI subprocess (debug mode) inside WSL
wsl bash -lc 'cargo run --package aoba -- --slave-listen-persist /tmp/vcom1 --debug-ci-e2e-test'
```

If you run the above commands in native Windows shells (PowerShell / cmd) you may encounter path or permission issues because debug status files are written to Unix-style temporary directories (e.g., `/tmp`). Using `wsl bash -lc '...'` runs the command explicitly in WSL and avoids these problems.

### Status File Format

#### TUI Status (`/tmp/ci_tui_status.json`)

```json
{
  "ports": [
    {
      "name": "/tmp/vcom1",
      "enabled": true,
      "state": "OccupiedByThis",
      "modbus_masters": [
        {
          "station_id": 1,
          "register_type": "Holding",
          "start_address": 0,
          "register_count": 10
        }
      ],
      "modbus_slaves": [],
      "log_count": 5
    }
  ],
  "page": "ModbusDashboard",
  "timestamp": "2025-10-19T16:41:40.123+00:00"
}
```

#### CLI Status (`/tmp/ci_cli_{port}_status.json`)

```json
{
  "port_name": "/tmp/vcom1",
  "station_id": 1,
  "register_mode": "Holding",
  "register_address": 0,
  "register_length": 10,
  "mode": "SlaveListen",
  "timestamp": "2025-10-19T16:41:40.456+00:00"
}
```

### Testing with Status Monitoring

#### Example Test Structure

```rust
use ci_utils::{
    spawn_expect_process,
    wait_for_tui_page,
    wait_for_port_enabled,
    wait_for_modbus_config,
    read_tui_status,
};

async fn test_tui_master_configuration() -> Result<()> {
    // Spawn TUI with debug mode enabled
    let mut tui_session = spawn_expect_process(&["--tui", "--debug-ci-e2e-test"])?;

    // Wait for TUI to initialize and start writing status
    // Note: In production tests, prefer using wait_for_tui_page() instead of sleep
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Wait for TUI to reach Entry page
    wait_for_tui_page("Entry", 10, None).await?;

    // Perform UI actions (navigate, configure, etc.)
    // ... cursor actions to configure port ...

    // Wait for port to be enabled
    wait_for_port_enabled("/tmp/vcom1", 10, None).await?;

    // Wait for modbus master configuration
    wait_for_modbus_config("/tmp/vcom1", true, 1, 10, None).await?;

    // Read current status for detailed verification
    let status = read_tui_status()?;
    assert_eq!(status.page, "ModbusDashboard");

    Ok(())
}
```

#### Available Monitoring Functions

##### Wait Functions (with timeout and retry)

- `wait_for_tui_page(page, timeout_secs, retry_interval_ms)` - Wait for TUI to reach a specific page
- `wait_for_port_enabled(port_name, timeout_secs, retry_interval_ms)` - Wait for port to be enabled
- `wait_for_modbus_config(port_name, is_master, station_id, timeout_secs, retry_interval_ms)` - Wait for modbus configuration
- `wait_for_cli_status(port_name, timeout_secs, retry_interval_ms)` - Wait for CLI subprocess status

##### Direct Read Functions

- `read_tui_status()` - Read current TUI status from `/tmp/tui_e2e_status.json`
- `read_cli_status(port)` - Read current CLI status from `/tmp/cli_e2e_{port}.log`
- `port_exists_in_tui(port_name)` - Check if port exists in TUI
- `get_port_log_count(port_name)` - Get number of logs for a port

### TUI Port Enable Mechanism (CRITICAL)

**IMPORTANT**: Understanding how ports are enabled/disabled in TUI is critical for writing correct E2E tests.

#### How Port Enable Works

**Port is automatically enabled when you save Modbus configuration with `Ctrl+S`:**

```rust
// Configure stations (Station ID, Register Type, Address, Length)
// ... create station 1, station 2, etc. ...

// Save configuration - THIS ENABLES THE PORT AUTOMATICALLY
let actions = vec![
    CursorAction::PressCtrlS,
    CursorAction::Sleep { ms: 5000 }, // Wait for port to enable and stabilize
];
```

**Key Points:**

1. **Ctrl+S triggers port enable**: When you press `Ctrl+S` in Modbus Panel, TUI saves the configuration AND automatically enables the port
2. **Port state changes from `Disabled` → `Running`**: After Ctrl+S, the status indicator in title bar changes to show `Running ●`
3. **No manual toggle needed**: You do NOT need to manually toggle "Enable Port" field or press Right arrow on it
4. **Escape does NOT enable port**: Pressing Escape to leave Modbus Panel does NOT trigger port enable (this was a previous misunderstanding)

#### Common Mistake: Redundant Port Restart

**WRONG - Redundant leave/return/verify after updating registers:**

```rust
// After Ctrl+S, port is already Running ●
update_tui_registers(&mut session, &mut cap, &data, false).await?;

// ❌ WRONG: No need to leave and return to trigger restart
let actions = vec![
    CursorAction::PressEscape,  // ❌ This doesn't restart the port
    CursorAction::Sleep { ms: 3000 },
    CursorAction::PressArrow { direction: ArrowKey::Down, count: 2 },
    CursorAction::PressEnter,  // ❌ Unnecessary return to panel
];
```

**CORRECT - Port already enabled after Ctrl+S:**

```rust
// Save configuration (enables port automatically)
let actions = vec![
    CursorAction::PressCtrlS,
    CursorAction::Sleep { ms: 5000 },
];
execute_cursor_actions(&mut session, &mut cap, &actions, "save_and_enable").await?;

// Verify port is enabled (we're already in Modbus Panel with status indicator visible)
let status = verify_port_enabled(&mut session, &mut cap, "verify_enabled").await?;

// Update register values (port stays Running)
update_tui_registers(&mut session, &mut cap, &data, false).await?;

// ✅ CORRECT: Port is still Running, directly proceed to testing
test_modbus_communication(...).await?;
```

#### When Port Gets Disabled

Port is disabled (status changes to `Disabled` or `Not Started ×`) when:

1. User manually disables it (not typically done in E2E tests)
2. TUI process exits
3. Configuration is discarded with `Ctrl+Esc`

**IMPORTANT**: Pressing Escape (Esc) alone does NOT enable the port. You MUST use `Ctrl+S` to save configuration and trigger port enabling. After `Ctrl+S`, you can then press `Esc` to return to the previous page (ConfigPanel).

#### Verification Best Practice

Always verify port status AFTER Ctrl+S, while still in Modbus Panel:

```rust
// Save configuration
execute_cursor_actions(&mut session, &mut cap, &save_actions, "save_config").await?;

// Verify immediately (status indicator is visible in Modbus Panel title bar)
let status = verify_port_enabled(&mut session, &mut cap, "verify_after_save").await?;
// Status should be "Running ●" or "Applied ✔"
```

### Multi-Station Configuration Workflow

When configuring multiple Modbus stations, follow this two-phase approach:

#### Phase 1: Station Creation

Create all stations first before configuring any:

```ignore
// Press Enter on "Create Station" station_count times, resetting with Ctrl+PgUp between iterations.
// After creation, confirm the final station by matching the literal string formed by the hash symbol followed by station_count via CursorAction::MatchPattern.
```

**Connection Mode Configuration:**

After creating stations, press Down arrow once to move to "Connection Mode" field. The TUI defaults to **Master** mode:

- If Master mode is needed: No action required (already at default)
- If Slave mode is needed: Press `Enter`, `Left`, `Enter` to switch from Master to Slave

**Note**: There may be some ambiguity in the Chinese requirements which suggest pressing `Right` to switch to Master mode, but code inspection and existing tests confirm that the default is Master, and `Left` switches to Slave.

#### Phase 2: Station Configuration

Configure each station individually, using absolute positioning:

```rust
for (i, station_config) in station_configs.iter().enumerate() {
    let station_number = i + 1; // 1-indexed

    // Navigate to station using Ctrl+PgUp + PgDown
    let mut actions = vec![CursorAction::PressCtrlPageUp];
    for _ in 0..=i {
        actions.push(CursorAction::PressPageDown);
    }

    // Configure Station ID
    actions.extend(vec![
        CursorAction::PressArrow { direction: ArrowKey::Down, count: 1 },
        CursorAction::PressEnter,
        CursorAction::PressCtrlA,     // Select all
        CursorAction::PressBackspace, // Clear
        CursorAction::TypeString(station_id.to_string()),
        CursorAction::PressEnter,
        CursorAction::Sleep { ms: 200 },
    ]);

    // Configure Register Type (field 2, using Down count: 2)
    // ... similar pattern for other fields ...
}
```

**Key Points:**

- Always use `Ctrl+PgUp` to reset to top of panel before navigating to a station
- Use `PgDown` to jump to station sections (one PgDown per station from top)
- Use `Down` arrow keys to navigate between fields within a station
- After configuring all stations, use `Ctrl+S` once to save all configurations and enable the port
- Use `Ctrl+PgUp` at the end of each station configuration to return to top (ensures consistent state)

### Register Value Configuration Workflow

After configuring station fields (ID, Type, Address, Count), you can optionally configure individual register values:

#### Detailed Step-by-Step Process

1. **Navigate to First Register**: After setting Register Length and confirming, cursor moves to the register grid area
2. **For Each Register**:
   - If register doesn't need a value: Press `Right` to skip to next register
   - If register needs a value:
     - Press `Enter` to enter edit mode
     - Type hexadecimal value (without 0x prefix)
     - Press `Enter` to confirm
     - **Verify value in status tree**: Use `CheckStatus` action to verify the value was written to global status
     - Press `Right` to move to next register
3. **After All Registers Configured**: Press `Ctrl+PgUp` to return to top

#### Status Verification Path Format

For master stations:

```
ports[0].modbus_masters[station_index].registers[register_index]
```

For slave stations:

```
ports[0].modbus_slaves[station_index].registers[register_index]
```

#### Important Notes

- **Station Index**: 0-based (Station 1 has index 0)
- **Register Index**: 0-based (First register has index 0)
- **Value Format**: Hexadecimal without 0x prefix (e.g., "1234" not "0x1234")
- **Status Verification**: Always verify critical values were committed to status tree before proceeding
- **Clean State**: Remove JSON cache files (`rm -f ~/.config/aoba/*.json`) before running tests to avoid interference from previous test runs

### Best Practices

#### When to Use UI Testing vs Status Monitoring

**Use UI Testing (terminal capture) for:**

- Validating UI rendering and layout
- Checking visual indicators (status symbols, colors)
- Verifying edit mode brackets and formatting
- Testing keyboard navigation and cursor movement

**Use Status Monitoring for:**

- Verifying port states (enabled/disabled)
- Checking modbus configuration (stations, registers)
- Waiting for state transitions
- Validating communication logs
- Testing multi-process scenarios

#### Combining Both Approaches

For comprehensive tests, combine both approaches:

```rust
// 1. Use UI testing to configure
execute_cursor_actions(&mut session, &mut cap, &actions, "configure").await?;

// 2. Use status monitoring to verify
wait_for_port_enabled("/tmp/vcom1", 10, None).await?;

// 3. Use UI testing to verify visual feedback
let screen = cap.capture(&mut session, "after_enable").await?;
assert!(screen.contains("●")); // Green dot indicator
```

#### Timeout and Retry Configuration

Default retry interval is 500ms. Adjust based on expected operation duration:

```rust
// Fast operations (page navigation)
wait_for_tui_page("Entry", 5, Some(200)).await?;

// Slow operations (port initialization)
wait_for_port_enabled("/tmp/vcom1", 30, Some(1000)).await?;
```

### Migration Guide

#### Old Approach (Terminal Capture Only)

```rust
// Old: Wait for terminal content to appear
let screen = cap.capture(&mut session, "after_enable").await?;
assert!(screen.contains("Enable Port: Yes"));
```

#### New Approach (Status Monitoring)

```rust
// New: Wait for status to reflect the change
wait_for_port_enabled("/tmp/vcom1", 10, None).await?;
let status = read_tui_status()?;
assert!(status.ports.iter().any(|p| p.name == "/tmp/vcom1" && p.enabled));
```

#### Benefits of New Approach

1. **Reliability**: Status monitoring is not affected by terminal rendering timing
2. **Precision**: Direct access to application state, not visual representation
3. **Speed**: No need to wait for UI refresh cycles
4. **Debuggability**: JSON dumps can be inspected independently
5. **Simplicity**: Clear assertions on structured data instead of text matching

### Debugging TUI E2E Tests

**Important Principle**: The separation of UI and Logic tests does NOT mean abandoning terminal simulation. The terminal is still essential for debugging.

#### When to Use DebugBreakpoint

While TUI E2E tests primarily use status monitoring (CheckStatus) for validation, terminal capture remains critical for debugging:

1. **During Development**: Insert `DebugBreakpoint` actions to capture and inspect the current terminal state when something goes wrong
2. **Troubleshooting Failures**: If a `CheckStatus` assertion fails, add a breakpoint before it to see what the UI actually shows
3. **Verifying UI State**: Use breakpoints to confirm the TUI is in the expected state before performing actions

**Example Usage:**

```rust
let actions = vec![
    // Navigate to a port
    CursorAction::PressArrow { direction: ArrowKey::Down, count: 1 },
    CursorAction::Sleep { ms: 500 },

    // Debug: Check what the terminal shows
    CursorAction::DebugBreakpoint {
        description: "verify_port_selection".to_string(),
    },

    // Then verify via status monitoring
    CursorAction::CheckStatus {
        description: "Port should be selected".to_string(),
        path: "current_selection".to_string(),
        expected: json!("/tmp/vcom1"),
        timeout_secs: Some(5),
        retry_interval_ms: Some(500),
    },
];
```

**Key Point**: Don't debug "blind" using only status checks. Use `DebugBreakpoint` to visually confirm UI behavior, then add appropriate `CheckStatus` assertions once you understand what's happening.

### Menu Navigation Timing and Race Conditions

#### Understanding the TUI Architecture

The TUI uses a multi-threaded architecture that can cause timing issues in E2E tests:

1. **Input Thread**: Captures keyboard events and updates global status synchronously
2. **Core Thread**: Handles subprocess management, polls UI messages every ~50ms
3. **Rendering Thread**: Draws UI based on global status, polls with 100ms timeout

When a menu action like pressing Enter on "Enter Business Configuration" occurs:

1. Input handler updates status immediately (`Page::ConfigPanel` → `Page::ModbusDashboard`)
2. Sends `Refresh` message to rendering thread via channel
3. Rendering thread processes message on next poll cycle (up to 100ms latency)
4. Terminal is redrawn with new page content

**Critical Issue**: E2E tests using terminal capture may see stale content if they capture before the rendering thread completes the draw cycle.

#### Best Practices for Menu Navigation

**DO**: Use status tree verification for page transitions

```rust
// Navigate to menu item and press Enter
execute_cursor_actions(&mut session, &mut cap, &actions, "press_enter").await?;

// Wait for status tree to reflect the page change
wait_for_tui_page("ModbusDashboard", 5, Some(300)).await?;

// Now safe to verify terminal content
let screen = cap.capture(&mut session, "after_navigation").await?;
assert!(screen.contains("ModBus Master/Slave Set"));
```

**DON'T**: Rely solely on terminal pattern matching immediately after navigation

```rust
// ❌ WRONG: May capture before rendering completes
let actions = vec![
    CursorAction::PressEnter,
    CursorAction::Sleep { ms: 1000 },
    CursorAction::MatchPattern {
        pattern: Regex::new(r"ModBus Master/Slave Set")?,
        // ... may fail intermittently
    },
];
```

#### Implementing Robust Menu Navigation

For reliable menu navigation in E2E tests, use a multi-attempt strategy with status verification:

```rust
pub async fn enter_menu_with_retry<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    menu_item: &str,
    expected_page: &str,
    max_attempts: usize,
) -> Result<()> {
    for attempt in 1..=max_attempts {
        // Navigate and press Enter
        navigate_to_menu_item(session, cap, menu_item).await?;
        
        let actions = vec![
            CursorAction::PressEnter,
            CursorAction::Sleep { ms: 1000 },
        ];
        execute_cursor_actions(session, cap, &actions, "press_enter").await?;
        
        // Wait for status tree to update
        match wait_for_tui_page(expected_page, 3, Some(300)).await {
            Ok(()) => {
                // Verify terminal also updated
                tokio::time::sleep(Duration::from_millis(500)).await;
                let screen = cap.capture(session, "verify").await?;
                if screen.contains(expected_page) {
                    return Ok(());
                }
            }
            Err(_) if attempt < max_attempts => {
                log::warn!("Attempt {} failed, retrying...", attempt);
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
            Err(e) => return Err(e),
        }
    }
    Err(anyhow!("Failed after {} attempts", max_attempts))
}
```

#### Synchronization Points

Always use status tree verification at these synchronization points:

- **Page Navigation**: After pressing Enter on menu items
- **Port Enable/Disable**: After toggling port state
- **Configuration Save**: After pressing Ctrl+S
- **Station Creation**: After creating new Modbus stations

This ensures the TUI's internal state has fully updated before proceeding with subsequent actions.

### Troubleshooting

#### Status file not found

Ensure debug mode is enabled by passing the `--debug-ci-e2e-test` flag when spawning the TUI or CLI process:

```rust
spawn_expect_process(&["--tui", "--debug-ci-e2e-test"])?;
```

#### Status file not updating

Check that the status dump thread is running. Look for log messages:

```
Started status dump thread, writing to /tmp/ci_tui_status.json
```

#### Timeout waiting for status

- Increase timeout value
- Increase retry interval if file I/O is slow
- Check if the expected state is actually reachable
- Inspect `/tmp/tui_e2e_status.json` manually to see current state

#### Intermittent menu navigation failures

If menu navigation (e.g., "Enter Business Configuration") fails intermittently:

- **Root Cause**: Race condition between status update and terminal rendering
- **Solution**: Use multi-attempt retry with status tree verification (see "Menu Navigation Timing" section)
- **Implementation**: The `enter_modbus_panel` function now includes:
  - Up to 10 retry attempts with 1-second delays
  - Status tree polling (when debug mode enabled) or terminal verification fallback
  - Automatic re-navigation on failure
- **Prevention**: Always verify page changes via status tree before checking terminal content
- **Debugging**: Add DebugBreakpoint actions to see actual terminal state during failures

#### Multi-station configuration issues

When configuring multiple Modbus stations:

- **Navigation**: Use `Ctrl+PgUp` to reset to top, then `PgDown` to jump to specific stations
- **Timing**: Allow sufficient delays between field edits (1000-2000ms after Enter to exit edit mode)
- **Verification**: Check each station's configuration before pressing Ctrl+S to save
- **Known Issue**: PgDown navigation may not position cursor correctly on all stations; verify with debug breakpoints

## E2E Test Matrix Structure

### Overview

The E2E test suite is organized into a comprehensive matrix covering CLI and TUI modes with various register types and station configurations. All tests follow the principle of independence - each test is a standalone unit and should not be combined with others.

### Test Organization

#### CLI E2E Tests (`examples/cli_e2e`)

**Single-Station Tests** (`e2e/single_station/register_modes.rs`)

- Test all 4 Modbus register modes with Master/Slave communication via stdio pipes
- Modes: 01 Coils, 02 DiscreteInputs (writable), 03 Holding, 04 Input (writable)
- Address ranges: 0x0000-0x0030 (spaced by 0x0010)
- Bidirectional write testing for modes 02 and 04

**Multi-Station Tests** (`e2e/multi_station/two_stations.rs`)

- Test 2-station configurations with various scenarios
- Mixed register types (Coils + Holding)
- Spaced addresses (0x0000 and 0x00A0)
- Mixed station IDs (1 and 5)

#### TUI E2E Tests (`examples/tui_e2e`)

**Single-Station Master Mode** (`e2e/single_station/master_modes.rs`)

- TUI acts as Modbus Master, CLI acts as Slave
- Tests all 4 register modes
- Includes `configure_tui_station` helper following CLAUDE.md workflow
- Full status monitoring and verification

**Single-Station Slave Mode** (`e2e/single_station/slave_modes.rs`)

- TUI acts as Modbus Slave, CLI acts as Master
- Tests all 4 register modes
- Bidirectional write testing for writable modes

**Multi-Station Master Mode** (`e2e/multi_station/master_modes.rs`)

- TUI Master with 2 stations
- Mixed types, spaced addresses, mixed IDs

**Multi-Station Slave Mode** (`e2e/multi_station/slave_modes.rs`)

- TUI Slave with 2 stations
- Mixed types (WritableCoils + WritableRegisters)
- Spaced addresses, mixed IDs (2 and 6)

### Running Tests

```bash
# CLI single-station tests
cargo run --package cli_e2e -- --module modbus_single_station_coils
cargo run --package cli_e2e -- --module modbus_single_station_discrete_inputs
cargo run --package cli_e2e -- --module modbus_single_station_holding
cargo run --package cli_e2e -- --module modbus_single_station_input

# CLI multi-station tests
cargo run --package cli_e2e -- --module modbus_multi_station_mixed_types
cargo run --package cli_e2e -- --module modbus_multi_station_spaced_addresses
cargo run --package cli_e2e -- --module modbus_multi_station_mixed_ids

# TUI single-station Master tests
cargo run --package tui_e2e -- --module tui_master_coils
cargo run --package tui_e2e -- --module tui_master_discrete_inputs
cargo run --package tui_e2e -- --module tui_master_holding
cargo run --package tui_e2e -- --module tui_master_input

# TUI single-station Slave tests
cargo run --package tui_e2e -- --module tui_slave_coils
cargo run --package tui_e2e -- --module tui_slave_discrete_inputs
cargo run --package tui_e2e -- --module tui_slave_holding
cargo run --package tui_e2e -- --module tui_slave_input

# TUI multi-station Master tests
cargo run --package tui_e2e -- --module tui_multi_master_mixed_types
cargo run --package tui_e2e -- --module tui_multi_master_spaced_addresses
cargo run --package tui_e2e -- --module tui_multi_master_mixed_ids

# TUI multi-station Slave tests
cargo run --package tui_e2e -- --module tui_multi_slave_mixed_types
cargo run --package tui_e2e -- --module tui_multi_slave_spaced_addresses
cargo run --package tui_e2e -- --module tui_multi_slave_mixed_ids
```

### Test Implementation Guidelines

1. **Station Configuration Workflow** (TUI tests)
   - Follow the step-by-step process in CLAUDE.md
   - Use `configure_tui_station` helper for consistency
   - Always verify status tree after configuration steps

2. **Register Value Configuration**
   - Set individual register values using hex format (without 0x prefix)
   - Verify each value with `CheckStatus` action
   - Use `Ctrl+PgUp` to return to top after configuration

3. **Port Enable Mechanism**
   - Port is enabled automatically when saving config with `Ctrl+S`
   - Status changes from `Disabled` → `Running` after save
   - Wait at least 5 seconds after `Ctrl+S` for port stabilization

4. **Data Verification**
   - CLI tests: Use stdio pipes and JSON parsing
   - TUI tests: Combine status monitoring with CLI slave/master verification
   - Always verify bidirectional communication for writable modes

5. **Clean State**
   - Remove TUI config cache before each test: `~/.config/aoba/*.json`
   - Clean up debug status files: `/tmp/ci_tui_status.json`, `/tmp/ci_cli_*_status.json`
   - Run `socat_init.sh` to reset virtual serial ports if needed
