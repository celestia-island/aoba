# TUI E2E Test Rewrite Plan

## Overview
This document outlines the detailed plan for rewriting hybrid TUI+CLI tests with step-by-step verification and regex probes after each action.

## TUI Interface Structure Analysis

### 1. Entry Page (Port List)
Based on source code analysis (`src/tui/ui/pages/entry/`):

**Layout:**
```
AOBA > Port List
┌─────────────────────────────────────────────────────────┐
│ > /tmp/vcom1        [Status]                            │
│   /tmp/vcom2        [Status]                            │
│   [Other ports...]                                      │
│                                                          │
│   Refresh                                                │
│   Manual Specify                                         │
│   About                                                  │
└─────────────────────────────────────────────────────────┘
Press ↑/k or ↓/j to move    Press Enter to select
```

**Key Points:**
- Cursor indicated by `>` character at start of line
- Ports listed first (order may vary: vcom1, vcom2, or vice versa)
- 3 special items at bottom: Refresh, Manual Specify, About
- Status shown after port name (e.g., "Closed", "Opened")

**Navigation Strategy:**
1. Capture full screen
2. Parse lines to find line containing "/tmp/vcom1"
3. Parse lines to find line with `>` (current cursor position)
4. Calculate delta: `vcom1_line - cursor_line`
5. Send Up/Down arrows for exact delta
6. Verify `>` is now on vcom1 line
7. Send Enter

### 2. Port Details Page
After pressing Enter on a port:

**Layout:**
```
AOBA > /tmp/vcom1
┌─────────────────────────────────────────────────────────┐
│ > Port Name         /tmp/vcom1                          │
│   Baud Rate         9600                                │
│   Data Bits         8                                   │
│   Stop Bits         1                                   │
│   Parity            None                                │
│   ModBus Settings                                        │
│   ModBus Log                                            │
│   Enable                                                │
└─────────────────────────────────────────────────────────┘
Press ↑/k or ↓/j to move    Press Enter to edit/select
Press Esc to go back
```

**Key Points:**
- Title shows "AOBA > /tmp/vcom1"
- Fields listed vertically
- "ModBus Settings" is typically around line 6-7
- "Enable" is at the bottom

**Navigation to Modbus Settings:**
1. After entering port, capture screen
2. Verify title contains "/tmp/vcom1"
3. Find line containing "ModBus Settings"
4. Find current cursor line (`>`)
5. Calculate delta
6. Navigate precisely
7. Verify cursor on "ModBus Settings"
8. Send Enter

### 3. Modbus Settings Page

**Layout (Master mode):**
```
AOBA > /tmp/vcom1 > ModBus Master/Slave Settings
┌─────────────────────────────────────────────────────────┐
│ > Create Station                                        │
│   Connection Mode       Master                          │
│ ─────────────────────────────────────────────────────── │
│ #1 - ID: 1                                              │
│   Station ID            0x01 (1)                        │
│   Register Type         Holding Registers(03)           │
│   Start Address         0x0000 (0)                      │
│   Register Length       0x0004 (4)                      │
│     0x0000              0x0000 0x0000 0x0000 0x0000     │
│   Delete Station                                        │
└─────────────────────────────────────────────────────────┘
```

**Key Points:**
- "Create Station" at top
- "Connection Mode" shows current mode (Master/Slave)
- After creating station, "#1 - ID: 1" appears
- Station fields: Station ID, Register Type, Start Address, Register Length
- Register values displayed in rows of 4
- "Delete Station" at bottom of each station

**Creating Station:**
1. Verify "Create Station" visible
2. Press Enter on "Create Station"
3. Wait 500ms
4. Capture screen
5. Verify "#1" appears (new station created)

**Setting Register Length:**
1. Find "Register Length" line
2. Navigate to it
3. Press Enter to edit
4. Type hex value (e.g., "4" for 4 registers)
5. Press Enter to confirm
6. Verify "0x0004 (4)" appears

**Setting Register Values:**
1. Find register value line (starts with "0x0000" address)
2. Navigate to it
3. Press Enter to edit
4. Type first value in hex (e.g., "0")
5. Press Tab to next field
6. Type second value (e.g., "A" for 10)
7. Press Tab
8. Type third value (e.g., "14" for 20)
9. Press Tab  
10. Type fourth value (e.g., "1E" for 30)
11. Press Enter to confirm
12. Verify values appear: "0x0000 0x000A 0x0014 0x001E"

**Changing Connection Mode:**
1. Find "Connection Mode" line
2. Navigate to it
3. Press Enter to toggle
4. Verify mode changes (Master ↔ Slave)

**Enabling Port:**
1. Press Esc to go back to port details
2. Find "Enable" line at bottom
3. Navigate to it
4. Press Enter
5. Verify port status changes to "Enabled"

## Test 1: TUI Master + CLI Slave

