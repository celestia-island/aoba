# Screenshot Integration Implementation Plan

## Overview

Integrate screenshot generation/verification into tui_e2e test infrastructure with incremental state management.

## Current State (Phase 1 Complete)

✅ Infrastructure in place:

- `screenshot.rs` module with `ScreenshotContext` and `ExecutionMode`
- Incremental state modification with `apply_state_change`
- `--generate-screenshots` flag in main.rs

## Phase 2: Modify Action Functions

### 2.1 Navigation Functions

Files to modify: `examples/tui_e2e/src/e2e/common/navigation/*.rs`

#### `navigate_to_modbus_panel` (navigation/modbus.rs)

Current signature:

```rust
pub async fn navigate_to_modbus_panel<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    port1: &str,
) -> Result<()>
```

New signature:

```rust
pub async fn navigate_to_modbus_panel<T: ExpectSession>(
    session: &mut T,
    cap: &mut TerminalCapture,
    port1: &str,
    screenshot_ctx: Option<&mut ScreenshotContext>,
    base_state: TuiStatus,
) -> Result<TuiStatus>
```

State predictions needed:

1. **After entry_to_config_panel**: Page changes to ConfigPanel
2. **After navigate_to_vcom**: Cursor positioned on selected port
3. **After enter_modbus_panel**: Page changes to ModbusDashboard

### 2.2 Station Configuration Functions

Files to modify: `examples/tui_e2e/src/e2e/common/station/*.rs`

#### `create_station` (station/creation.rs)

State prediction:

- Add new station to `modbus_masters` or `modbus_slaves` array
- Update cursor position

#### `configure_station` (station/configure.rs)

State prediction:

- Modify station fields (ID, register_type, address, length)
- Apply incremental changes to existing station

### 2.3 Test Orchestrators

Files to modify: `examples/tui_e2e/src/e2e/common/execution/*.rs`

#### `run_single_station_master_test` (execution/single_station.rs)

Current signature:

```rust
pub async fn run_single_station_master_test(
    port1: &str,
    port2: &str,
    config: StationConfig,
) -> Result<()>
```

New signature:

```rust
pub async fn run_single_station_master_test(
    port1: &str,
    port2: &str,
    config: StationConfig,
    execution_mode: ExecutionMode,
) -> Result<()>
```

Implementation steps:

1. Create `ScreenshotContext` at function start
2. Build initial base state
3. Pass context and state through all action calls
4. Capture/verify screenshot after each action

## Phase 3: Update Test Entry Points

### 3.1 Main.rs Changes

Update test dispatch in main():

```rust
let execution_mode = if args.generate_screenshots {
    ExecutionMode::GenerateScreenshots
} else {
    ExecutionMode::Normal
};

// Pass execution_mode to all test functions
match module {
    "tui_master_coils" => {
        e2e::test_tui_master_coils(&args.port1, &args.port2, execution_mode).await
    }
    // ... etc
}
```

### 3.2 Test Function Signatures

Update all test functions in:

- `e2e/single_station/master_modes.rs`
- `e2e/single_station/slave_modes.rs`
- `e2e/multi_station/master_modes.rs`
- `e2e/multi_station/slave_modes.rs`

Add `execution_mode: ExecutionMode` parameter to each.

## Phase 4: State Prediction Helpers

Create helper functions for common state transformations:

### Port State Helpers

```rust
pub fn create_base_port(name: &str) -> TuiPort {
    TuiPort {
        name: name.to_string(),
        enabled: false,
        state: PortState::Disabled,
        modbus_masters: Vec::new(),
        modbus_slaves: Vec::new(),
        log_count: 0,
    }
}

pub fn enable_port(mut state: TuiStatus) -> TuiStatus {
    apply_state_change(state, |s| {
        if let Some(port) = s.ports.first_mut() {
            port.enabled = true;
            port.state = PortState::Running;
        }
    })
}
```

### Station Helpers

```rust
pub fn add_master_station(
    mut state: TuiStatus,
    station_id: u8,
    register_type: &str,
    start_address: u16,
    register_count: usize,
) -> TuiStatus {
    apply_state_change(state, |s| {
        if let Some(port) = s.ports.first_mut() {
            port.modbus_masters.push(TuiModbusMaster {
                station_id,
                register_type: register_type.to_string(),
                start_address,
                register_count,
            });
        }
    })
}
```

## Phase 5: Screenshot Directory Structure

```
examples/tui_e2e/screenshots/
├── tui_master_coils/
│   └── test_basic_configuration/
│       ├── 001.txt  # Initial entry page
│       ├── 002.txt  # After navigation to ConfigPanel
│       ├── 003.txt  # After selecting port
│       ├── 004.txt  # After entering Modbus panel
│       ├── 005.txt  # After creating station
│       └── ...
├── tui_master_discrete_inputs/
│   └── test_basic_configuration/
│       └── ...
└── ...
```

## Phase 6: Migration Strategy

### Incremental Migration

1. **Start with one test module**: `tui_master_coils`
2. **Implement full screenshot integration** for that module
3. **Generate reference screenshots**
4. **Verify screenshots work in normal mode**
5. **Repeat for other modules**

### Priority Order

1. `tui_master_coils` (simplest, good starting point)
2. `tui_master_holding` (similar to coils)
3. Multi-station tests (more complex state)

## Implementation Checklist

### Phase 2: Action Functions

- [ ] Modify `navigate_to_modbus_panel` with screenshot support
- [ ] Modify `setup_tui_test` with screenshot support
- [ ] Modify `create_station` with screenshot support
- [ ] Modify `configure_station` with screenshot support
- [ ] Add state prediction helper functions

### Phase 3: Test Orchestrators

- [ ] Update `run_single_station_master_test` signature
- [ ] Thread ExecutionMode through orchestrator
- [ ] Create ScreenshotContext in orchestrator
- [ ] Pass context to all action functions

### Phase 4: Test Entry Points

- [ ] Update all test function signatures
- [ ] Update main() dispatch logic
- [ ] Pass ExecutionMode from Args to tests

### Phase 5: Initial Test

- [ ] Generate screenshots for `tui_master_coils`
- [ ] Verify screenshots are non-empty
- [ ] Test verification mode
- [ ] Fix any issues

### Phase 6: Rollout

- [ ] Migrate remaining single-station tests
- [ ] Migrate multi-station tests
- [ ] Generate all reference screenshots
- [x] Delete tui_ui_e2e directory

## Testing Strategy

### Generate Mode

```bash
cargo run --package tui_e2e -- \
    --module tui_master_coils \
    --generate-screenshots
```

### Verify Mode

```bash
cargo run --package tui_e2e -- \
    --module tui_master_coils
```

### Check Screenshots

```bash
ls -lh examples/tui_e2e/screenshots/tui_master_coils/test_basic_configuration/
cat examples/tui_e2e/screenshots/tui_master_coils/test_basic_configuration/001.txt
```

## Notes

- All state predictions must be exact - no approximations
- Screenshot verification is strict - any mismatch fails
- Use incremental state changes to avoid repetition
- Test each phase before moving to next
- Keep backward compatibility during migration
