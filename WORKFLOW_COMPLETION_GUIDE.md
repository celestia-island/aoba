# Multi-Station Workflow Completion Guide

## Status: 1 of 6 Complete ✅

### Completed
- ✅ master/mixed_types.toml (~1550 lines) - **Both drill-down and rendering modes passing**
  - Updated to 10 registers per station
  - Station IDs: 1 and 2 (standardized)

### Remaining (5 files)
- [ ] master/mixed_ids.toml
- [ ] master/spaced_addresses.toml
- [ ] slave/mixed_types.toml
- [ ] slave/mixed_ids.toml
- [ ] slave/spaced_addresses.toml

## **IMPORTANT: Standardized Specifications**

All multi-station tests now follow these standards:
1. **Register count**: ALL stations have **10 registers** (not 8)
2. **Mixed ID tests**: Use station IDs **1 and 2** (not 2 and 6, not 1 and 3)
3. **Mixed address tests**: Use addresses **0x0000 and 0x0100** (decimal 256, not 0x00A0)

## Template File
**File:** `examples/tui_e2e/workflow/multi_station/master/mixed_types.toml`  
**Size:** ~1550 lines (updated from 1362)  
**Status:** Complete and tested ✅

## Completion Approach

### Option 1: Manual Editing (2-3 hours per file)
Copy mixed_types.toml and modify specific sections:

1. **Header (lines 1-13)**: Update test ID, station configs, data values (10 registers each)
2. **Manifest (lines 15-23)**: Update id, description, station configs (register_count = 10)
3. **init_order (lines 25-36)**: Add "switch_to_slave_mode" for slave workflows
4. **Station 1 config**: Update ID, type, address values, register_count = 10
5. **Station 1 round 1**: Update 10 register values
6. **Station 2 config**: Update ID, type, address values, register_count = 10
7. **Station 2 round 1**: Update 10 register values (note: Coils use Enter toggle, Holding uses hex entry)
8. **Station 1 round 2**: Update 10 register values
9. **Station 2 round 2**: Update 10 register values
10. **Station 1 round 3**: Update 10 register values
11. **Station 2 round 3**: Update 10 register values

### Option 2: Automated Script (30 min setup + 5 min per file)
Create Python script to:
1. Read mixed_types.toml as template
2. Apply regex substitutions for IDs, types, addresses
3. Replace register values based on test data specs
4. Generate output file

## Workflow Specifications

### master/mixed_ids.toml
**Purpose**: Test two stations with different IDs on identical address ranges  
**Changes from mixed_types:**
- Station A: ID=1 (same), Type=Holding (same), Address=0x0000 (same), **10 registers**
- Station B: ID 2→**2** (same now), Type Coils→**Holding**, Address 0x0010→**0x0000**, **10 registers**
- Register editing: Station B changes from coil toggles to hex value entry
- Test data: 10 registers × 3 rounds per station (see file header)
- Round 1 values: See file header comments
- Round 2 values: See file header comments  
- Round 3 values: See file header comments

**Critical Changes:**
1. Station 2 ID: Already 2 (no change needed)
2. Station 2 type selection: Remove `key = "Char(h)"`, `times = 2` (Coils→Holding uses default Enter)
3. Station 2 address: "0x0010" → "0x0000", mock_set_value: 16 → 0
4. Station 2 register editing: Replace all coil toggles with hex value entry pattern (Enter, input hex, Enter, Right)
5. Update all register arrays from 8 to 10 elements
6. Add register 8 and 9 editing for all rounds

### master/spaced_addresses.toml
**Purpose**: Test two stations with separated address ranges  
**Changes from mixed_types:**
- Station A: ID=1 (same), Type=Holding (same), Address=0x0000 (same), **10 registers**
- Station B: ID 2 (same), Type Coils→**Holding**, Address 0x0010→**0x0100** (decimal 256), **10 registers**
- Register editing: Station B changes from coil toggles to hex value entry
- Test data: 10 registers × 3 rounds per station

**Critical Changes:**
1. Station 2 type: Coils→Holding
2. Station 2 address: "0x0010" → "0x0100", mock_set_value: 16 → 256
3. Station 2 register editing: Coil toggles → hex value entry
4. Update all register arrays from 8 to 10 elements
5. Add register 8 and 9 editing for all rounds

### slave/mixed_types.toml
**Purpose**: Test slave mode with different register types  
**Changes from mixed_types:**
- Add "switch_to_slave_mode" step in init_order
- Station A: ID=1 (same), Type Holding→**Coils**, Address=0x0000 (same), **10 registers**
- Station B: ID 2 (same), Type Coils→**Holding**, Address=0x0010 (same), **10 registers**
- Register editing: Station A changes to coil toggles, Station B to hex entry
- Test data: 10 registers × 3 rounds per station

**Critical Changes:**
1. Add switch_to_slave_mode step
2. Change mock_path from "modbus_masters" to "modbus_slaves"
3. Station A: Type Holding→Coils (use `key = "Char(h)"` with `times = 2` for type selection)
4. Station A register editing: Hex values → coil toggles
5. Station B: Type Coils→Holding
6. Station B register editing: Coil toggles → hex values
7. Update all register arrays from 8 to 10 elements
8. Add register 8 and 9 editing for all rounds

