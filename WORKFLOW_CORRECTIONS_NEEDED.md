# Multi-Station Workflow Corrections Required

## Summary

All 6 multi-station workflow files need substantial corrections to match the standardized specifications. This affects approximately 10,000+ lines across all files.

## New Standardized Requirements

### 1. Register Value Patterns (Consistent Across ALL Tests)

**Coils (Boolean registers):**
- Round 1: All ON → `[ON, ON, ON, ON, ON, ON, ON, ON, ON, ON]`
- Round 2: ON/OFF cycle → `[ON, OFF, ON, OFF, ON, OFF, ON, OFF, ON, OFF]`
- Round 3: ON/ON/OFF cycle → `[ON, ON, OFF, ON, ON, OFF, ON, ON, OFF, ON]`

**Holding (Integer registers):**
- Round 1: Count 0-9 → `[0x0000, 0x0001, 0x0002, 0x0003, 0x0004, 0x0005, 0x0006, 0x0007, 0x0008, 0x0009]`
- Round 2: Count 9-0 → `[0x0009, 0x0008, 0x0007, 0x0006, 0x0005, 0x0004, 0x0003, 0x0002, 0x0001, 0x0000]`
- Round 3: Pseudo-random → `[0x1234, 0x5678, 0x9ABC, 0xDEF0, 0x1111, 0x2222, 0x3333, 0x4444, 0x5555, 0x6666]`

### 2. Station Configuration Rules

**mixed_types** - Test variation: Different register types only
- Station A & B: Same ID (1), Same address (0x0000)
- Station A: One type (Holding for master, Coils for slave)
- Station B: Different type (Coils for master, Holding for slave)

**mixed_ids** - Test variation: Different station IDs only
- Station A & B: Same type (Holding), Same address (0x0000)
- Station A: ID=1
- Station B: ID=2

**spaced_addresses** - Test variation: Different addresses only
- Station A & B: Same ID (1), Same type (Holding)
- Station A: Address 0x0000
- Station B: Address 0x0100

## Files Requiring Corrections

### Master Mode

#### 1. `master/mixed_types.toml` (Currently: 1529 lines)

**Header Changes:**
- Station B: ID `2` → `1`
- Station B: Address `0x0010` → `0x0000`
- Test data: Update to standard patterns

**Manifest Changes:**
```toml
# Current:
{ station_id = 2, register_type = "Coils", start_address = 0x0010, register_count = 10 },

# Should be:
{ station_id = 1, register_type = "Coils", start_address = 0x0000, register_count = 10 },
```

**Workflow Changes:**
- Station 2 ID field: Change from "2" to "1"
- Station 2 Address field: Change from "16" (0x0010) to "0" (0x0000)
- All Station A register values (Holding): Update to 0-9, 9-0, pseudo-random patterns
- All Station B register values (Coils): Update to all-ON, ON/OFF, ON/ON/OFF patterns
- All mock state station_id: Change from 2 to 1
- All mock state start_address: Change from 16 to 0

**Estimated edits:** ~150-200 changes

#### 2. `master/mixed_ids.toml` (Currently: 1764 lines)

**Header Changes:**
- Test data: Update to standard patterns (both stations use Holding)

**Configuration:** Already correct (IDs 1 and 2, both at 0x0000)

**Workflow Changes:**
- All Station A register values: Update to 0-9, 9-0, pseudo-random patterns
- All Station B register values: Update to 0-9, 9-0, pseudo-random patterns

**Estimated edits:** ~120 changes

#### 3. `master/spaced_addresses.toml` (Currently: 1764 lines)

**Header Changes:**
- Station B: ID `2` → `1`
- Test data: Update to standard patterns (both stations use Holding)

**Manifest Changes:**
```toml
# Current:
{ station_id = 2, register_type = "Holding", start_address = 0x0100, register_count = 10 },

# Should be:
{ station_id = 1, register_type = "Holding", start_address = 0x0100, register_count = 10 },
```

