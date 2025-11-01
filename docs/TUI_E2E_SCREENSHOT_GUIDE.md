# TUI E2E Screenshot Assertion Guide

## Overview

The TUI E2E test framework now includes screenshot assertion capabilities that allow tests to:

1. Generate reference screenshots from predicted TUI states
2. Verify actual TUI output against reference screenshots during test execution
3. Support placeholders for random values (e.g., register values) to avoid brittle tests

## Architecture

### Components

1. **ScreenshotContext** (`packages/ci_utils/src/screenshot.rs`)
   - Manages screenshot generation and verification
   - Handles execution mode (Normal vs GenerateScreenshots)
   - Maintains screenshot counter and directory structure

2. **State Helpers** (`examples/tui_e2e/src/e2e/common/state_helpers.rs`)
   - Predict TUI state at various points in test workflow
   - Create TuiStatus objects representing expected UI state

3. **Screenshot Integration** (`examples/tui_e2e/src/e2e/common/screenshot_integration.rs`)
   - Wrapper functions for common screenshot capture points
   - Combines state prediction with screenshot capture

4. **Placeholder System** (`packages/ci_utils/src/placeholder.rs`)
    - Registers dynamic values that will appear in screenshots
    - Replaces them with placeholders like `{{0x#001}}` or `{{0b#002}}` in reference files
    - Restores actual values during verification (boolean placeholders scan sequential `OFF`/`ON` states)

## How It Works

### Screenshot Generation Mode (`--generate-screenshots`)

1. Test predicts what the TUI state should be at a checkpoint
2. Serializes the predicted state to `/tmp/status.json`
3. Spawns a **separate TUI instance** with `--debug-screen-capture` flag
4. The TUI reads `/tmp/status.json` and renders once without interaction
5. Screenshot is captured and saved as reference (e.g., `000.txt`, `001.txt`)
6. TUI exits immediately after rendering

### Normal Test Mode (default)

1. Test performs actual TUI interactions (keyboard input, navigation)
2. At each checkpoint, captures the actual terminal output
3. Compares captured output against reference screenshot
4. Test fails if there's any mismatch

## Adding Screenshot Assertions to Tests

### Step 1: Thread ScreenshotContext Through Test

```rust
pub async fn test_tui_master_coils(
    port1: &str,
    port2: &str,
    execution_mode: ExecutionMode,  // Passed from main.rs
) -> Result<()> {
    // Create screenshot context
    let screenshot_ctx =
        ScreenshotContext::new(execution_mode, "tui_master_coils".into(), "default".into());

    // Pass to test orchestrator
    run_single_station_master_test(port1, port2, config, &screenshot_ctx).await
}
```

### Step 2: Add Screenshot Points at Key Milestones

Screenshot assertions should be added **after logical action groups**:

```rust
// After setting up TUI and entering ConfigPanel
let (mut session, mut cap) = setup_tui_test(port1, port2, Some(screenshot_ctx)).await?;
// Screenshot captured inside setup_tui_test for Entry and ConfigPanel pages

// After navigating to Modbus panel
navigate_to_modbus_panel(&mut session, &mut cap, port1).await?;
wait_for_tui_page("ModbusDashboard", 10, None).await?;
screenshot_after_modbus_panel(&mut session, &mut cap, port1, Some(screenshot_ctx)).await?;

// After configuring station
configure_tui_station(&mut session, &mut cap, port1, &config).await?;
screenshot_after_station_config(
    &mut session, &mut cap, port1,
    config.station_id(),
    config.register_mode(),
    config.start_address(),
    config.register_count() as usize,
    config.is_master(),
    Some(screenshot_ctx),
).await?;

// After enabling port
wait_for_port_enabled(port1, 20, Some(500)).await?;
screenshot_after_port_enabled(
    &mut session, &mut cap, port1,
    config.station_id(),
    config.register_mode(),
    config.start_address(),
    config.register_count() as usize,
    config.is_master(),
    Some(screenshot_ctx),
).await?;
```

### Step 3: Create State Prediction Functions

For custom screenshot points, create state prediction functions:

```rust
pub fn create_my_custom_state(port_name: &str) -> TuiStatus {
    let mut state = StateBuilder::new()
        .with_page(TuiPage::ModbusDashboard)
        .add_port(create_base_port(port_name))
        .build();
    
    // Apply state modifications
    state = apply_state_change(state, |s| {
        if let Some(port) = s.ports.first_mut() {
            port.enabled = true;
            // ... more modifications
        }
    });
    
    state
}
```

## Usage

### Generate Reference Screenshots

```bash
cargo run --package tui_e2e -- --module tui_master_coils --generate-screenshots
```

This creates screenshot files in:

```
examples/tui_e2e/screenshots/
  └── tui_master_coils/
      └── default/
          ├── 000.txt  # Entry page
          ├── 001.txt  # ConfigPanel
          ├── 002.txt  # ModbusDashboard
          └── ...
```

### Run Tests with Verification

```bash
cargo run --package tui_e2e -- --module tui_master_coils
```

This verifies actual terminal output matches reference screenshots.

## Placeholder System for Random Values

When tests use dynamic data (e.g., register values), use placeholders to keep screenshots deterministic. Boolean data defaults to `OFF` and is numbered by match order, so mixing placeholder kinds is safe:

```rust
use aoba_ci_utils::{
    reset_snapshot_placeholders,
    register_placeholder_values,
    PlaceholderValue,
};

// At start of test
reset_snapshot_placeholders();

// Register values exactly in the order they appear in MatchScreenCapture
register_placeholder_values(&[
    PlaceholderValue::Hex(0x1234),
    PlaceholderValue::Dec(42),
    PlaceholderValue::Boolean(false), // sequentially replaces OFF entries
]);

// Screenshot will replace:
//   "0x1234" → "{{0x#000}}"
//   "42"    → "{{#001}}"
//   "OFF"   → "{{0b#002}}"
```

## Directory Structure

```text
examples/tui_e2e/screenshots/
  ├── tui_master_coils/
  │   └── default/
  │       ├── 000.txt
  │       ├── 001.txt
  │       └── ...
  ├── tui_master_holding/
  │   └── default/
  │       └── ...
  └── ... (one directory per test module)
```

## Best Practices

1. **Add screenshots at stable UI states** - After actions complete and UI settles
2. **Use descriptive state predictors** - Make it clear what each screenshot represents
3. **Register placeholders for random data** - Avoid brittle exact-value comparisons
4. **Keep screenshots focused** - One screenshot per logical UI state, not per keystroke
5. **Review generated screenshots** - Manually verify they match expected UI before committing

## Troubleshooting

### "Reference screenshot not found"

Generate screenshots first:

```bash
cargo run --package tui_e2e -- --module <module_name> --generate-screenshots
```

### "Screenshot verification failed: content does not match"

1. Check if UI changed intentionally - if so, regenerate screenshots
2. Check for timing issues - add delays before screenshot capture
3. Check for random values - ensure they're registered with placeholder system

### Test hangs during screenshot generation

This is expected if the test tries to perform lengthy operations. The screenshot generation mode still runs the full test but generates screenshots at key points.

## Future Improvements

- [ ] Add support for partial screenshot comparison (specific regions only)
- [ ] Support multiple test scenarios within one module (beyond "default")
- [ ] Add automatic placeholder detection for common patterns
- [ ] Integrate screenshot diff tool for easier debugging

## Example: Complete Test with Screenshots

See `examples/tui_e2e/src/e2e/single_station/master_modes.rs` for a complete example of tests with screenshot integration.
