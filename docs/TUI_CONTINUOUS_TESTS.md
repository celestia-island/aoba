# TUI E2E Continuous Tests

This document describes the continuous testing suite for TUI (Terminal User Interface) modbus operations.

## Overview

The continuous tests verify that TUI can correctly handle continuous random data updates in both Master and Slave modes, testing all supported Modbus register types:

**TUI Master Test** - Tests all 4 register types:
- `holding` - Holding registers (read/write, 16-bit)
- `input` - Input registers (read-only, 16-bit)
- `coils` - Coils (read/write, 1-bit boolean)
- `discrete` - Discrete inputs (read-only, 1-bit boolean)

**TUI Slave Test** - Tests only writable types:
- `holding` - Holding registers (read/write, 16-bit)
- `coils` - Coils (read/write, 1-bit boolean)

Note: The slave test only uses writable register types because the CLI master can only write to holding registers and coils. Input registers and discrete inputs are read-only and cannot be written by the master.

## Test Files

### 1. TUI Master Continuous Test (`tui_e2e_test_master_continuous.rs`)

**Purpose**: Tests TUI acting as Modbus Master (Server) with continuous random data updates.

**Test Flow**:
1. Start TUI and configure it as Modbus Master
2. Set initial random register values
3. Enable the serial port
4. Start CLI slave in persistent mode to continuously poll
5. Perform 3 iterations of random data updates in TUI
6. Verify CLI slave received all expected value sets

**Key Features**:
- Tests all 4 register types (holding, input, coils, discrete)
- Generates random data appropriate for each type (0-1 for coils/discrete, full range for holding/input)
- Uses coordinate correction when navigating TUI interface
- Validates data integrity through JSONL output from CLI slave

**Usage**:
```bash
cargo run --example tui_e2e_test_master_continuous
```

### 2. TUI Slave Continuous Test (`tui_e2e_test_slave_continuous.rs`)

**Purpose**: Tests TUI acting as Modbus Slave (Client) polling from CLI Master (Server).

**Test Flow**:
1. Create data file with 5 sets of random values
2. Start CLI master in persistent mode with data file
3. Start TUI and configure it as Modbus Slave
4. Enable the serial port (starts automatic polling)
5. Wait for communication to occur
6. Capture TUI display multiple times to verify received values
7. Verify at least some expected values were captured

**Key Features**:
- Tests writable register types only (holding, coils) - master can only write to these
- CLI master provides continuous data stream
- TUI slave automatically polls and updates display
- **Uses TUI log file (`/tmp/tui_e2e.log`) for data verification** - more reliable than screen scraping
- Maintains TUI interaction tests for UI validation
- Screen capture checkpoints to verify display consistency

**Usage**:
```bash
cargo run --example tui_e2e_test_slave_continuous
```

## New CLI Functionality

### `--slave-poll-persist`

A new CLI option that enables continuous polling mode for the slave:

```bash
aoba --slave-poll-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-mode holding \
  --register-address 0 \
  --register-length 10 \
  --output file:/tmp/output.json
```

**Features**:
- Continuously polls the master for data
- Outputs JSONL (JSON Lines) format for easy parsing
- Supports all register types (holding, input, coils, discrete)
- Can output to stdout, file, or named pipe
- Includes proper error handling and retry logic

## Register Type Details

### Holding Registers (Function Code 03)
- 16-bit read/write registers
- Used for configuration and operational data
- Value range: 0-65535

### Input Registers (Function Code 04)
- 16-bit read-only registers
- Used for sensor data and status information
- Value range: 0-65535

### Coils (Function Code 01)
- 1-bit read/write registers
- Used for binary outputs (relays, LEDs, etc.)
- Value range: 0 (OFF) or 1 (ON)

### Discrete Inputs (Function Code 02)
- 1-bit read-only registers
- Used for binary inputs (switches, sensors, etc.)
- Value range: 0 (OFF) or 1 (ON)

## Coordinate Correction

The tests include coordinate correction when navigating the TUI interface:
- Automatically detects cursor position in port list
- Calculates required arrow key presses to reach target port
- Handles both upward and downward navigation
- Verifies successful navigation before proceeding

## CI Integration

The continuous tests are integrated into the GitHub Actions workflow:

```yaml
strategy:
  matrix:
    example: [
      cli_e2e,
      tui_e2e_test_master,
      tui_e2e_test_slave,
      tui_e2e_test_master_continuous,
      tui_e2e_test_slave_continuous
    ]
```