**Workflow Changes:**
- Station 2 ID field: Change from "2" to "1"
- All Station A register values: Update to 0-9, 9-0, pseudo-random patterns
- All Station B register values: Update to 0-9, 9-0, pseudo-random patterns
- All mock state station_id: Change from 2 to 1

**Estimated edits:** ~130 changes

### Slave Mode

#### 4. `slave/mixed_types.toml` (Currently: 1816 lines)

**Header Changes:**
- Station B: ID `2` → `1`
- Station B: Address `0x0010` → `0x0000`
- Test data: Update to standard patterns

**Manifest Changes:**
```toml
# Current:
{ station_id = 2, register_type = "Holding", start_address = 0x0010, register_count = 10 },

# Should be:
{ station_id = 1, register_type = "Holding", start_address = 0x0000, register_count = 10 },
```

**Workflow Changes:**
- Station 2 ID field: Change from "2" to "1"
- Station 2 Address field: Change from "16" (0x0010) to "0" (0x0000)
- All Station A register values (Coils): Update to all-ON, ON/OFF, ON/ON/OFF patterns
- All Station B register values (Holding): Update to 0-9, 9-0, pseudo-random patterns
- All mock state station_id: Change from 2 to 1
- All mock state start_address: Change from 16 to 0

**Estimated edits:** ~150-200 changes

#### 5. `slave/mixed_ids.toml` (Currently: 1787 lines)

**Header Changes:**
- Test data: Update to standard patterns (both stations use Holding)

**Configuration:** Already correct (IDs 1 and 2, both at 0x0000)

**Workflow Changes:**
- All Station A register values: Update to 0-9, 9-0, pseudo-random patterns
- All Station B register values: Update to 0-9, 9-0, pseudo-random patterns

**Estimated edits:** ~120 changes

#### 6. `slave/spaced_addresses.toml` (Currently: 1787 lines)

**Header Changes:**
- Station B: ID `2` → `1`
- Test data: Update to standard patterns (both stations use Holding)

**Manifest Changes:**
```toml
# Current:
{ station_id = 2, register_type = "Holding", start_address = 0x0100, register_count = 10 },

# Should be:
{ station_id = 1, register_type = "Holding", start_address = 0x0100, register_count = 10 },
```

**Workflow Changes:**
- Station 2 ID field: Change from "2" to "1"
- All Station A register values: Update to 0-9, 9-0, pseudo-random patterns
- All Station B register values: Update to 0-9, 9-0, pseudo-random patterns
- All mock state station_id: Change from 2 to 1

**Estimated edits:** ~130 changes

## Total Scope

- **Files affected:** 6 workflow files
- **Total lines:** ~10,000+ lines
- **Estimated total edits:** ~800-1000 individual changes
- **Types of changes:**
  - Header comments (test data documentation)
  - Manifest entries (station configurations)
  - Station ID field edits in workflows
  - Station address field edits in workflows
  - Register value edits across all 3 rounds for both stations
  - Mock state updates (station IDs, addresses, register values)

## Approach Options

### Option 1: Manual Editing (10-15 hours)
Systematically go through each file and make corrections section by section.

### Option 2: Automated Script (2-3 hours development + 30 min execution)
Create a Python script that:
1. Parses each TOML file
2. Identifies sections needing changes
3. Applies transformations systematically
4. Validates output

### Option 3: Hybrid Approach (4-6 hours)
Use scripts for bulk transformations (register values, mock states), manual editing for configuration fields.

## Recommendation

Given the scope and repetitive nature of the changes, **Option 2 (Automated Script)** is recommended to ensure consistency and reduce errors.

## Next Steps

1. Confirm approach with stakeholders
2. Develop/execute correction methodology
3. Test each corrected file in rendering mode
4. Verify all 6 workflows pass tests
5. Proceed with drill-down mode testing

## Note on Testing

After corrections:
- All 6 workflows must pass rendering mode tests
- Drill-down mode testing can proceed once rendering tests pass
- Multi-station slave mode may require CLI configuration file approach (as previously noted)
