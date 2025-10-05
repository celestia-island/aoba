# TUI E2E Testing Debug Guide

## Using Custom Log File Location

When running TUI or E2E tests, you can specify a custom log file location using the `AOBA_LOG_FILE` environment variable:

```bash
# Set custom log location
export AOBA_LOG_FILE=/tmp/aoba_master.log

# Run TUI
./target/debug/aoba --tui

# Or run E2E tests
cd examples/tui_e2e_tests
cargo run
```

## Analyzing Debug Logs

### Quick Analysis
```bash
# Show last 100 lines
tail -n 100 /tmp/aoba_master.log

# Find all timeouts
grep "Timeout" /tmp/aoba_master.log

# Find successful responses
grep "✅" /tmp/aoba_master.log

# Find all TX (transmitted packets)
grep "TX" /tmp/aoba_master.log

# Find all RX (received packets)
grep "RX" /tmp/aoba_master.log

# Count errors and warnings
grep -c "\[ERROR\]" /tmp/aoba_master.log
grep -c "\[WARN\]" /tmp/aoba_master.log
```

### Using the Built-in Analysis Tool

You can use the log analysis utilities programmatically:

```rust
use aoba::ci::log_utils::{tail_log_file, analyze_log_tail};

// Get last 50 lines
let lines = tail_log_file("/tmp/aoba_master.log", 50)?;

// Get analysis summary
let analysis = analyze_log_tail("/tmp/aoba_master.log", 100)?;
println!("{}", analysis);
```

Output example:
```
=== Last 100 lines of log ===

=== Summary ===
Errors: 2
Warnings: 5
Timeouts: 3
TX (transmitted): 45
RX (received): 42

=== Log entries ===
...
```

## Understanding Modbus Sequential Polling Logs

### Successful Communication Pattern
```
[INFO] 📤 Sent modbus slave request for /dev/ttyS0 station 1 (idx 0): 01 03 00 00 00 01 84 0a
[INFO] Received modbus slave response for /dev/ttyS0: 01 03 02 00 0a b8 44
[INFO] ✅ Success response for station 1 (idx 0), moving to next station idx 1
[INFO] 📤 Sent modbus slave request for /dev/ttyS0 station 2 (idx 1): 02 03 00 00 00 01 85 f8
[INFO] Received modbus slave response for /dev/ttyS0: 02 03 02 00 14 b9 8f
[INFO] ✅ Success response for station 2 (idx 1), moving to next station idx 0
```

### Timeout Pattern
```
[INFO] 📤 Sent modbus slave request for /dev/ttyS0 station 1 (idx 0): 01 03 00 00 00 01 84 0a
[WARN] ⏱️  Request timeout for /dev/ttyS0 station 1 (idx 0), staying on same station to retry
[INFO] 📤 Sent modbus slave request for /dev/ttyS0 station 1 (idx 0): 01 03 00 00 00 01 84 0a
[INFO] Received modbus slave response for /dev/ttyS0: 01 03 02 00 0a b8 44
[INFO] ✅ Success response for station 1 (idx 0), moving to next station idx 1
```

### Master Mode Throttling
```
[INFO] Slave RX (request): 01 03 00 64 00 01 c5 d5
[INFO] Sent modbus master response for /dev/ttyS1: 01 03 02 00 0a b8 44
[DEBUG] Throttling response to station 1 register 100: only 300ms since last response (need 1000ms)
[DEBUG] Skipped response due to 1-second throttling
```

## Common Issues and Solutions

### Issue: Logs show "moving to next station" immediately
**Problem**: Old behavior - stations were polled in parallel
**Solution**: Check if you're using the updated code with sequential polling

### Issue: Station stuck, never moves forward
**Problem**: May be waiting for response that never comes
**Check**: Look for timeout messages in logs
**Solution**: Verify serial connection is working

### Issue: Too many timeouts
**Problem**: 3-second timeout may be too short for slow devices
**Solution**: Consider adjusting timeout in code or check baud rate

### Issue: Master responds too frequently
**Problem**: Throttling not working
**Check**: Verify `last_response_time` is being updated
**Solution**: Check logs for throttling debug messages

## Running E2E Tests with Logging

```bash
# Terminal 1: Run master with logging
export AOBA_LOG_FILE=/tmp/master.log
./target/debug/aoba --tui

# Terminal 2: Run slave with logging  
export AOBA_LOG_FILE=/tmp/slave.log
./target/debug/aoba --tui

# Terminal 3: Monitor logs
tail -f /tmp/master.log /tmp/slave.log

# Or analyze after test
./examples/tui_e2e_tests/target/debug/tui_e2e_tests
cat /tmp/master.log | grep -E "(Timeout|✅|⏱️)" | tail -50
```
