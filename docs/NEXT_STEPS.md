# Next Steps - Quick Start Guide

## Current State (Commit: 5e168dd)

✅ **All infrastructure complete**
- Config structure redesigned
- IPC messages updated
- Conversion layer working
- CLI master integrated
- TUI helpers ready
- All tests passing (4/4)

## To Continue Work

### 1. Wire Up TUI Events (30-60 min)

**Goal**: Call `send_stations_update_for_port()` when config changes

**Files to modify**:
```
src/tui/ui/pages/modbus_panel/input/actions.rs
src/tui/ui/pages/modbus_panel/input/editing.rs
```

**What to do**:
Find `SendRegisterUpdate` usage and either:
- Replace with `send_stations_update_for_port()` call, OR
- Add periodic full sync in addition to individual updates

**Test**:
```bash
cargo check
cargo test --lib
```

### 2. Run E2E Tests (Initial Run)

**Setup**:
```bash
cd /home/runner/work/aoba/aoba
scripts/socat_init.sh --mode tui
```

**Run tests with logging**:
```bash
cd examples/tui_e2e
cargo run --release -- --skip-basic 2>&1 | tee /tmp/multi_test_$(date +%Y%m%d_%H%M%S).log
```

⚠️ **Note**: First run takes ~5-10 minutes to compile

### 3. Analyze Test Results

**Check log for**:
- ✅ Test starts successfully
- ✅ TUI spawns
- ✅ Ports configured
- ✅ IPC connection established
- ❌ Any errors or panics
- ❌ Timeout issues
- ❌ Serialization failures

**Common issues to look for**:
```
"IPC: Failed to connect"          → Socket not created
"Failed to deserialize"            → Version mismatch
"Port not found"                   → Port config issue
"Timeout waiting"                  → Timing issue
```

### 4. Fix Issues Found

**Create improvement plan**:
1. List all failures from log
2. Group by root cause
3. Prioritize critical issues
4. Plan fixes

**Implement fixes**:
```bash
# Make changes
cargo check
cargo test --lib
cargo clippy

# Commit
git add -A
git commit -m "Fix: [description]"
```

### 5. Iterate

Repeat steps 2-4 until:
- ✅ multi_masters tests pass
- ✅ multi_slaves tests pass
- ✅ No IPC errors
- ✅ All register values propagate correctly

## Quick Commands

### Build & Test
```bash
cargo check                    # Quick compile check
cargo test --lib              # Run unit tests
cargo clippy --all-targets    # Lint check
cargo fmt                     # Format code
cargo build --release         # Full release build
```

### Reset Environment
```bash
scripts/socat_init.sh --mode tui    # Reset virtual ports
pkill -f aoba                        # Kill any running instances
rm /tmp/aoba_*                       # Clean temp files
```

### Debug Mode
```bash
# Run with debug logging and screenshots
cd examples/tui_e2e
DEBUG_MODE=1 cargo run --release -- --skip-basic debug
```

### Check Specific Test
```bash
cd examples/tui_e2e
cargo run --release -- multi-masters  # Only multi_masters
cargo run --release -- multi-slaves   # Only multi_slaves
```

## Files You'll Likely Edit

### For TUI Integration
- `src/tui/ui/pages/modbus_panel/input/actions.rs` - Button actions
- `src/tui/ui/pages/modbus_panel/input/editing.rs` - Edit handlers
- `src/tui/mod.rs` - Core message handling

### For Fixing Test Issues
- `src/cli/modbus/master.rs` - CLI master logic
- `src/cli/config_convert.rs` - Conversion issues
- `src/protocol/ipc.rs` - IPC message handling

### For Understanding Flow
- `docs/CONFIG_REDESIGN.md` - Architecture overview
- `src/cli/config.rs` - Config structure definitions
- `examples/tui_e2e/src/e2e/multi_masters/` - Master tests
- `examples/tui_e2e/src/e2e/multi_slaves/` - Slave tests

## Debugging Tips

### Enable Verbose Logging
```rust
log::info!("🔍 Station config: {:?}", stations);
log::debug!("📤 Sending IPC: {} bytes", data.len());
```

### Check IPC Communication
```bash
# Watch for IPC socket creation
ls -la /tmp/aoba-ipc-* 

# Check if processes are connected
lsof | grep aoba
```

### Verify Register Values
```rust
// In CLI master
log::info!("📊 Holding regs: {:?}", context.get_holdings_as_u16());
```

### Screenshot on Failure
```rust
// In test
if result.is_err() {
    tui_cap.capture(&mut session, "failure_state").await?;
}
```

## Expected Timeline

- **Wire TUI events**: 30-60 minutes
- **First E2E test run**: 10-15 minutes (compilation + execution)
- **Log analysis**: 15-30 minutes per iteration
- **Fix implementation**: 1-2 hours per major issue
- **Full cycle**: 2-4 iterations typically needed

## Success Criteria

When done, you should have:
- ✅ All unit tests passing
- ✅ multi_masters E2E test passing
- ✅ multi_slaves E2E test passing
- ✅ No clippy warnings
- ✅ Code formatted
- ✅ Documentation updated

## Getting Help

If stuck:
1. Check `docs/CONFIG_REDESIGN.md` for architecture details
2. Review test logs for specific errors
3. Run unit tests in isolation: `cargo test --lib test_name`
4. Use `cargo expand` to see macro expansions
5. Add more logging and re-run tests

## Commit Message Format

```
Fix: [brief description]

- Detail 1
- Detail 2

Addresses: [issue/test that was failing]
```

Example:
```
Fix: Apply StationsUpdate to all register types in CLI master

- Added handling for coils and discrete inputs
- Fixed address range calculation
- Added debug logging for verification

Addresses: multi_masters test timeout
```
