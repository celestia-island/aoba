# Multi-Station Workflow Completion - Technical Guide

## Current Progress
- ✅ **master/mixed_types.toml** - Complete and tested (1529 lines)
- 🔄 **Remaining 5 workflows** - Need Station 2 register editing transformation

## The Core Challenge

The main complexity in generating the remaining workflows is transforming Station 2's register editing logic when changing from **Coils** (toggle) to **Holding** (hex entry).

### Coils Pattern (Current in mixed_types.toml)
```toml
[[workflow.edit_station2_round1]]
description = "Register 0 - Set ON (value=1)"
key = "Enter"  # Toggle the coil

[[workflow.edit_station2_round1]]
description = "Update mock state register 0"
mock_path = "$.ports['/tmp/vcom1'].modbus_masters[1].registers[0]"
mock_set_value = 1

[[workflow.edit_station2_round1]]
description = "Move to register 1"
key = "Right"
```

### Holding Pattern (Needed for mixed_ids, spaced_addresses, slave files)
```toml
[[workflow.edit_station2_round1]]
description = "Register 0 - Enter and type 0x0010"
key = "Enter"  # Enter edit mode

[[workflow.edit_station2_round1]]
input = "hex"
value = 0x0010  # Type the value

[[workflow.edit_station2_round1]]
key = "Enter"  # Confirm

[[workflow.edit_station2_round1]]
description = "Update mock state register 0"
mock_path = "$.ports['/tmp/vcom1'].modbus_masters[1].registers[0]"
mock_set_value = 0x0010

[[workflow.edit_station2_round1]]
key = "Right"  # Move to next
```

## Transformation Algorithm

For each workflow that needs Station 2 changed from Coils to Holding:

1. **Find all** `[[workflow.edit_station2_roundN]]` sections
2. **For each register** (0-9):
   - If current pattern is Coils toggle:
     - Replace with Holding hex entry pattern
     - Use correct hex value from test data
   - Update mock_set_value to match

3. **Update other Station 2 references**:
   - Type selection: Remove `key = "Char(h)", times = 2`, use `key = "Enter"`
   - Register type in mock state: `"Coils"` → `"Holding"`
   - Start address: Update as needed (0x0010 → 0x0000 or 0x0100)

## Test Data Specifications

### master/mixed_ids.toml
```python
# Station A (Holding): ID=1, Address=0x0000
round1_a = [0x0001, 0x0003, 0x0005, 0x0007, 0x0009, 0x000B, 0x000D, 0x000F, 0x0011, 0x0013]
round2_a = [0x0013, 0x0011, 0x000F, 0x000D, 0x000B, 0x0009, 0x0007, 0x0005, 0x0003, 0x0001]
round3_a = [0x0101, 0x0202, 0x0303, 0x0404, 0x0505, 0x0606, 0x0707, 0x0808, 0x0909, 0x0A0A]

# Station B (Holding): ID=2, Address=0x0000
round1_b = [0x0010, 0x0012, 0x0014, 0x0016, 0x0018, 0x001A, 0x001C, 0x001E, 0x0020, 0x0022]
round2_b = [0x0022, 0x0020, 0x001E, 0x001C, 0x001A, 0x0018, 0x0016, 0x0014, 0x0012, 0x0010]
round3_b = [0x0A0A, 0x0B0B, 0x0C0C, 0x0D0D, 0x0E0E, 0x0F0F, 0x1010, 0x1111, 0x1212, 0x1313]
```

### master/spaced_addresses.toml
```python
# Station A (Holding): ID=1, Address=0x0000
round1_a = [0x0001, 0x0002, 0x0003, 0x0004, 0x0005, 0x0006, 0x0007, 0x0008, 0x0009, 0x000A]
round2_a = [0x0010, 0x0020, 0x0030, 0x0040, 0x0050, 0x0060, 0x0070, 0x0080, 0x0090, 0x00A0]
round3_a = [0x1111, 0x2222, 0x3333, 0x4444, 0x5555, 0x6666, 0x7777, 0x8888, 0x9999, 0xAAAA]

# Station B (Holding): ID=2, Address=0x0100
round1_b = [0x00A0, 0x00A1, 0x00A2, 0x00A3, 0x00A4, 0x00A5, 0x00A6, 0x00A7, 0x00A8, 0x00A9]
round2_b = [0x00B0, 0x00B1, 0x00B2, 0x00B3, 0x00B4, 0x00B5, 0x00B6, 0x00B7, 0x00B8, 0x00B9]
round3_b = [0x0C0C, 0x0C0D, 0x0C0E, 0x0C0F, 0x0C10, 0x0C11, 0x0C12, 0x0C13, 0x0C14, 0x0C15]
```