All tests run on Ubuntu with virtual serial ports created by `socat`.

## Implementation Notes

### Random Data Generation
- For holding/input registers: Full u16 range (0-65535)
- For coils/discrete: Binary values (0 or 1)
- Uses `rand` crate for pseudo-random generation

### Data Verification
- **Master test**: Verifies CLI slave JSONL output file contains expected values
- **Slave test**: Parses TUI log file (`/tmp/tui_e2e.log`) for "Received holding registers" / "Received coils" entries
- Screen capture checkpoints verify display consistency
- Accepts partial matches due to timing variations
- Logs detailed information for debugging

### Why Log-Based Verification?
The slave test uses TUI log parsing instead of screen scraping because:
1. **More Reliable**: Log entries have structured format that's easier to parse
2. **Complete Data**: Logs capture all received values, not just what's visible on screen
3. **Better Debugging**: Log file can be inspected independently if tests fail
4. **Maintains UI Tests**: Screen capture still happens at checkpoints for visual verification

### Error Handling
- Graceful handling of timing issues
- Proper cleanup of processes and temporary files
- Clear error messages with context
- No false positives from unrelated issues

## Testing Locally

1. Install dependencies:
```bash
sudo apt-get install socat
```

2. Setup virtual serial ports:
```bash
sudo ./scripts/socat_init.sh
```

3. Run a specific test:
```bash
cargo run --example tui_e2e_test_master_continuous
```

4. Clean up:
```bash
sudo pkill socat
sudo rm -f /tmp/vcom1 /tmp/vcom2
```

## Troubleshooting

### Debug Mode with Breakpoints

The TUI E2E tests now support a debug mode using `CursorAction::DebugBreakpoint` to verify the workflow at key stages. This is especially useful when tests fail due to navigation or timing issues.

**How to use debug breakpoints**:
1. Set the `DEBUG_MODE` environment variable:
```bash
export DEBUG_MODE=1
cargo run --example tui_e2e
```

2. The test will stop at each `DebugBreakpoint`, capture the screen state, and print it to the log
3. After a breakpoint is hit, the virtual serial ports are automatically reset
4. The process exits immediately for manual inspection

**Key debug breakpoints in TUI tests**:
- `after_navigate_to_vcom1` - Verifies port selection
- `after_configure_master` - Verifies return to port details after configuration
- `after_enable_port` - Verifies port is enabled
- `after_enter_modbus_panel` - Verifies entry into Modbus configuration panel
- `after_create_station` - Verifies station creation
- `after_set_register_length` - Verifies register length configuration
- `before_update_registers_round_N` - Verifies state before updating registers
- `after_update_registers_round_N` - Verifies register updates were applied

**Expected workflow verified by breakpoints**:
1. Port selection → Navigate to vcom1 in port list
2. Enable port → Toggle "Enable Port" option
3. Enter configuration panel → Navigate to "Enter Business Configuration"
4. Add station → Create new Modbus station
5. Configure station length → Set register/coil count
6. Register update loop → Update values and wait for communication

### Port Access Issues
If you get permission errors, ensure virtual ports have correct permissions:
```bash
sudo chmod 666 /tmp/vcom1 /tmp/vcom2
```

### Timing Issues
The tests include built-in delays for synchronization. If tests fail intermittently:
- Check system load
- Verify socat is running properly
- Increase sleep durations in test code

### Display Capture Issues
For TUI slave tests, if values aren't captured:
- Verify TUI is updating display properly
- Check that CLI master is sending data
- Review test logs for timing information

### Common Issues and Solutions

**Issue**: Test times out waiting for CLI master/slave to complete
- **Cause**: Missing `enter_modbus_panel()` call after enabling port
- **Solution**: Tests must enter the Modbus panel after enabling the port to access registers for updates
- **Debug**: Use `DEBUG_MODE=1` and check `after_enable_port` and `after_enter_modbus_panel` breakpoints

**Issue**: Port shows as "Disabled" after trying to enable
- **Cause**: Cursor not on "Enable Port" option before pressing Enter
- **Solution**: Navigate to "Enable Port" option first (usually by pressing Up arrow 3 times)
- **Debug**: Use `DEBUG_MODE=1` and check `before_enable` breakpoint to verify cursor position

**Issue**: Register values not updating in TUI
- **Cause**: Not in Modbus configuration panel when trying to update
- **Solution**: Call `enter_modbus_panel()` before `update_tui_registers()`
- **Debug**: Check `before_update_registers_round_1` breakpoint to verify screen shows register editing interface
