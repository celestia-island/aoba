# CLI + TUI Hybrid Testing Guide

## Overview

Testing Modbus master-slave communication can be complex when using only TUI E2E tests, as automating UI interactions requires intricate scripting. This guide explains how to use the CLI (Command Line Interface) in combination with TUI for more efficient testing.

## Benefits of Hybrid Testing

1. **Simpler Setup**: CLI commands are easier to script and automate than UI interactions
2. **Faster Iteration**: No need to navigate through UI menus for every test
3. **Better Debugging**: CLI output is easier to parse and analyze
4. **Complementary Testing**: Test both interfaces to ensure consistent behavior

## Test Scenarios

### Scenario 1: TUI Master + CLI Slave

Test the TUI acting as a Modbus Master (Slave/Server) with CLI as the polling client.

**Setup TUI Master:**
```bash
# Terminal 1: Start TUI with logging
export AOBA_LOG_FILE=/tmp/tui_master.log
./target/debug/aoba --tui

# Navigate to port (e.g., /dev/vcom1)
# Set mode to "Master"
# Configure station with:
#   - Station ID: 1
#   - Register Type: Holding Registers
#   - Start Address: 0
#   - Register Length: 4
# Set register values: 0x0000, 0x000A, 0x0014, 0x001E (0, 10, 20, 30 decimal)
# Enable the port
```

**Run CLI Slave (acts as Modbus Master/Client):**
```bash
# Terminal 2: Use CLI to poll the TUI master
./target/debug/aoba modbus slave poll \
    --port /dev/vcom2 \
    --baud-rate 9600 \
    --station-id 1 \
    --register-mode holding \
    --register-address 0 \
    --register-length 4 \
    --interval 1000

# Expected output: Should show register values 0, 10, 20, 30
```

**Monitor logs:**
```bash
# Terminal 3: Watch the TUI master logs
tail -f /tmp/tui_master.log | grep -E "(TX|RX|Success|Timeout)"
```

### Scenario 2: CLI Master + TUI Slave

Test the CLI providing data as a Modbus Master (Slave/Server) with TUI polling as client.

**Setup CLI Master:**
```bash
# Terminal 1: Start CLI master with test data
# First, create a data file with test values
echo "5 10 15 20" > /tmp/test_data.txt

./target/debug/aoba modbus master provide-persist \
    --port /dev/vcom2 \
    --baud-rate 9600 \
    --station-id 1 \
    --register-mode holding \
    --register-address 0 \
    --register-length 4 \
    --data-source file:/tmp/test_data.txt
```

**Setup TUI Slave:**
```bash
# Terminal 2: Start TUI with logging
export AOBA_LOG_FILE=/tmp/tui_slave.log
./target/debug/aoba --tui

# Navigate to port (e.g., /dev/vcom1)
# Set mode to "Slave"
# Configure station with:
#   - Station ID: 1
#   - Register Type: Holding Registers
#   - Start Address: 0
#   - Register Length: 4
# Enable the port
# Navigate to the Modbus panel to view received values
```

**Verify Results:**
```bash
# Check TUI slave logs for successful communication
tail -f /tmp/tui_slave.log | grep -E "(Success|Timeout|RX)"

# Expected to see:
# - "Master TX (request): ..." (TUI sending requests)
# - "Master RX (response): ..." (TUI receiving responses)
# - "✅ Success response for station 1"
# - Register values should update to: 5, 10, 15, 20
```

## Virtual COM Port Setup

For testing on the same machine, use virtual COM port pairs:

### Linux (socat)
```bash
# Create a virtual COM port pair
socat -d -d pty,raw,echo=0 pty,raw,echo=0

# Output will show something like:
# PTY is /dev/pts/2
# PTY is /dev/pts/3

# Use /dev/pts/2 for master, /dev/pts/3 for slave
```

### macOS (via com0com alternative)
```bash
# Install python ptyprocess for virtual ports
pip install ptyprocess

# Use Python script to create virtual port pair
# (Script not included here, but similar to socat)
```

### Windows (com0com)
```bash
# Download and install com0com
# Creates pairs like COM10/COM11

# Use in commands:
./aoba.exe --tui  # Use COM10
./aoba.exe modbus slave poll --port COM11 ...
```

## Common CLI Commands Reference