### slave/mixed_ids.toml
**Purpose**: Test slave mode with different IDs on identical addresses  
**Changes from mixed_types:**
- Add "switch_to_slave_mode" step
- Station A: ID=1 (same), Type=Holding (same), Address=0x0000 (same), **10 registers**
- Station B: ID=2 (same), Type Coils→**Holding**, Address 0x0010→**0x0000**, **10 registers**
- Both stations use hex value entry
- Test data: 10 registers × 3 rounds per station

**Critical Changes:**
1. Add switch_to_slave_mode step
2. Change mock_path from "modbus_masters" to "modbus_slaves"
3. Station B: Type Coils→Holding, Address 0x0010→0x0000
4. Station B register editing: Coil toggles → hex value entry
5. Update all register arrays from 8 to 10 elements
6. Add register 8 and 9 editing for all rounds

### slave/spaced_addresses.toml
**Purpose**: Test slave mode with separated address ranges  
**Changes from mixed_types:**
- Add "switch_to_slave_mode" step  
- Station A: ID=1 (same), Type=Holding (same), Address=0x0000 (same), **10 registers**
- Station B: ID=2 (same), Type Coils→**Holding**, Address 0x0010→**0x0100** (decimal 256), **10 registers**
- Both stations use hex value entry
- Test data: 10 registers × 3 rounds per station

**Critical Changes:**
1. Add switch_to_slave_mode step
2. Change mock_path from "modbus_masters" to "modbus_slaves"
3. Station B: Type Coils→Holding, Address 0x0010→0x0100 (256)
4. Station B register editing: Coil toggles → hex value entry
5. Update all register arrays from 8 to 10 elements
6. Add register 8 and 9 editing for all rounds

## Register Editing Patterns

### Holding/Input Registers (Hex Values)
```toml
[[workflow.edit_stationN_roundM]]
description = "Register X - Enter and type 0xVALUE"
key = "Enter"

[[workflow.edit_stationN_roundM]]
input = "hex"
value = 0xVALUE

[[workflow.edit_stationN_roundM]]
key = "Enter"

[[workflow.edit_stationN_roundM]]
description = "Update mock state register X"
mock_path = "$.ports['/tmp/vcom1'].modbus_masters[N].registers[X]"
mock_set_value = VALUE

[[workflow.edit_stationN_roundM]]
key = "Right"
```

### Coils (Boolean Toggle)
```toml
# Set ON (toggle if currently OFF)
[[workflow.edit_stationN_roundM]]
description = "Register X - Set ON (value=1)"
key = "Enter"

[[workflow.edit_stationN_roundM]]
description = "Update mock state register X"
mock_path = "$.ports['/tmp/vcom1'].modbus_masters[N].registers[X]"
mock_set_value = 1

[[workflow.edit_stationN_roundM]]
key = "Right"

# Skip if already at desired state
[[workflow.edit_stationN_roundM]]
description = "Register Y - Skip (already OFF=0)"
key = "Right"
```

## switch_to_slave_mode Step (Slave Workflows Only)

Insert after `enter_modbus_panel` step:

```toml
# Step: Switch to Slave Mode
[[workflow.switch_to_slave_mode]]
description = "Navigate to Connection Mode field"
key = "Down"

[[workflow.switch_to_slave_mode]]
description = "Enter edit mode for Connection Mode"
key = "Enter"

[[workflow.switch_to_slave_mode]]
description = "Switch to Slave mode"
key = "Left"

[[workflow.switch_to_slave_mode]]
description = "Confirm Connection Mode"
key = "Enter"

[[workflow.switch_to_slave_mode]]
description = "Update connection mode in mock state"
mock_path = "$.page_state.modbus_dashboard.connection_mode"
mock_set_value = "slave"

[[workflow.switch_to_slave_mode]]
description = "Return to AddLine cursor"
key = "Ctrl+PageUp"
```

## Testing

After completing each file, test with:

```bash
# Clean up processes
pkill -9 aoba; pkill -9 tui_e2e; sleep 2

# Initialize virtual ports
./scripts/socat_init.sh

# Test drill-down mode
timeout 120 cargo run --package tui_e2e -- --module [MODULE_NAME]

# Test rendering mode
timeout 60 cargo run --package tui_e2e -- --screen-capture-only --module [MODULE_NAME]
```

## Validation Checklist

For each completed workflow:
- [ ] Header comments match test data
- [ ] Manifest ID and description correct
- [ ] Station configurations (ID, type, address) correct
- [ ] init_order includes switch_to_slave_mode for slave workflows
- [ ] Register type selection logic correct (Enter for Holding, Char(h) x2 for Coils, etc.)
- [ ] All register values updated for 3 rounds
- [ ] mock_path uses "modbus_masters" for master, "modbus_slaves" for slave
- [ ] Drill-down mode test passes
- [ ] Rendering mode test passes

## Completion Time Estimate

- Manual per file: 2-3 hours × 5 = 10-15 hours
- Automated script: 2 hours setup + 5 min/file × 5 = 2.5 hours
- Testing per file: 10 minutes × 5 = 50 minutes

**Total:** 11-16 hours manual OR 3-4 hours automated
