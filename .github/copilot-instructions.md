# Copilot Instructions for TUI E2E Testing

## TUI E2E Testing with Status Monitoring

### Overview

The TUI E2E testing framework has been refactored to support two complementary testing approaches:

1. **TUI UI E2E**: Pure UI element testing using simulated terminal
   - Uses terminal screen capture and pattern matching
   - Validates UI rendering, layout, and visual elements
   - Example: Checking if configuration fields show editing brackets `[value]`

2. **TUI Logic E2E**: Logic testing using status tree monitoring
   - Reads global status from JSON dumps
   - Validates application state and behavior
   - Example: Checking if port is enabled, modbus stations are configured

### Debug Mode Activation

#### For TUI Processes

Set the environment variable before starting TUI:
```bash
export AOBA_DEBUG_CI_E2E_TEST=1
cargo run --package aoba -- --tui
```

This will create `/tmp/ci_tui_status.json` with periodic status dumps (every 500ms).

#### For CLI Subprocesses

CLI subprocesses automatically inherit debug mode when spawned by a TUI process in debug mode. The `--debug-ci-e2e-test` flag is injected automatically.

Manual CLI invocation:
```bash
cargo run --package aoba -- --slave-listen-persist /tmp/vcom1 --debug-ci-e2e-test
```

This will create `/tmp/ci_cli_vcom1_status.json` with periodic status dumps (uses port basename, e.g., "/tmp/vcom1" -> "vcom1").

### Note: Running commands on Windows (non-CI)

If you run these commands on a local Windows machine (not in CI) and you use WSL (Windows Subsystem for Linux), we recommend wrapping commands that must run in a Unix-like environment with `wsl bash -lc '...'` so environment variables, paths and temporary file locations (for example `/tmp`) are resolved correctly inside WSL.

For example:

```bash
# Start TUI in debug mode inside WSL
wsl bash -lc 'export AOBA_DEBUG_CI_E2E_TEST=1; cargo run --package aoba -- --tui'

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

#[tokio::test]
async fn test_tui_master_configuration() -> Result<()> {
    // Enable debug mode
    std::env::set_var("AOBA_DEBUG_CI_E2E_TEST", "1");

    // Spawn TUI
    let mut tui_session = spawn_expect_process(&["--tui"])?;

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

### Troubleshooting

#### Status file not found

Ensure debug mode is enabled:
```rust
std::env::set_var("AOBA_DEBUG_CI_E2E_TEST", "1");
```

#### Status file not updating

Check that the status dump thread is running. Look for log messages:
```
Started status dump thread, writing to /tmp/tui_e2e_status.json
```

#### Timeout waiting for status

- Increase timeout value
- Increase retry interval if file I/O is slow
- Check if the expected state is actually reachable
- Inspect `/tmp/tui_e2e_status.json` manually to see current state

### Examples

See `examples/status_monitoring_example.rs` for a complete working example.

For real-world usage, refer to the updated TUI E2E tests in `examples/tui_e2e/`.
