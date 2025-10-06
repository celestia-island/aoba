# Hybrid TUI + CLI Tests

This directory contains hybrid tests that combine TUI and CLI for more robust and easier-to-debug testing.

## Test Structure

### 1. TUI Master + CLI Slave (`tui_master_cli_slave.rs`)

**Setup:**
- TUI runs as Modbus Master (Slave/Server) on `/dev/vcom1`
- CLI runs as Modbus Slave (Master/Client) on `/dev/vcom2`

**Test Flow:**
1. Start TUI and navigate to vcom1
2. Configure TUI as Master with test values (0, 10, 20, 30)
3. Enable the port
4. Run CLI slave poll command to request data
5. Verify CLI receives the correct values

**Benefits:**
- TUI provides data (easy to set up visually)
- CLI polls (easy to verify output programmatically)
- Simpler than full TUI-TUI test

### 2. CLI Master + TUI Slave (`cli_master_tui_slave.rs`)

**Setup:**
- CLI runs as Modbus Master (Slave/Server) on `/dev/vcom2`
- TUI runs as Modbus Slave (Master/Client) on `/dev/vcom1`

**Test Flow:**
1. Start CLI master in persistent mode with test data (5, 15, 25, 35)
2. Start TUI and navigate to vcom1
3. Configure TUI as Slave
4. Enable the port
5. Wait for communication
6. Check TUI display for received values

**Benefits:**
- CLI provides data (easy to control)
- TUI polls (visual verification possible)
- Tests TUI's ability to receive data from external sources

## Running the Tests

### Prerequisites

Virtual COM ports must be available (vcom1 and vcom2 paired).

On Linux with socat:
```bash
# In a separate terminal, create virtual ports
socat -d -d pty,raw,echo=0,link=/dev/vcom1 pty,raw,echo=0,link=/dev/vcom2
```

### Run All Tests

```bash
cd examples/tui_e2e_tests
cargo run
```

This will run:
1. Full TUI master-slave test (original)
2. TUI Master + CLI Slave hybrid test
3. CLI Master + TUI Slave hybrid test

### Run Specific Test

You can modify `main.rs` to run only specific tests, or run with filters:

```bash
# Run with verbose logging
RUST_LOG=debug cargo run
```

## Test Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Hybrid Test Framework                     │
└─────────────────────────────────────────────────────────────┘
                               │
              ┌────────────────┴────────────────┐
              │                                 │
    ┌─────────▼──────────┐          ┌─────────▼──────────┐
    │  TUI Master +      │          │  CLI Master +      │
    │  CLI Slave Test    │          │  TUI Slave Test    │
    └────────────────────┘          └────────────────────┘
              │                                 │
    ┌─────────▼──────────┐          ┌─────────▼──────────┐
    │ TUI (vcom1)        │          │ CLI (vcom2)        │
    │ - Master mode      │◄────────►│ - Master mode      │
    │ - Provides data    │  Serial  │ - Provides data    │
    └────────────────────┘          └────────────────────┘
              │                                 │
    ┌─────────▼──────────┐          ┌─────────▼──────────┐
    │ CLI (vcom2)        │          │ TUI (vcom1)        │
    │ - Slave mode       │          │ - Slave mode       │
    │ - Polls data       │          │ - Polls data       │
    └────────────────────┘          └────────────────────┘
```

## Advantages of Hybrid Testing

1. **Easier Debugging**: CLI output is plain text, easy to parse
2. **Better Coverage**: Tests both TUI and CLI implementations
3. **Faster Iteration**: CLI commands are quicker than UI automation
4. **More Reliable**: Less dependent on UI timing and rendering
5. **Complementary**: Each approach tests different aspects

## Implementation Details

### TUI Automation
Uses `expectrl` and `auto_cursor` framework to:
- Navigate TUI menus
- Enter values
- Verify screen content

### CLI Execution
Uses `std::process::Command` to:
- Run CLI commands
- Capture output
- Verify results

### Communication
- Virtual serial ports (vcom1 ↔ vcom2)
- Modbus RTU protocol
- 9600 baud rate (default)

## Troubleshooting

### "Port not found" errors
- Ensure virtual COM ports are created and paired
- Check permissions: `sudo chmod 666 /dev/vcom*`

### "Pattern not found" in TUI
- Check screen capture logs for actual content
- UI timing might need adjustment (increase sleep times)

### CLI command fails
- Verify CLI commands work standalone first
- Check if port is already in use

### Communication timeout
- Ensure ports are properly paired
- Check baud rate matches on both sides
- Look for errors in debug logs

## Future Improvements

- Add more test scenarios (different baud rates, register types)
- Add negative tests (error handling)
- Add performance tests (throughput, latency)
- Add stress tests (many requests, long running)
