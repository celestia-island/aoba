# TUI E2E Testing with IPC and TestBackend

## Overview

The TUI E2E tests use `ratatui::TestBackend` for rendering and IPC-based communication for screen verification. This provides a clean, deterministic testing approach without needing to parse VT100 escape sequences.

## Architecture

### Key Components

1. **Renderer Module** (`src/renderer.rs`)
   - Renders TUI directly to a `TestBackend`
   - Converts ratatui `Buffer` to string representation
   - Supports IPC-based screen content retrieval in DrillDown mode
   - No process spawning required in ScreenCaptureOnly mode

2. **Executor Module** (`src/executor.rs`)
   - Executes TOML workflow definitions
   - Verifies screen content using `verify` and `at_line` fields from TOML
   - Supports two modes: `--screen-capture-only` and DrillDown (default)

3. **IPC Module** (`src/ipc.rs`)
   - Handles communication between E2E tests and TUI process
   - Sends keyboard events to TUI
   - Receives rendered screen content from TUI

## Testing Modes

### 1. Screen Capture Only Mode (`--screen-capture-only`)

In this mode:
- Mock state is manipulated directly
- No keyboard input simulation
- Tests rendering logic with specific state
- Faster execution, good for UI regression tests

### 2. DrillDown Mode (default)

In this mode:
- Spawns real TUI process with `--debug-ci` flag
- Simulates keyboard input events via IPC
- Tests interactive workflows
- Gets screen content via IPC from TestBackend
- More realistic testing of user interactions

## Usage

### Running Tests

Use the provided script to run all TUI E2E tests:

```bash
# Run all TUI rendering tests (screen-capture mode)
../scripts/run_ci_locally.sh --workflow tui-rendering

# Run all TUI drilldown tests (keyboard simulation mode)
../scripts/run_ci_locally.sh --workflow tui-drilldown

# Run specific module
../scripts/run_ci_locally.sh --workflow tui-rendering --module single_station_master_coils

# Run all tests
../scripts/run_ci_locally.sh --workflow all
```

Or run directly with cargo:

```bash
# Run in screen-capture mode
cargo run --package tui_e2e -- --screen-capture-only --module single_station_master_coils

# Run in drilldown mode (default)
cargo run --package tui_e2e -- --module single_station_master_coils

# List available modules
cargo run --package tui_e2e -- --list
```

## Workflow Format

Test workflows are defined in TOML files under `workflow/**/*.toml`. Each workflow contains:

### Manifest Section

```toml
[manifest]
id = "single_station_master_coils"
description = "Single station master mode - Coils (01)"
station_id = 1
register_type = "Coils"
start_address = 0
register_count = 10
is_master = true
init_order = ["setup", "configure", "verify"]
recycle_order = []
```

### Workflow Steps

Steps are organized into named sequences specified in `init_order` and `recycle_order`:

```toml
# Setup sequence
[[workflow.setup]]
description = "Initialize test environment"
verify = "aoba"

# Keyboard input
[[workflow.configure]]
description = "Navigate to configuration"
key = "enter"

[[workflow.configure]]
description = "Type station ID"
input = "station_id"
value = 1
index = 0

# Screen verification
[[workflow.verify]]
description = "Verify port is enabled"
verify = "Enable Port: Yes"
at_line = 15

# Verification with placeholder
[[workflow.verify]]
description = "Verify station ID configured"
verify_with_placeholder = "Station ID: {0}"

# Mock state operations (ScreenCaptureOnly mode)
[[workflow.setup]]
mock_path = "ports[0].enabled"
mock_set_value = true

# Sleep
[[workflow.configure]]
sleep_ms = 1000
```

### Key Fields

- `description`: Human-readable description of the step
- `key`: Key to press (`"enter"`, `"down"`, `"up"`, `"ctrl-s"`, etc.)
- `times`: Number of times to repeat key press (default: 1)
- `input`: Input type for value generation (`"station_id"`, `"register_address"`, etc.)
- `value`: Explicit value to use (overrides generation)
- `index`: Placeholder index for storing/retrieving values
- `verify`: Expected text to find anywhere on screen
- `verify_with_placeholder`: Expected text with placeholder substitution `{index}`
- `at_line`: Line number to verify text at (0-indexed)
- `mock_path`: Mock state path for ScreenCaptureOnly mode
- `mock_set_value`: Value to set in mock state
- `sleep_ms`: Sleep duration in milliseconds

## How It Works

### Rendering Process (ScreenCaptureOnly Mode)

```rust
// 1. Initialize TUI global status
ensure_status_initialized()?;

// 2. Create TestBackend with specified dimensions
let backend = TestBackend::new(120, 40);
let mut terminal = Terminal::new(backend)?;

// 3. Render TUI to backend
terminal.draw(|frame| {
    aoba::tui::render_ui_for_testing(frame)?;
})?;

// 4. Convert buffer to string
let screen_content = buffer_to_string(terminal.backend().buffer());
```

