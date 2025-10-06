# Hybrid TUI + CLI Tests - Complete Rewrite

This directory contains hybrid tests that combine TUI and CLI, completely rewritten with step-by-step verification.

## Complete Rewrite Approach

These tests follow a methodical approach requested by @langyo:
- **Step-by-step execution** with verification after each action
- **Regex probes** after every critical operation
- **Screen captures** at each major checkpoint
- **Detailed logging** with emojis for easy scanning
- **Iterative debugging** until tests pass

## Test Structure

### 1. TUI Master + CLI Slave (`tui_master_cli_slave.rs`)

**What it tests:**
- TUI as Modbus Master (Slave/Server) on vcom1
- CLI as Modbus Slave (Master/Client) on vcom2

**Step-by-step flow:**
1. ✓ Spawn TUI, verify "AOBA" title appears
2. ✓ Navigate to vcom1 (with screen capture and cursor detection)
3. ✓ Enter vcom1 details, verify header shows "/dev/vcom1"
4. ✓ Navigate to Modbus Settings
5. ✓ Create station, verify "#1" appears
6. ✓ Set Register Length to 4, verify "0x0004"
7. ✓ Set 4 register values (0, A, 14, 1E), verify each
8. ✓ Exit register editing
9. ✓ Enable port, verify "Enabled" status
10. ✓ Wait for initialization (3 seconds)
11. ✓ Run CLI slave poll command
12. ✓ Verify CLI output contains: 0, 10, 20, 30

**Key features:**
- Screen capture before/after navigation to find cursor position
- Parse screen lines to calculate exact navigation steps
- Verify each register value was set correctly
- CLI output is validated against expected values

### 2. CLI Master + TUI Slave (`cli_master_tui_slave.rs`)

**What it tests:**
- CLI as Modbus Master (Slave/Server) on vcom2
- TUI as Modbus Slave (Master/Client) on vcom1

**Step-by-step flow:**
1. ✓ Create test data file (5, 15, 25, 35)
2. ✓ Start CLI master, verify process is running
3. ✓ Spawn TUI, verify "AOBA" title
4. ✓ Navigate to vcom1 with verification
5. ✓ Enter Modbus Settings
6. ✓ Create station, verify "#1"
7. ✓ Change mode to Slave, verify "Connection Mode Slave"
8. ✓ Set Register Length to 4, verify "0x0004"
9. ✓ Enable port, verify "Enabled"
10. ✓ Wait for communication (7 seconds)
11. ✓ Navigate to Modbus panel
12. ✓ Check display for received values (5, 15, 25, 35)

**Key features:**
- CLI provides reliable test data
- TUI polling verified step by step
- Values checked in TUI display (with warning if not found)

## Running the Tests

### Prerequisites

Create virtual COM port pair:
```bash
# Linux/macOS
socat -d -d pty,raw,echo=0,link=/dev/vcom1 pty,raw,echo=0,link=/dev/vcom2
```

### Run Tests
```bash
cd examples/tui_e2e_tests
cargo run
```

### Verbose Logging
```bash
RUST_LOG=debug cargo run
```

## Debugging Guide

### Screen Captures (📸)
Every major action logs a screen capture:
```
📸 Initial screen:
[screen content here]
```

### Navigation Logs (📍)
Navigation is carefully logged:
```
📍 Finding vcom1 in port list...
  Found vcom1 at line 5
  Current cursor at line 3
  Moving DOWN 2 steps to reach vcom1
  ✓ Cursor is now on vcom1
```

### Verification (✓ / ✗ / ⚠️)
- ✓ = Success
- ✗ = Failure  
- ⚠️ = Warning

### Common Issues

**"vcom1 not found in port list"**
```bash
# Check if ports exist
ls -l /dev/vcom*

# Fix permissions
sudo chmod 666 /dev/vcom*
```

**"Failed to navigate to vcom1"**
- Check screen capture in logs
- Cursor detection looks for lines starting with `>`
- May need to adjust cursor detection logic

**"Pattern not found"**
- Compare expected pattern with actual screen content
- Check regex syntax
- Adjust line_range if needed

**"CLI command failed"**
- Run CLI command manually to test
- Check if port is already in use: `lsof /dev/vcom2`
- Verify baud rate matches

## Code Structure

### Careful Navigation Function
```rust
async fn navigate_to_vcom1_carefully<T: Expect>(...)
```
- Captures screen before navigation
- Finds vcom1 line number
- Finds current cursor line
- Calculates delta and moves precisely
- Verifies cursor is on vcom1 before Enter

### Step-by-Step Configuration
```rust
async fn configure_tui_master_carefully<T: Expect>(...)
```
- Each navigation step verified
- Each value set with confirmation
- Register values checked after setting
- Mode changes verified with regex

### Enable Port with Verification
```rust
async fn enable_port_carefully<T: Expect>(...)
```
- Screen capture before/after
- Regex probe for "Enabled" text
- Detailed logging

## Benefits of This Rewrite

1. **Clear Failure Points**: Know exactly which step failed
2. **Visual Evidence**: Screen captures show what went wrong
3. **Precise Navigation**: Uses screen parsing, not guesswork
4. **Better Logging**: Emojis and structure make logs scannable
5. **Maintainable**: Organized by logical steps
6. **Debuggable**: Can reproduce issues from logs

## Test Philosophy

These tests follow the principle:
> "Test should fail fast and fail clearly"

Every assertion is checked immediately with clear error messages and screen evidence.
