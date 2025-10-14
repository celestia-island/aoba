# TUI Slave IPC Integration Fix

## Problem Summary

The `tui_slave` E2E test was failing because data sent from an external CLI master was not being displayed in the TUI. The test showed:

```
External CLI master sends: [31482, 13908, 64517, ...]
TUI displays:             [0x0000, 0x0000, 0x0000, ...]  ❌
```

## Root Cause

The `handle_slave_poll_persist` function in `src/cli/modbus/slave.rs` was missing IPC (Inter-Process Communication) integration. When TUI spawned a CLI subprocess in slave-poll mode:

1. ✅ CLI subprocess successfully polled the external master
2. ✅ CLI subprocess received data from master  
3. ❌ CLI subprocess never sent RegisterUpdate messages to TUI via IPC
4. ❌ TUI never received updates, so registers showed as 0x0000

Compare with `handle_slave_listen_persist` which HAD IPC integration and worked correctly.

## Solution Implemented

### Changes to `src/cli/modbus/slave.rs`

#### 1. Added IPC Setup
```rust
// Setup IPC if requested (like handle_slave_listen_persist does)
let mut ipc = crate::cli::actions::setup_ipc(matches);
```

#### 2. Added PortOpened/PortError Messages
```rust
// Notify IPC when port opens successfully
if let Some(ref mut ipc_conns) = ipc {
    let _ = ipc_conns.status.send(&IpcMessage::PortOpened {
        port_name: port.to_string(),
        timestamp: None,
    });
    log::info!("IPC: Sent PortOpened message for {port}");
}
```

#### 3. Added RegisterUpdate Messages  
```rust
// Send RegisterUpdate via IPC when data is received
if let Some(ref mut ipc_conns) = ipc {
    log::info!(
        "IPC: Sending RegisterUpdate for {port}: station={station_id}, values={:?}",
        response.values
    );
    let _ = ipc_conns.status.send(&IpcMessage::RegisterUpdate {
        port_name: port.to_string(),
        station_id,
        register_type: register_mode.clone(),
        start_address: register_address,
        values: response.values.clone(),
        timestamp: None,
    });
}
```

#### 4. Added Cleanup Handler
```rust
// Register cleanup to ensure port is released on program exit  
{
    let pa = port_arc.clone();
    let port_name_clone = port.to_string();
    cleanup::register_cleanup(move || {
        log::debug!("Cleanup handler: Releasing port {port_name_clone}");
        if let Ok(mut port) = pa.lock() {
            let _ = std::io::Write::flush(&mut **port);
            log::debug!("Cleanup handler: Flushed port {port_name_clone}");
        }
        drop(pa);
        std::thread::sleep(Duration::from_millis(200));
        log::debug!("Cleanup handler: Port {port_name_clone} released");
    });
    log::debug!("Registered cleanup handler for port {port}");
}
```

### Changes to `src/cli/modbus/master.rs` and `slave.rs`

Added comprehensive logging throughout to help debug issues:

- `send_request_and_wait`: Log request/response bytes and parsed values
- `respond_to_request`: Log frame parsing, function codes, and responses
- All IPC operations logged with INFO level

## Verification

### Test 1: Basic Modbus Communication ✅

```bash
$ target/debug/aoba --master-provide-persist /tmp/vcom2 --data-source file:test.json &
$ target/debug/aoba --slave-poll /tmp/vcom1 --json

Result: {"values":[111,222,333,444,555,666,777,888,999,1010,1111,1212],...}
Status: PASSED ✅
```

### Test 2: IPC Integration ✅

Created a Python IPC listener to simulate TUI and tested the full flow:

```
[IPC Listener] Connection accepted!
✅ PortOpened: /tmp/vcom1
✅ RegisterUpdate #1: station=1, addr=0x0000, values=[1111, 2222, 3333, ...]
Status: PASSED ✅
```

This confirms:
1. CLI subprocess connects to IPC channel ✅
2. Sends PortOpened message ✅
3. Polls master and receives data ✅  
4. Sends RegisterUpdate via IPC ✅

## Data Flow (Fixed)

**Before:**
```
TUI → Spawns CLI subprocess
      ↓
CLI subprocess → Opens port
      ↓
CLI subprocess → Polls external master
      ↓
CLI subprocess → Receives data [111, 222, ...]
      ↓
CLI subprocess → ❌ STOPS HERE (no IPC)
      
TUI → ❌ Never receives updates → Shows [0x0000, ...]
```

**After:**
```
TUI → Spawns CLI subprocess with IPC channel
      ↓
CLI subprocess → Opens port → Sends PortOpened via IPC
      ↓
CLI subprocess → Polls external master
      ↓
CLI subprocess → Receives data [111, 222, ...]
      ↓
CLI subprocess → ✅ Sends RegisterUpdate via IPC
      ↓
TUI → ✅ Receives RegisterUpdate → Updates display
```

## Debugging Recommendations

To debug similar issues in the future:

1. **Check CLI subprocess logs** - Look for IPC messages:
   ```
   grep -E "IPC|RegisterUpdate|PortOpened" /path/to/cli.log
   ```

2. **Check TUI IPC handler logs** - In `src/tui/mod.rs`:
   ```rust
   log::info!("CLI[{port_name}]: RegisterUpdate station={station_id}, values={values:?}");
   ```

3. **Use --debug flag** - The E2E tests support debug mode:
   ```bash
   DEBUG_MODE=1 cargo run --example tui_e2e
   ```

4. **Enable detailed logging**:
   ```bash
   RUST_LOG=debug cargo run --example tui_e2e
   ```

## Files Modified

1. `src/cli/modbus/slave.rs` - Added IPC integration to `handle_slave_poll_persist`
2. `src/cli/modbus/master.rs` - Enhanced logging in `respond_to_request`  
3. `src/cli/modbus/slave.rs` - Enhanced logging in `send_request_and_wait`

## Summary

The fix adds IPC communication to `handle_slave_poll_persist` so that when TUI spawns a CLI subprocess in slave-poll mode, the subprocess properly reports received data back to TUI via RegisterUpdate messages. This allows TUI to display the data correctly instead of showing all zeros.

The solution follows the existing pattern used in `handle_slave_listen_persist` and includes comprehensive logging for future debugging.