### Architecture
```
TUI (vcom1) = Master (provides data)
  ↓ responds to requests
CLI (vcom2) = Slave (polls data)
```

### Detailed Steps

**Step 1: Setup Virtual COM Ports**
- Action: `socat -d -d pty,raw,echo=0,link=/tmp/vcom1 pty,raw,echo=0,link=/tmp/vcom2`
- Wait: 2 seconds
- Verification: Check `/tmp/vcom1` and `/tmp/vcom2` exist

**Step 2: Launch TUI**
- Action: `cargo run --release -- --tui`
- Wait: 3 seconds
- Capture: Screen
- Regex: `AOBA.*Port List` or `/tmp/vcom` present

**Step 3: Navigate to vcom1**
- Capture: Screen before navigation
- Parse: Find "/tmp/vcom1" line number
- Parse: Find line with `>` (cursor)
- Calculate: delta = vcom1_line - cursor_line
- Navigate: Send delta Down/Up arrows
- Capture: Screen after navigation
- Regex: `>\s*/tmp/vcom1` (cursor on vcom1)

**Step 4: Enter vcom1 Details**
- Action: Press Enter
- Wait: 500ms
- Capture: Screen
- Regex: `AOBA\s*>\s*/tmp/vcom1` (title changed)

**Step 5: Navigate to Modbus Settings**
- Parse: Find "ModBus Settings" line
- Parse: Find cursor line
- Calculate: delta
- Navigate: Send delta arrows
- Capture: Screen
- Regex: `>\s*ModBus Settings`

**Step 6: Enter Modbus Settings**
- Action: Press Enter
- Wait: 500ms
- Capture: Screen
- Regex: `ModBus Master/Slave Settings` (in title)

**Step 7: Create Station**
- Verify: "Create Station" visible
- Navigate: To "Create Station" if needed
- Action: Press Enter
- Wait: 500ms
- Capture: Screen
- Regex: `#1` (station created)

**Step 8: Set Register Length to 4**
- Parse: Find "Register Length" line
- Navigate: To that line
- Action: Press Enter to edit
- Type: "4"
- Action: Press Enter to confirm
- Wait: 300ms
- Capture: Screen
- Regex: `Register Length.*0x0004.*\(4\)`

**Step 9: Set Register Values**
- Parse: Find register value line (starts with "0x0000")
- Navigate: To that line
- Action: Press Enter to edit
- Type: "0\t" (0 then Tab)
- Type: "A\t" (10 in hex then Tab)
- Type: "14\t" (20 in hex then Tab)
- Type: "1E" (30 in hex)
- Action: Press Enter to confirm
- Wait: 500ms
- Capture: Screen
- Regex: `0x0000.*0x000A.*0x0014.*0x001E` (values appear)

**Step 10: Enable Port**
- Action: Press Esc to go back to port details
- Wait: 300ms
- Capture: Screen
- Regex: `AOBA\s*>\s*/tmp/vcom1` (back at port details)
- Parse: Find "Enable" line
- Navigate: To that line
- Action: Press Enter
- Wait: 500ms
- Capture: Screen
- Regex: `Enabled` or status indicator

**Step 11: Run CLI Slave Poll**
- Action: Spawn CLI process
- Command: `cargo run --release -- modbus slave poll --port /tmp/vcom2 --station-id 1 --register-address 0 --register-length 4 --register-mode holding --baud-rate 9600`
- Wait: 3 seconds for communication
- Capture: CLI stdout
- Regex: Parse JSON output for values
- Verify: Contains values 0, 10, 20, 30 (in decimal)

## Test 2: CLI Master + TUI Slave

### Architecture
```
CLI (vcom2) = Master (provides data via provide-persist)
  ↓ pushes data continuously
TUI (vcom1) = Slave (polls data)
```

### Detailed Steps

**Step 1: Setup Virtual COM Ports**
- Same as Test 1

**Step 2: Create Test Data File**
- Action: Write JSON file `/tmp/test_data.jsonl`
- Content: `{"values": [0, 10, 20, 30]}`
- Verification: File exists

**Step 3: Start CLI Master Provide-Persist**
- Action: Spawn CLI process in background
- Command: `cargo run --release -- modbus master provide-persist --port /tmp/vcom2 --station-id 1 --register-address 0 --register-length 4 --register-mode holding --baud-rate 9600 --data-source file:/tmp/test_data.jsonl`
- Wait: 2 seconds for startup
- Verification: Process running (no immediate error)

**Step 4: Launch TUI**
- Same as Test 1 Step 2

**Step 5: Navigate to vcom1**
- Same as Test 1 Step 3

**Step 6: Enter vcom1 Details**
- Same as Test 1 Step 4

**Step 7: Navigate to Modbus Settings**
- Same as Test 1 Step 5

**Step 8: Enter Modbus Settings**
- Same as Test 1 Step 6

**Step 9: Create Station**
- Same as Test 1 Step 7

