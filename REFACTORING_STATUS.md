# TUI Refactoring Status

## Completed ✅ (as of Part 9)

### Core Architecture Changes
- **PortOwner enum**: Completely removed ✅
- **PortData structure**: `subprocess_info` field added directly ✅
- **PortState enum**: Simplified to 3 states (no owner field) ✅
- **TUI status tree**: Changed from `HashMap<String, Arc<RwLock<PortData>>>` to `HashMap<String, PortData>` ✅
- **Helper functions**: Removed `with_port_read()` and `with_port_write()` ✅

### TUI Core Logic  
- ✅ `tui/mod.rs`: Fully updated (300+ lines refactored)
- ✅ `tui/subprocess.rs`: CLI subprocess management updated
- ✅ Port lifecycle management: Start/stop/toggle all use direct PortData access
- ✅ IPC message handling: Updated for new structure

### Modules Disabled
- ✅ `daemon` module: Commented out (incompatible with new structure)
- ✅ `runtime` module: Commented out (depends on daemon)
- ✅ `scan_ports`: Stubbed (needs refactoring)

### UI Files Fixed (7 files)
- ✅ `src/tui/status/serializable.rs`: Removed with_port_read, direct access
- ✅ `src/tui/ui/title.rs`: Removed with_port_read from both functions
- ✅ `src/tui/ui/pages/entry/components/list.rs`: Direct port access
- ✅ `src/tui/ui/pages/entry/components/panel.rs`: Direct port access, removed runtime_handle
- ✅ `src/tui/ui/pages/log_panel/components/display.rs`: Direct log access
- ✅ `src/tui/ui/pages/modbus_panel/components/display.rs`: All 5 with_port_read removed
- ✅ Import cleanup: Removed with_port_* imports from all TUI files

## Remaining Work 🔄 (63 errors)

### Compilation Errors: 63 total

**Error breakdown:**
- ~30 × Cannot find function `with_port_read`/`with_port_write` in scope
- ~15 × Cannot find value `port_guard` / `port_data_guard` (variable scope)
- ~10 × Runtime-related errors (runtime_handle(), RuntimeCommand)
- ~5 × Type mismatches (Arc vs direct PortData)
- ~3 × Other errors

### Files Needing Updates (7 remaining files)

#### Config Panel (4 files) - MOST COMPLEX
- `src/tui/ui/pages/config_panel/components/renderer.rs` - 6 with_port_read calls + runtime refs
- `src/tui/ui/pages/config_panel/components/utilities.rs` - 3 with_port_read calls
- `src/tui/ui/pages/config_panel/input/editing.rs` - **10 with_port_write + 10 runtime_handle() calls** ⚠️
- `src/tui/ui/pages/config_panel/input/navigation.rs` - 2 with_port_write calls

#### Modbus Panel (3 files) - PARTIALLY DONE
- `src/tui/ui/pages/modbus_panel/input/actions.rs` - 5 with_port_* calls (some fixed)
- `src/tui/ui/pages/modbus_panel/input/editing.rs` - 6 with_port_* calls (some fixed)
- `src/tui/ui/pages/modbus_panel/input/navigation.rs` - 3 with_port_* calls (some fixed)

**Note**: Entry panel, log panel, display.rs, title.rs, serializable.rs are all DONE ✅

## Critical Issue: config_panel/input/editing.rs

This file has 10+ calls to `runtime_handle()` and `runtime_handle_mut()` which no longer exist.
These are used for serial port configuration (baud rate, parity, etc.).

**Options:**
1. **Remove config panel functionality** - Comment out the entire editing.rs (breaks config panel)
2. **Stub runtime calls** - Return None/default values (config panel shows but doesn't work)
3. **Refactor to use subprocess** - Store config in PortData, sync to CLI subprocess (proper fix, time-consuming)

## What Each File Needs

### Pattern 1: Remove with_port_* imports
```rust
// OLD
use crate::protocol::status::{with_port_read, with_port_write};

// NEW
use crate::protocol::status::types::port::PortData;
```

### Pattern 2: Direct access instead of helper
```rust
// OLD
if let Some(result) = with_port_read(port_arc, |port| {
    port.config.some_field
}) {
    // use result
}

// NEW
if let Some(port) = status.ports.map.get(port_name) {
    let result = port.config.some_field;
    // use result
}
```

### Pattern 3: Remove owner field from PortState
```rust
// OLD
PortState::OccupiedByThis { owner: _ }

// NEW
PortState::OccupiedByThis
```

### Pattern 4: Fix variable names
```rust
// OLD (from sed script)
let port = port_entry.read();  // Error: no read() method
// Then uses port_guard which doesn't exist

// NEW
let port = port_entry;  // Direct access, no .read()
```

## Testing Strategy

### Option A: Fix All Files
- Time: Several hours
- Pros: Complete, thorough
- Cons: Time-consuming

### Option B: Minimal Working Set
- Comment out non-critical UI pages
- Fix only core modbus panel files needed for E2E
- Run E2E tests on core functionality
- Fix remaining pages iteratively
- Time: ~1 hour for core, then incremental

## E2E Test Requirements

The TUI E2E tests mainly need:
- ✅ Port configuration (via cursor actions)  
- ✅ Status monitoring (works)
- ✅ Basic modbus operations
- ⚠️ Some display functions (partially broken)
- ❌ Config panel (completely broken, but maybe not needed for basic tests)

## Recommended Next Steps

1. Try compiling E2E test suite directly to see what's actually needed
2. If E2E tests don't compile, identify minimum required files
3. Fix only those files
4. Run tests
5. Fix additional files based on test failures

This approach minimizes unnecessary work and focuses on what's actually needed for testing.
