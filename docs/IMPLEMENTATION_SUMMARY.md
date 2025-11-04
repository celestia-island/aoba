# Implementation Summary: TUI IPC Mode for E2E Testing

## Overview

Successfully implemented IPC (Inter-Process Communication) mode for the TUI to enable automated E2E testing without terminal emulation dependencies.

## Changes Made

### 1. Command Line Interface (`packages/cli/src/lib.rs`)

Added `--debug-ci` flag:
```rust
.arg(
    Arg::new("debug-ci")
        .long("debug-ci")
        .help("Enable CI mode for IPC-based E2E testing: TUI listens for keyboard events via IPC")
        .action(clap::ArgAction::SetTrue)
        .hide(true), // Hidden from normal help output
)
```

### 2. TUI Startup Logic (`packages/tui/src/tui/mod.rs`)

**Modified `start()` function:**
- Detects `--debug-ci` flag
- Routes to `start_with_ipc()` when flag is present

**Added `start_with_ipc()` function (200+ lines):**
- Initializes TUI without real terminal
- Uses `ratatui::TestBackend` for headless rendering
- Creates core processing thread
- Enters IPC message loop (stdin/stdout)
- Handles 4 message types:
  - `key_press`: Simulate keyboard events
  - `char_input`: Simulate character input
  - `request_screen`: Render and return screen content
  - `shutdown`: Clean shutdown

**Added `parse_key_string()` utility:**
- Converts string key names to crossterm KeyEvent
- Supports special keys, arrows, Ctrl+ combinations

### 3. Input Handler (`packages/tui/src/tui/input.rs`)

Made `handle_event()` public:
```rust
pub fn handle_event(event: crossterm::event::Event, bus: &Bus) -> Result<()>
```

This allows IPC mode to route keyboard events through the existing input handling infrastructure.

### 4. Documentation (`docs/IPC_MODE.md`)

- Complete usage guide
- Message protocol specification
- Integration examples
- Advantages and implementation details

### 5. Testing (`scripts/test_ipc_mode.sh`)

Automated test suite with 5 tests:
1. Screen request
2. Keyboard + screen
3. Character input
4. Graceful shutdown
5. Invalid JSON handling

**All tests pass: 5/5 ✓**

## Technical Details

### Message Protocol

**Request format (JSON over stdin):**
```json
{"type": "key_press", "key": "Enter"}
{"type": "char_input", "ch": "a"}
{"type": "request_screen"}
{"type": "shutdown"}
```

**Response format (JSON over stdout):**
```json
{
  "type": "screen_content",
  "content": "...",
  "width": 120,
  "height": 40
}
```

### Architecture

```
E2E Test Process         TUI Process (--debug-ci)
─────────────────        ─────────────────────────
                         
stdin (JSON) ────────────> IPC Message Loop
                             │
                             ├─> parse_key_string()
                             │     │
                             │     └─> handle_event()
                             │           │
                             │           └─> Page handlers
                             │
                             ├─> render_ui()
                             │     │
                             │     └─> TestBackend
                             │
stdout (JSON) <──────────── Screen Content
```

### Key Benefits

1. **No Terminal Dependencies**: Uses TestBackend, no expectrl/vt100
2. **Reliable**: Direct access to rendered content
3. **Fast**: No terminal emulation overhead
4. **Portable**: Works on all platforms via stdin/stdout
5. **Maintainable**: Reuses existing input handling code

## Testing Results

### Manual Testing
```bash
$ echo '{"type": "request_screen"}' | cargo run --package aoba -- --tui --debug-ci
{"type":"screen_content","content":"...","width":120,"height":40}
```

### Automated Testing
```bash
$ ./scripts/test_ipc_mode.sh
=== TUI IPC Mode Test Suite ===
Test 1: Screen request... ✓
Test 2: Keyboard + screen... ✓
Test 3: Character input... ✓
Test 4: Graceful shutdown... ✓
Test 5: Invalid JSON handling... ✓

Tests run: 5
Tests passed: 5
All tests passed!
```

### Build & Clippy
- ✓ Debug build succeeds
- ✓ Release build succeeds
- ✓ All workspace tests pass (4 tests)
- ✓ Clippy passes (only pre-existing warnings in other code)

## Files Modified

- `packages/cli/src/lib.rs` (+4 lines)
- `packages/tui/src/tui/mod.rs` (+237 lines, ~241 net)
- `packages/tui/src/tui/input.rs` (+1 line, pub visibility)

## Files Created

- `docs/IPC_MODE.md` (complete documentation)
- `scripts/test_ipc_mode.sh` (test suite)

## Compatibility

- ✓ Backward compatible (existing functionality unchanged)
- ✓ Feature is opt-in via `--debug-ci` flag
- ✓ No dependencies on external IPC libraries
- ✓ Cross-platform (stdin/stdout works everywhere)

## Next Steps

The E2E test framework in `examples/tui_e2e` can now:
1. Spawn TUI with `--debug-ci` flag
2. Send keyboard events and receive rendered screens
3. Test TUI behavior without terminal emulation

This provides a solid foundation for comprehensive E2E testing.

## Commits

1. `9a19617` - Implement --debug-ci IPC support for TUI E2E testing
2. `8d2b714` - Fix clippy warning: use strip_prefix instead of manual indexing
3. `fcbe1f8` - Add documentation and tests for IPC mode
