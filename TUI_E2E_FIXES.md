# TUI E2E Test Fixes

## Summary

Fixed two failing TUI E2E test modules by addressing navigation and field editing issues:
- `modbus_tui_slave_cli_master` - Basic slave configuration test
- `modbus_tui_multi_master_mixed_types` - Multi-station master configuration test

## Issues Identified

### Issue 1: Field Editing Without Clearing (basic_slave.rs)

**Problem**: When editing the "Register Length" field, the test typed "12" without clearing the existing default value of "1", resulting in an invalid value.

**Symptom**: 
- Terminal showed `> 12_ <` indicating the field was in edit mode
- Status tree showed `register_count: 1` instead of the expected `12`
- Test timeout waiting for status update

**Root Cause**: Missing field clear sequence before typing new value.

**Fix**: Added `Ctrl+A` and `Backspace` actions before typing the new value, following the same pattern used in other successful tests.

```rust
// Before:
CursorAction::PressEnter,         // Enter edit mode
CursorAction::TypeString(REGISTER_LENGTH.to_string()),

// After:
CursorAction::PressEnter,         // Enter edit mode
CursorAction::PressCtrlA,         // Select all
CursorAction::PressBackspace,     // Clear field
CursorAction::TypeString(REGISTER_LENGTH.to_string()),
```

### Issue 2: Incorrect PageDown Navigation (master_modes.rs, slave_modes.rs)

**Problem**: The multi-station configuration helper functions used incorrect PageDown counts to navigate to each station's fields.

**Symptom**:
- Station configuration landed on wrong fields
- Fields were configured out of order
- Status checks failed because values weren't committed to the right stations

**Root Cause**: Off-by-one error in PageDown loop counter.

**Navigation Logic**:
```
Ctrl+PgUp        → AddLine
1st PgDown       → ModbusMode
2nd PgDown       → StationId{0}
3rd PgDown       → StationId{1}
...
(i+2)th PgDown   → StationId{i}
```

**Fix**: Changed loop from `0..=i` to `0..(i+2)`:

```rust
// Before:
for _ in 0..=i {  // For i=0: 1 PgDown → ModbusMode ❌
                  // For i=1: 2 PgDown → StationId{0} ❌
    actions.push(CursorAction::PressPageDown);
}

// After:
for _ in 0..(i + 2) {  // For i=0: 2 PgDown → StationId{0} ✅
                       // For i=1: 3 PgDown → StationId{1} ✅
    actions.push(CursorAction::PressPageDown);
}
```

### Issue 3: Redundant Down Navigation (master_modes.rs, slave_modes.rs)

**Problem**: After navigating to a station with PageDown, the code performed an additional `Down 1` movement before configuring StationId, which moved the cursor to the RegisterMode field instead.

**Fix**: Removed the redundant `Down 1` movement:

```rust
// Before:
execute_cursor_actions(session, cap, &actions, "nav_to_station").await?;
let actions = vec![
    CursorAction::PressArrow { direction: ArrowKey::Down, count: 1 },  // ❌ Redundant
    CursorAction::PressEnter,  // Now editing RegisterMode instead of StationId
    ...
];

// After:
execute_cursor_actions(session, cap, &actions, "nav_to_station").await?;
let actions = vec![
    CursorAction::PressEnter,  // ✅ Editing StationId as intended
    ...
];
```

### Issue 4: Repeated Down Navigation for Register Values (master_modes.rs)

**Problem**: When configuring register values, the code navigated `Down 1` for EVERY register, but after the first register, the cursor should move horizontally with `Right`.

**Fix**: Only navigate Down once for the first register:

```rust
// Before:
for (j, &value) in values.iter().enumerate() {
    let actions = vec![
        CursorAction::PressArrow { direction: ArrowKey::Down, count: 1 },  // ❌ Every time
        CursorAction::PressEnter,
        ...
    ];
}

// After:
for (j, &value) in values.iter().enumerate() {
    let mut actions = Vec::new();
    if j == 0 {  // ✅ Only for first register
        actions.push(CursorAction::PressArrow { direction: ArrowKey::Down, count: 1 });
    }
    actions.extend(vec![
        CursorAction::PressEnter,
        ...
    ]);
}
```

## Files Modified

1. **examples/tui_e2e/src/e2e/basic_slave.rs**
   - Added field clearing before editing Register Length
   - Removed debug breakpoints
   - Optimized sleep timings

2. **examples/tui_e2e/src/e2e/multi_station/master_modes.rs**
   - Fixed PageDown navigation count: `0..=i` → `0..(i+2)`
   - Removed redundant Down 1 after PageDown
   - Fixed register value configuration loop
   - Added clarifying comments

3. **examples/tui_e2e/src/e2e/multi_station/slave_modes.rs**
   - Applied same PageDown navigation fix
   - Removed redundant Down 1 after PageDown
   - Removed unnecessary "skip_mode" step
   - Added clarifying comments

## Testing Recommendations

To verify these fixes:

```bash
# Test basic slave configuration
cargo run --package tui_e2e -- --module modbus_tui_slave_cli_master

# Test multi-master configurations
cargo run --package tui_e2e -- --module tui_multi_master_mixed_types
cargo run --package tui_e2e -- --module tui_multi_master_spaced_addresses
cargo run --package tui_e2e -- --module tui_multi_master_mixed_ids

# Test multi-slave configurations (also fixed)
cargo run --package tui_e2e -- --module tui_multi_slave_mixed_types
cargo run --package tui_e2e -- --module tui_multi_slave_spaced_addresses
cargo run --package tui_e2e -- --module tui_multi_slave_mixed_ids
```

## Impact

These fixes ensure:
1. Field values are properly cleared before editing
2. Multi-station navigation correctly positions cursor on intended fields
3. Register value configuration works correctly for multiple registers
4. All multi-station tests (both master and slave modes) use consistent navigation logic

The fixes follow the established patterns in the codebase and align with the TUI navigation logic defined in `src/tui/ui/pages/modbus_panel/input/navigation.rs`.