### IPC Communication Flow (DrillDown Mode)

In DrillDown mode, the E2E test framework communicates with a real TUI process via IPC:

1. **Test spawns TUI**: `cargo run --package aoba -- --tui --debug-ci <channel_id>`
2. **IPC channel established**: Unix sockets in `/tmp/aoba_*_<channel_id>.sock`
3. **Test sends keyboard event**: `E2EToTuiMessage::KeyPress { key: "enter" }`
4. **TUI processes input**: Normal input handler processes the key
5. **Test requests screen**: `E2EToTuiMessage::RequestScreen`
6. **TUI renders to TestBackend**: Calls `render_ui()` on TestBackend
7. **TUI sends back content**: `TuiToE2EMessage::ScreenContent { content, width, height }`
8. **Test verifies content**: Checks for expected text using TOML `verify` fields

### Screen Verification

Screen verification is done using TOML workflow definitions:

```toml
# Verify text anywhere on screen
[[workflow.verify_entry_page]]
description = "Verify entry page title"
verify = "aoba"

# Verify text at specific line
[[workflow.verify_port_name]]
description = "Verify port name at line 5"
verify = "/tmp/vcom1"
at_line = 5
```

The executor will:
1. Render the screen (via TestBackend or IPC)
2. Extract expected text from `verify` or `verify_with_placeholder`
3. If `at_line` is specified, check only that line
4. Otherwise, check if text appears anywhere on screen
5. Report detailed error if verification fails

## Benefits

1. **No Process Spawning**: Faster test execution in ScreenCaptureOnly mode
2. **Pure Rendering Tests**: Tests UI rendering logic independently  
3. **Direct Verification**: Text matching directly from TOML workflows
4. **Deterministic**: No timing issues or race conditions
5. **Keyboard Simulation**: DrillDown mode supports interactive testing via IPC
6. **Real TUI Rendering**: DrillDown mode gets actual rendered content from live TUI process
7. **Clean Output**: No VT100 escape sequences to parse

## Current Status

### Implemented
- ✅ TestBackend-based rendering (no process spawning in ScreenCaptureOnly)
- ✅ IPC-based screen verification using TOML workflows
- ✅ Screen-capture mode for static UI tests
- ✅ DrillDown mode for interactive tests with real TUI process
- ✅ Direct text verification using `verify` and `at_line` fields
- ✅ Keyboard input simulation via IPC
- ✅ Placeholder substitution for dynamic values

## Migration from Old Tests

### Before (with expectrl/vt100/insta)

```rust
// Spawn TUI process
let mut session = spawn_tui_process()?;

// Send keys
session.send("\r")?; // Enter

// Capture and parse terminal
let screen = capture_screen(&mut session)?;
assert!(screen.contains("Expected text"));

// Create snapshot
insta::assert_snapshot!("test_name", screen);
```

### After (with TOML/IPC)

```toml
# In workflow TOML file
[[workflow.press_enter]]
description = "Press Enter"
key = "enter"

[[workflow.verify_result]]
description = "Verify result text"
verify = "Expected text"
at_line = 10
```

No need for:
- `cargo insta review` 
- `.snap` files in version control
- VT100 escape sequence parsing
- Snapshot review workflow

## Troubleshooting

### Screen Verification Failures

If screen verification fails, the error message will show:
- Expected text
- Actual line content (if using `at_line`)
- Full screen content for debugging

Example error:
```
Screen verification failed at line 10:
  Expected text: 'Port: /tmp/vcom1'
  Actual line: 'Port: Not configured'
  Full screen:
[... full screen content ...]
```

### IPC Connection Issues

If DrillDown mode fails to connect:
1. Check that TUI process started successfully
2. Verify Unix sockets exist in `/tmp/aoba_*`
3. Increase connection timeout if needed
4. Check TUI logs for IPC initialization errors

### Debug Mode

Enable debug logging for more details:

```bash
cargo run --package tui_e2e -- --module single_station_master_coils --debug
```

This will show:
- IPC communication details
- Keyboard events sent
- Screen content received
- Verification steps

## Test Development Workflow

1. **Write TOML workflow**: Define steps in `workflow/**/*.toml`
2. **Run test**: `cargo run --package tui_e2e -- --module <module_name>`
3. **Check output**: Review any verification failures
4. **Iterate**: Adjust TOML steps and re-run
5. **Commit**: Commit TOML workflow file (no snapshot files needed)

## References

- [ratatui TestBackend](https://docs.rs/ratatui/latest/ratatui/backend/struct.TestBackend.html)
- [copilot-instructions.md](../../.github/copilot-instructions.md) - TUI E2E testing guidelines
- [IPC_ARCHITECTURE.md](IPC_ARCHITECTURE.md) - IPC communication details
