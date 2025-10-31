# TUI UI E2E Test Runner

This program generates screenshots for TUI E2E tests by preparing global state trees and rendering them using the actual TUI with terminal capture.

## Overview

The TUI UI E2E framework validates that the TUI correctly renders different states by:
1. Creating a serialized global state representing a specific test scenario
2. Spawning the TUI in screen-capture mode with that state loaded
3. Capturing the rendered terminal output using expectrl + vt100
4. Saving the captured output as text files for verification

## Usage

### Generate All Screenshots

```bash
cargo run --package tui_ui_e2e -- generate-screenshots
```

### Generate Screenshots for Specific Module

For faster testing and debugging, you can filter by module:

```bash
# Generate only common base states
cargo run --package tui_ui_e2e -- generate-screenshots --module common

# Generate single station master mode screenshots
cargo run --package tui_ui_e2e -- generate-screenshots --module single_station/master_modes

# Generate single station slave mode screenshots  
cargo run --package tui_ui_e2e -- generate-screenshots --module single_station/slave_modes

# Generate multi station master mode screenshots
cargo run --package tui_ui_e2e -- generate-screenshots --module multi_station/master_modes

# Generate multi station slave mode screenshots
cargo run --package tui_ui_e2e -- generate-screenshots --module multi_station/slave_modes
```

## Output

Screenshots are saved to `examples/tui_ui_e2e/screenshots/` with the following structure:

```
screenshots/
├── common/
│   ├── single_station_master_base.txt
│   ├── single_station_slave_base.txt
│   ├── multi_station_master_base.txt
│   └── multi_station_slave_base.txt
├── single_station/
│   ├── master_modes/
│   │   ├── tui_master_coils_final.txt
│   │   ├── tui_master_discrete_inputs_final.txt
│   │   ├── tui_master_holding_registers_final.txt
│   │   └── tui_master_input_registers_final.txt
│   └── slave_modes/
│       └── ...
└── multi_station/
    ├── master_modes/
    │   └── ...
    └── slave_modes/
        └── ...
```

## How It Works

### Terminal Capture Architecture

1. **State Preparation**: Test states are created programmatically in `src/e2e/` and serialized to `/tmp/status.json`

2. **TUI Spawn**: The TUI is spawned with `--debug-screen-capture --no-config-cache` flags:
   - `--debug-screen-capture`: Enables one-shot rendering mode
   - `--no-config-cache`: Prevents loading/saving config files

3. **Screen Capture Mode**: In this mode, the TUI:
   - Loads the state from `/tmp/status.json`
   - Enters alternate screen
   - Renders the UI once
   - **Waits for Ctrl+C** (this is crucial - allows parent to capture before cleanup)
   - Exits on Ctrl+C, restoring terminal

4. **Terminal Capture**: The parent process uses:
   - `expectrl` to manage the PTY and spawn the TUI
   - `vt100` parser to process ANSI escape sequences
   - Concrete `Session` types implementing `ExpectSession` for non-blocking reads

5. **Content Extraction**: After rendering, the parent:
   - Reads from the PTY using non-blocking I/O
   - Feeds bytes to vt100 parser
   - Extracts rendered screen contents
   - Sends Ctrl+C to terminate TUI
   - Saves output to file

### Key Implementation Details

**Type System**: Uses `spawn_expect_session_with_size()` instead of `spawn_expect_process()` to return concrete `Session<UnixProcess, PtyStream>` types that implement both `Expect` and `ExpectSession` traits, enabling terminal capture operations.

**Alternate Screen Handling**: The TUI must keep the alternate screen active until capture completes. Earlier implementation immediately called `LeaveAlternateScreen`, clearing the buffer before capture could occur.

**PTY Size**: Terminal is spawned with 40x80 dimensions (Large size) to accommodate TUI content without scrolling.

## Troubleshooting

### Empty Screenshot Files

If screenshots are empty (0 bytes or whitespace only):
- Check TUI logs at `/tmp/tui_e2e.log`
- Verify the state file exists at `/tmp/status.json`
- Ensure TUI is waiting for Ctrl+C before cleanup

### Compilation Errors

If you see trait bound errors about `ExpectSession`:
- Ensure you're using `spawn_expect_session_with_size()` not `spawn_expect_process()`
- Verify `aoba_ci_utils` has been updated with the new functions

### Timeout Issues

If generation times out:
- Use `--module` to test smaller subsets
- Check that Ctrl+C is being sent properly
- Verify no infinite loops in the event handling

## Related Files

- `packages/ci_utils/src/terminal.rs`: Terminal spawn functions
- `packages/ci_utils/src/snapshot.rs`: Terminal capture implementation
- `packages/tui/src/tui/mod.rs`: `run_screen_capture_mode()` function
- `src/e2e/`: Test state generators
