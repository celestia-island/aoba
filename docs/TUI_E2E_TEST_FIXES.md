# TUI E2E Test Fixes

## Summary

This document describes the fixes applied to resolve two main issues in the TUI E2E test for Modbus master-slave communication.

## Issues Identified

### Issue 1: Only 4 of 12 Registers Were Being Set

**Symptom**: The test was only successfully setting the first 4 registers instead of all 12, causing a warning "Register values may not be set correctly".

**Root Cause**: The test was typing decimal values (0, 10, 20, 30, ...) into the register value fields, but the TUI input parser expected hexadecimal format. This caused:
- Typed "10" → Parsed as 0x10 (16 decimal) instead of 10 decimal
- Typed "20" → Parsed as 0x20 (32 decimal) instead of 20 decimal
- And so on...

The verification patterns expected hex values like 0x000A (10 decimal), but the actual values in storage were 0x0010 (16 decimal), causing verification failures for registers beyond the first few.

**Solution**: Modified the test to type hexadecimal strings instead of decimal:
```rust
// Before:
CursorAction::TypeString(format!("{}", i * 10))

// After:
let decimal_value = i * 10;
let hex_string = format!("{:X}", decimal_value);
CursorAction::TypeString(hex_string)
```

Now the test types: 0, A, 14, 1E, 28, 32, 3C, 46, 50, 5A, 64, 6E (hex) which correctly represents 0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110 (decimal).

**File Modified**: `examples/tui_e2e_tests/src/tests/modbus_master_slave/modbus_config.rs`

### Issue 2: Communication Timeout Between Master and Slave

**Symptom**: The CLI slave (or second TUI slave) would timeout waiting for data from the TUI master, with error "Operation timed out".

**Root Causes**:
1. **Master Port Not Enabled**: The master configuration set register values but didn't enable the port, so the Modbus daemon wasn't actually listening for requests.
2. **Insufficient Wait Time**: After enabling the slave port, the test immediately checked for register values without allowing time for:
   - Modbus daemon initialization
   - Slave sending poll request
   - Master responding
   - Slave updating storage
   - UI refreshing

**Solutions**:

1. **Enable Master Port**: Added port enabling step after master configuration:
```rust
// Now enable the port so the master can start responding to requests
log::info!("🔌 Enabling {session_name} port");
let enable_actions = vec![
    CursorAction::PressEnter, // Press Enter on "Enable Port"
    CursorAction::Sleep { ms: 500 },
    CursorAction::MatchPattern {
        pattern: Regex::new("Enabled")?,
        description: "Port shows as 'Enabled'".to_string(),
        line_range: Some((2, 2)),
        col_range: None,
    },
];
```

2. **Add Wait Times for Communication**:
   - 3-second wait after enabling slave port for daemon initialization
   - 2-second wait before verification for data propagation
   - Total 5+ seconds allows multiple 1-second poll cycles

```rust
// Wait for modbus daemon to start and establish communication
CursorAction::Sleep { ms: 3000 },
// ... navigate to Modbus Panel ...
// Allow time for registers to populate from master
CursorAction::Sleep { ms: 2000 },
```

**Files Modified**: 
- `examples/tui_e2e_tests/src/tests/modbus_master_slave/modbus_config.rs`
- `examples/tui_e2e_tests/src/tests/modbus_master_slave/register_operations.rs`

## Test Flow After Fixes

The corrected test flow is:

1. **Master Configuration**:
   - Navigate to vcom1
   - Create Modbus station (defaults to Master mode, Holding registers)
   - Set register length to 12
   - Set all 12 register values using hex format: 0, A, 14, 1E, 28, 32, 3C, 46, 50, 5A, 64, 6E
   - Verify values with pattern matching for each row
   - **Enable port** to start Modbus daemon

2. **Slave Configuration**:
   - Navigate to vcom2
   - Create Modbus station
   - Change mode to Slave (acts as Modbus Master/Client)
   - Set register length to 12
   - Exit configuration (port will be enabled during verification)

3. **Verification**:
   - Enable slave port
   - Wait 3 seconds for daemon initialization
   - Navigate to Modbus panel
   - Wait 2 seconds for data propagation
   - Capture screen and verify all 12 register values

## Key Insights

### Modbus Mode Naming Confusion

The codebase has counter-intuitive naming (kept for backwards compatibility):
- **"Master" mode** → Acts as Modbus Slave/Server → Responds to requests → Has register values
- **"Slave" mode** → Acts as Modbus Master/Client → Sends requests → Polls for values

This is clarified in comments at `src/protocol/daemon/modbus_daemon.rs:55-58`.

### Register Value Storage and Display

- Both Master and Slave modes have their own storage instances
- Master: Storage contains values set via TUI
- Slave: Storage receives values from polling Master
- TUI display reads from storage in real-time
- Default register mode is Holding (read/write)

### Communication Flow

With the fixes in place:
1. Master's Modbus daemon listens on vcom1
2. Slave's Modbus daemon polls vcom1 every 1 second
3. Master responds with register values from storage
4. Slave parses response and writes to its storage
5. Slave's TUI display updates to show received values
6. Test verifies values match expected 0, 10, 20, ..., 110

## Testing

To run the test after these fixes:

```bash
# Setup virtual serial ports
cd examples/tui_e2e_tests
bash scripts/socat_init.sh

# Run the test
cargo run --release
```

The test should now:
- ✅ Set all 12 register values correctly in master
- ✅ Enable both ports in proper sequence
- ✅ Allow sufficient time for Modbus communication
- ✅ Verify all 12 register values appear on slave side
- ✅ Complete without timeouts or verification errors

## Note on "Hybrid" Test

The problem statement mentioned a "hybrid" test combining TUI master with CLI slave (log path: `tui_e2e_tests::tests::hybrid::tui_master_cli_slave`). This test doesn't exist in the current codebase and may be a future enhancement. The current test uses two TUI processes and should work correctly with these fixes.