### Slave Poll (acts as Modbus Master/Client)
```bash
# Poll holding registers
./target/debug/aoba modbus slave poll \
    --port /dev/vcom2 \
    --baud-rate 9600 \
    --station-id 1 \
    --register-mode holding \
    --register-address 0 \
    --register-length 4 \
    --interval 1000

# Poll input registers
./target/debug/aoba modbus slave poll \
    --port /dev/vcom2 \
    --station-id 1 \
    --register-mode input \
    --register-address 100 \
    --register-length 2
```

### Master Provide (acts as Modbus Slave/Server)
```bash
# Provide data from stdin
echo "10 20 30 40" | ./target/debug/aoba modbus master provide \
    --port /dev/vcom2 \
    --station-id 1 \
    --register-mode holding \
    --register-address 0 \
    --register-length 4 \
    --data-source stdin

# Provide data from file (persistent mode)
./target/debug/aoba modbus master provide-persist \
    --port /dev/vcom2 \
    --station-id 1 \
    --register-mode holding \
    --register-address 0 \
    --register-length 4 \
    --data-source file:/tmp/data.txt
```

## Debugging Tips

### Check Communication with Logs

**TUI side:**
```bash
# Set log file before starting
export AOBA_LOG_FILE=/tmp/tui.log
./target/debug/aoba --tui

# In another terminal, monitor logs
tail -f /tmp/tui.log | grep -E "(TX|RX|Timeout|Success)"
```

**CLI side:**
```bash
# CLI has built-in verbose output
# Just watch the console output for request/response pairs
```

### Verify Port Connectivity

Before running tests, verify the virtual COM ports are connected:

```bash
# Linux: Check if ports exist
ls -l /dev/vcom* /dev/pts/*

# Test basic connectivity
echo "test" > /dev/vcom1 &
cat /dev/vcom2  # Should see "test"
```

### Common Issues

**Issue: "Port not found"**
- Verify virtual COM ports are created
- Check permissions: `sudo chmod 666 /dev/vcom*`

**Issue: "Timeout on all requests"**
- Check baud rate matches on both sides
- Verify port names are correct pair (vcom1 ↔ vcom2)
- Check if port is already in use: `lsof /dev/vcom1`

**Issue: "Invalid register values"**
- Ensure station ID matches on both sides
- Check register mode (holding vs input)
- Verify register address and length

## Example Test Scripts

### Quick Smoke Test
```bash
#!/bin/bash
# test_tui_master.sh

# Create virtual ports
socat -d -d pty,raw,echo=0,link=/tmp/vcom1 pty,raw,echo=0,link=/tmp/vcom2 &
SOCAT_PID=$!
sleep 1

# Start TUI in background (would need expect script in real scenario)
# For manual testing, start TUI in separate terminal

# Poll from CLI
./target/debug/aoba modbus slave poll \
    --port /tmp/vcom2 \
    --station-id 1 \
    --register-mode holding \
    --register-address 0 \
    --register-length 4 \
    --interval 1000 \
    --count 5  # Poll 5 times and exit

# Cleanup
kill $SOCAT_PID
```

### Full Integration Test
```bash
#!/bin/bash
# test_cli_master_tui_slave.sh

# Setup
socat -d -d pty,raw,echo=0,link=/tmp/vcom1 pty,raw,echo=0,link=/tmp/vcom2 &
SOCAT_PID=$!
sleep 1

# Prepare test data
echo "100 200 300 400" > /tmp/test_data.txt

# Start CLI master in background
./target/debug/aoba modbus master provide-persist \
    --port /tmp/vcom2 \
    --station-id 1 \
    --register-mode holding \
    --register-address 0 \
    --register-length 4 \
    --data-source file:/tmp/test_data.txt &
CLI_PID=$!

# Give it time to start
sleep 2

# Start TUI with logging (manual or expect-based)
export AOBA_LOG_FILE=/tmp/tui_slave.log
# ./target/debug/aoba --tui  # Start manually in another terminal

# Wait and check results
sleep 10

# Verify log shows successful communication
if grep -q "Success response" /tmp/tui_slave.log; then
    echo "✅ Test PASSED: Communication successful"
else
    echo "❌ Test FAILED: No successful responses"
fi

# Cleanup
kill $CLI_PID $SOCAT_PID
rm /tmp/test_data.txt
```

## Recommended Testing Workflow

1. **Start with CLI-only tests** to verify basic Modbus functionality
2. **Add CLI + TUI hybrid tests** to test the TUI implementation
3. **Use full TUI E2E tests** only for critical user workflows
4. **Keep CLI tests in CI** for fast feedback
5. **Run TUI tests periodically** or on-demand for UI validation

This approach provides better test coverage while keeping test maintenance manageable.
