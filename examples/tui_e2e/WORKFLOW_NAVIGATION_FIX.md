# Remaining TUI E2E Workflow Keyboard Navigation Fixes

## Summary
5/14 workflows fixed. 9 workflows remain using the same patterns.

## Pattern for Single-Station Workflows

Replace the "# Step 6: Configure Station Fields" section with:

```toml
# Step 6: Configure Station Fields  
# After creating station, cursor is on StationId automatically
[[workflow.configure_station_fields]]
description = "Highlight Station ID field (already there after creation)"
mock_path = "$.page_state.modbus_dashboard.cursor"
mock_set_value = { kind = "StationId", station_index = 0 }

[[workflow.configure_station_fields]]
description = "Enter edit mode for Station ID"
key = "Enter"

# ... (clear, type, confirm Station ID) ...

[[workflow.configure_station_fields]]
description = "Confirm Station ID"
key = "Enter"

[[workflow.configure_station_fields]]
description = "Update Station ID in mock state"
mock_path = "$.ports['/tmp/vcom1'].{MASTER_OR_SLAVE}[0].station_id"
mock_set_value = 1

# Navigate to Register Type using absolute positioning
[[workflow.configure_station_fields]]
description = "Return to top with Ctrl+PageUp"
key = "Ctrl+PageUp"

[[workflow.configure_station_fields]]
description = "Jump to station with PageDown (AddLine -> ModbusMode)"
key = "PageDown"

[[workflow.configure_station_fields]]
description = "Jump to station with PageDown (ModbusMode -> Station#1 StationId)"
key = "PageDown"

[[workflow.configure_station_fields]]
description = "Move to Register Type field (StationId -> RegisterType)"
key = "Down"

[[workflow.configure_station_fields]]
description = "Enter edit mode for Register Type"
key = "Enter"

# Register type cycling (varies by type):
# Coils: 'h' x 2 (Holding → DiscreteInputs → Coils)
# DiscreteInputs: 'h' x 1 (Holding → DiscreteInputs)
# Holding: Just Enter (default)
# Input: 'h' x 3 (Holding → DiscreteInputs → Coils → Input)

[[workflow.configure_station_fields]]
description = "Confirm Register Type"
key = "Enter"

# Navigate to Start Address
[[workflow.configure_station_fields]]
description = "Return to top with Ctrl+PageUp"
key = "Ctrl+PageUp"

[[workflow.configure_station_fields]]
description = "Jump to station with PageDown (AddLine -> ModbusMode)"
key = "PageDown"

[[workflow.configure_station_fields]]
description = "Jump to station with PageDown (ModbusMode -> Station#1 StationId)"
key = "PageDown"

[[workflow.configure_station_fields]]
description = "Move to Register Type field"
key = "Down"

[[workflow.configure_station_fields]]
description = "Move to Start Address field"
key = "Down"

# ... (edit Start Address) ...

# Navigate to Register Count
[[workflow.configure_station_fields]]
description = "Return to top with Ctrl+PageUp"
key = "Ctrl+PageUp"

[[workflow.configure_station_fields]]
description = "Jump to station with PageDown (AddLine -> ModbusMode)"
key = "PageDown"

[[workflow.configure_station_fields]]
description = "Jump to station with PageDown (ModbusMode -> Station#1 StationId)"
key = "PageDown"

[[workflow.configure_station_fields]]
description = "Move to Register Type field"
key = "Down"

[[workflow.configure_station_fields]]
description = "Move to Start Address field"
key = "Down"

[[workflow.configure_station_fields]]
description = "Move to Register Count field"
key = "Down"

# ... (edit Register Count) ...
```

## Files to Fix

### Single-Station Slave (use modbus_slaves in paths):
1. examples/tui_e2e/workflow/single_station/slave/discrete_inputs.toml (h x1)
2. examples/tui_e2e/workflow/single_station/slave/holding.toml (Enter only)
3. examples/tui_e2e/workflow/single_station/slave/input.toml (h x3)

### Multi-Station (2 stations each):
For Station#2, use PageDown x3 instead of x2:
- Ctrl+PageUp
- PageDown (to ModbusMode)
- PageDown (to Station#1)
- PageDown (to Station#2)

Files:
4. examples/tui_e2e/workflow/multi_station/master/mixed_types.toml
5. examples/tui_e2e/workflow/multi_station/master/spaced_addresses.toml
6. examples/tui_e2e/workflow/multi_station/master/mixed_ids.toml
7. examples/tui_e2e/workflow/multi_station/slave/mixed_types.toml
8. examples/tui_e2e/workflow/multi_station/slave/spaced_addresses.toml
9. examples/tui_e2e/workflow/multi_station/slave/mixed_ids.toml

## Validation

After each fix, run:
```bash
cargo run --package tui_e2e -- --module <module_name> --screen-capture-only
```

All should pass in ScreenCaptureOnly mode.