**Step 10: Change Connection Mode to Slave**
- Parse: Find "Connection Mode" line
- Navigate: To that line
- Action: Press Enter to toggle
- Wait: 300ms
- Capture: Screen
- Regex: `Connection Mode.*Slave` (mode changed)

**Step 11: Set Register Length to 4**
- Same as Test 1 Step 8

**Step 12: Enable Port**
- Same as Test 1 Step 10

**Step 13: Wait for Data Reception**
- Wait: 7 seconds (allow communication)
- Note: TUI Slave polls CLI Master, receives values

**Step 14: Navigate to Modbus Panel to Check Values**
- Action: Press Esc to go to port list
- Wait: 300ms
- Navigate: Back to vcom1
- Action: Press Enter
- Wait: 300ms
- Navigate: To "ModBus Settings" or "ModBus Log"
- Action: Press Enter
- Wait: 500ms
- Capture: Screen
- Regex: Look for values 0x000A, 0x0014, 0x001E
- Note: If not found, log warning (communication may take time)

## Implementation Guidelines

### Screen Capture Function
```rust
fn capture_screen(session: &mut impl Expect, label: &str) -> Result<String> {
    // Read all available output
    let mut buffer = Vec::new();
    loop {
        match session.try_read(Duration::from_millis(100)) {
            Ok(chunk) => buffer.extend_from_slice(&chunk),
            Err(_) => break,
        }
    }
    
    let output = String::from_utf8_lossy(&buffer).to_string();
    log::info!("📸 Screen capture: {}", label);
    log::info!("{}", output);
    
    Ok(output)
}
```

### Navigation Function
```rust
fn navigate_to_line_containing(
    session: &mut impl Expect,
    target_text: &str,
    step_label: &str,
) -> Result<()> {
    log::info!("📍 Navigation: Finding '{}'", target_text);
    
    // Capture current screen
    let screen = capture_screen(session, &format!("{} - before navigation", step_label))?;
    
    // Parse to find target line and cursor line
    let lines: Vec<&str> = screen.lines().collect();
    let mut target_line_num = None;
    let mut cursor_line_num = None;
    
    for (i, line) in lines.iter().enumerate() {
        if line.contains(target_text) {
            target_line_num = Some(i);
        }
        if line.trim_start().starts_with('>') {
            cursor_line_num = Some(i);
        }
    }
    
    let target = target_line_num.ok_or_else(|| anyhow!("Target '{}' not found", target_text))?;
    let cursor = cursor_line_num.ok_or_else(|| anyhow!("Cursor not found"))?;
    
    log::info!("  Target at line {}, cursor at line {}", target, cursor);
    
    // Calculate and execute navigation
    let delta = target as i32 - cursor as i32;
    if delta > 0 {
        log::info!("  Moving DOWN {} steps", delta);
        for _ in 0..delta {
            session.send("\x1b[B")?; // Down arrow
            std::thread::sleep(Duration::from_millis(100));
        }
    } else if delta < 0 {
        log::info!("  Moving UP {} steps", -delta);
        for _ in 0..-delta {
            session.send("\x1b[A")?; // Up arrow
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    
    // Verify navigation
    std::thread::sleep(Duration::from_millis(200));
    let screen_after = capture_screen(session, &format!("{} - after navigation", step_label))?;
    
    let lines_after: Vec<&str> = screen_after.lines().collect();
    let mut found_cursor_on_target = false;
    for line in &lines_after {
        if line.contains(target_text) && line.trim_start().starts_with('>') {
            found_cursor_on_target = true;
            break;
        }
    }
    
    if !found_cursor_on_target {
        return Err(anyhow!("Navigation failed: cursor not on '{}'", target_text));
    }
    
    log::info!("  ✓ Cursor successfully positioned on '{}'", target_text);
    Ok(())
}
```

### Regex Verification Function
```rust
fn verify_pattern(
    screen: &str,
    pattern: &str,
    step_label: &str,
) -> Result<()> {
    use regex::Regex;
    
    let re = Regex::new(pattern)?;
    if re.is_match(screen) {
        log::info!("🔍 Verification PASSED: '{}' found in {}", pattern, step_label);
        Ok(())
    } else {
        log::error!("✗ Verification FAILED: '{}' not found in {}", pattern, step_label);
        log::error!("Screen content:\n{}", screen);
        Err(anyhow!("Pattern '{}' not found", pattern))
    }
}
```

## Testing Strategy

1. **Incremental Development**: Implement one step at a time
2. **Verify Each Step**: Run test after each step addition
3. **Log Everything**: Capture screens and log all actions
4. **Fail Fast**: Stop immediately on verification failure
5. **Iterate**: Adjust navigation/timing based on CI feedback

## Success Criteria

- Test 1: CLI successfully receives all 4 values (0, 10, 20, 30)
- Test 2: TUI displays received values in Modbus panel
- Both tests: No crashes, clear error messages on failure
- CI: Tests pass consistently across multiple runs