### slave/mixed_types.toml
```python
# Station A (Coils): ID=1, Address=0x0000
round1_a = [1, 1, 1, 1, 1, 1, 1, 1, 1, 1]  # All ON
round2_a = [1, 0, 1, 0, 1, 0, 1, 0, 1, 0]  # Alternating
round3_a = [0, 0, 0, 0, 0, 1, 1, 1, 1, 1]  # Half ON

# Station B (Holding): ID=2, Address=0x0010
round1_b = [0x0A00, 0x0A01, 0x0A02, 0x0A03, 0x0A04, 0x0A05, 0x0A06, 0x0A07, 0x0A08, 0x0A09]
round2_b = [0x0B00, 0x0B02, 0x0B04, 0x0B06, 0x0B08, 0x0B0A, 0x0B0C, 0x0B0E, 0x0B10, 0x0B12]
round3_b = [0x0C10, 0x0C20, 0x0C30, 0x0C40, 0x0C50, 0x0C60, 0x0C70, 0x0C80, 0x0C90, 0x0CA0]
```

### slave/mixed_ids.toml
```python
# Station A (Holding): ID=1, Address=0x0000
round1_a = [0x0011, 0x0022, 0x0033, 0x0044, 0x0055, 0x0066, 0x0077, 0x0088, 0x0099, 0x00AA]
round2_a = [0x1000, 0x1001, 0x1002, 0x1003, 0x1004, 0x1005, 0x1006, 0x1007, 0x1008, 0x1009]
round3_a = [0xAAAA, 0xBBBB, 0xCCCC, 0xDDDD, 0xEEEE, 0xFFFF, 0x0000, 0x1111, 0x2222, 0x3333]

# Station B (Holding): ID=2, Address=0x0000
round1_b = [0x0099, 0x00AA, 0x00BB, 0x00CC, 0x00DD, 0x00EE, 0x00FF, 0x0100, 0x0111, 0x0122]
round2_b = [0x2000, 0x2001, 0x2002, 0x2003, 0x2004, 0x2005, 0x2006, 0x2007, 0x2008, 0x2009]
round3_b = [0x8888, 0x9999, 0xAAAA, 0xBBBB, 0xCCCC, 0xDDDD, 0xEEEE, 0xFFFF, 0x0000, 0x1111]
```

### slave/spaced_addresses.toml
```python
# Station A (Holding): ID=1, Address=0x0000
round1_a = [0x0100, 0x0101, 0x0102, 0x0103, 0x0104, 0x0105, 0x0106, 0x0107, 0x0108, 0x0109]
round2_a = [0x0F00, 0x0F01, 0x0F02, 0x0F03, 0x0F04, 0x0F05, 0x0F06, 0x0F07, 0x0F08, 0x0F09]
round3_a = [0xF000, 0xF111, 0xF222, 0xF333, 0xF444, 0xF555, 0xF666, 0xF777, 0xF888, 0xF999]

# Station B (Holding): ID=2, Address=0x0100
round1_b = [0x0200, 0x0201, 0x0202, 0x0203, 0x0204, 0x0205, 0x0206, 0x0207, 0x0208, 0x0209]
round2_b = [0x1000, 0x1001, 0x1002, 0x1003, 0x1004, 0x1005, 0x1006, 0x1007, 0x1008, 0x1009]
round3_b = [0xF888, 0xF999, 0xFAAA, 0xFBBB, 0xFCCC, 0xFDDD, 0xFEEE, 0xFFFF, 0x0000, 0x1111]
```

## Completion Checklist Per File

- [ ] Update header comments with test ID and 10-register test data
- [ ] Update manifest (ID, description, station configs)
- [ ] Update Station 2 field configuration (type, address)
- [ ] Transform Station 2 Round 1 register editing (if Coils→Holding)
- [ ] Update Station 1 Round 1 register values
- [ ] Update Station 2 Round 1 register values
- [ ] Update Station 1 Round 2 register values
- [ ] Update Station 2 Round 2 register values
- [ ] Update Station 1 Round 3 register values
- [ ] Update Station 2 Round 3 register values
- [ ] For slave mode: Add switch_to_slave_mode step
- [ ] For slave mode: Change modbus_masters → modbus_slaves
- [ ] Test rendering mode
- [ ] Test drill-down mode

## Recommended Approach

1. Use Python script to generate base file with header/manifest updates
2. Manually transform Station 2 editing sections (search/replace with careful review)
3. Use sed/awk for bulk value replacements once structure is correct
4. Test incrementally after each file

## Time Estimate
- With proper tooling: 30-45 min per file
- Manual editing: 2-3 hours per file
- Total for 5 files: 2.5-4 hours (tooling) or 10-15 hours (manual)
