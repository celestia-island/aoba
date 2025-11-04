# TUI IPC Mode for E2E Testing

## Overview

The TUI now supports an IPC mode (activated with `--debug-ci`) that allows E2E tests to:
- Send keyboard events to the TUI
- Request and receive rendered screen content
- Test TUI behavior without terminal emulation

## Usage

### Starting TUI in IPC Mode

```bash
cargo run --package aoba -- --tui --debug-ci
```

The TUI will:
1. Initialize without a real terminal
2. Use `ratatui::TestBackend` for rendering
3. Listen for JSON messages on stdin
4. Send JSON responses on stdout

### Message Protocol

#### Request Messages (stdin)

**Key Press:**
```json
{"type": "key_press", "key": "Enter"}
```

Supported keys:
- Special keys: `"Enter"`, `"Esc"`, `"Escape"`, `"Backspace"`, `"Tab"`
- Arrow keys: `"Up"`, `"Down"`, `"Left"`, `"Right"`
- Page navigation: `"PageUp"`, `"PageDown"`, `"Home"`, `"End"`
- Single characters: `"a"`, `"b"`, etc. or `"Char(a)"`, `"Char(b)"`, etc.
- Ctrl combinations: `"Ctrl+c"`, `"Ctrl+s"`, `"Ctrl+a"`, `"Ctrl+Esc"`, `"Ctrl+PageUp"`

**Character Input:**
```json
{"type": "char_input", "ch": "a"}
```

**Request Screen Content:**
```json
{"type": "request_screen"}
```

**Shutdown:**
```json
{"type": "shutdown"}
```

#### Response Messages (stdout)

**Screen Content:**
```json
{
  "type": "screen_content",
  "content": "...",
  "width": 120,
  "height": 40
}
```

### Example Test Session

```bash
(
  echo '{"type": "key_press", "key": "Down"}'
  sleep 0.5
  echo '{"type": "request_screen"}'
  sleep 0.5
  echo '{"type": "shutdown"}'
) | cargo run --package aoba -- --tui --debug-ci
```

### Integration with E2E Tests

The E2E test framework in `examples/tui_e2e` can now:

1. Spawn TUI process:
```rust
let child = Command::new("cargo")
    .args(&["run", "--package", "aoba", "--", "--tui", "--debug-ci"])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .spawn()?;
```

2. Send keyboard events:
```rust
let stdin = child.stdin.as_mut().unwrap();
writeln!(stdin, r#"{{"type": "key_press", "key": "Down"}}"#)?;
```

3. Request and verify screen:
```rust
writeln!(stdin, r#"{{"type": "request_screen"}}"#)?;
let stdout = BufReader::new(child.stdout);
let response: serde_json::Value = serde_json::from_reader(stdout)?;
assert_eq!(response["type"], "screen_content");
```

## Advantages

1. **Reliability**: No terminal parsing, direct access to rendered content
2. **Speed**: Faster than terminal emulation with expectrl
3. **Simplicity**: Clean JSON protocol
4. **Portability**: Works on all platforms without terminal dependencies

## Implementation Details

- Location: `packages/tui/src/tui/mod.rs` - `start_with_ipc()` function
- Backend: `ratatui::TestBackend` for headless rendering
- Input routing: Uses existing `input::handle_event()` for consistency
- Protocol: JSON over stdin/stdout

## Testing

Run the test suite:
```bash
./scripts/test_ipc_mode.sh
```

Or test manually:
```bash
echo '{"type": "request_screen"}' | cargo run --package aoba -- --tui --debug-ci
```
