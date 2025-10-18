# Config Structure Redesign - Implementation Guide

## Overview

This document describes the station-based configuration redesign implemented to support multiple Modbus masters and slaves operating on the same port with IPC communication.

## Architecture Changes

### Old Structure
```rust
pub struct Config {
    pub port_name: String,
    pub baud_rate: u32,
    pub communication_mode: CommunicationMode,  // Port-wide mode
    pub modbus_configs: Vec<ModbusRegister>,    // Flat list
}
```

### New Structure
```rust
pub struct Config {
    pub port_name: String,
    pub baud_rate: u32,
    pub communication_params: CommunicationParams,
    pub stations: Vec<StationConfig>,  // Station-based
}

pub struct StationConfig {
    pub id: u8,                    // Station ID (1-247)
    pub mode: StationMode,         // Per-station mode
    pub map: RegisterMap,          // Organized by register type
}

pub struct RegisterMap {
    pub coils: Vec<RegisterRange>,
    pub discrete_inputs: Vec<RegisterRange>,
    pub holding: Vec<RegisterRange>,
    pub input: Vec<RegisterRange>,
}
```

## Key Benefits

1. **Per-Station Modes**: Each station can be configured as Master or Slave independently
2. **Organized Registers**: Register ranges grouped by type within each station
3. **Multiple Stations**: Multiple stations can operate on the same port via IPC
4. **Full Synchronization**: Entire configuration sent via IPC ensures consistency

## Implementation Files

### Core Configuration
- `src/cli/config.rs` - New Config structures with StationMode and RegisterMap
- `src/cli/config_convert.rs` - Conversion between formats (with tests)

### IPC Communication
- `src/protocol/ipc.rs` - StationsUpdate, StateLockRequest/Ack messages
- Uses `postcard` for efficient binary serialization

### Integration Points
- `src/cli/modbus/master.rs` - CLI master processes StationsUpdate messages
- `src/tui/subprocess.rs` - TUI sends stations updates via `send_stations_update_for_port()`
- `src/protocol/status/util.rs` - Helper `port_stations_to_config()` for conversion

## Data Flow

```
TUI Configuration Change
    ↓
send_stations_update_for_port()
    ↓
port_stations_to_config() - Read from status
    ↓
register_items_to_stations() - Convert format
    ↓
postcard::to_allocvec() - Serialize
    ↓
IPC StationsUpdate message
    ↓
CLI Master receives
    ↓
postcard::from_bytes() - Deserialize
    ↓
Apply to Modbus storage context
    ↓
Update all register types
```

## Conversion Layer

The `config_convert` module provides bidirectional conversion:

```rust
// StationConfig → ModbusRegisterItem (flattens for runtime)
pub fn stations_to_register_items(stations: &[StationConfig]) 
    -> Vec<ModbusRegisterItem>

// ModbusRegisterItem → StationConfig (rebuilds hierarchy)
pub fn register_items_to_stations(
    items: &[ModbusRegisterItem],
    mode: ModbusConnectionMode
) -> Vec<StationConfig>

// Mode conversions
pub fn modbus_connection_mode_to_station_mode(mode: &ModbusConnectionMode) 
    -> StationMode
pub fn station_mode_to_modbus_connection_mode(mode: StationMode) 
    -> ModbusConnectionMode
```

## Testing

### Unit Tests
All tests passing (4/4):
```bash
cargo test --lib
```

Tests cover:
- Config JSON serialization
- Stations to register items conversion
- Register items to stations conversion
- TTY priority and annotation

### E2E Tests
Located in `examples/tui_e2e/src/e2e/`:
- `multi_masters/` - Tests for multiple masters on same port
- `multi_slaves/` - Tests for multiple slaves on same port

To run:
```bash
cd examples/tui_e2e
cargo run --release -- --skip-basic debug
```

## Integration Status

### ✅ Completed
- [x] Config structure redesigned
- [x] IPC message protocol updated
- [x] Postcard serialization integrated
- [x] Conversion layer implemented with tests
- [x] CLI master processes StationsUpdate
- [x] TUI helper methods implemented
- [x] All unit tests passing
- [x] Code quality checks passing

### 🔄 Integration Points Ready
- [ ] Wire TUI send calls to UI events
- [ ] Run multi_masters E2E tests
- [ ] Run multi_slaves E2E tests
- [ ] Fix issues found in testing
- [ ] Implement state locking if needed

## Wiring Up TUI Events

The helper method `send_stations_update_for_port()` is ready but not yet called from UI events.

### Current Usage of Individual Updates
Found in:
- `src/tui/ui/pages/modbus_panel/input/actions.rs:278`
- `src/tui/ui/pages/modbus_panel/input/editing.rs:453`

### Recommended Approach
Replace `SendRegisterUpdate` messages with calls to `send_stations_update_for_port()`:

```rust
// Instead of:
bus.ui_tx.send(UiToCore::SendRegisterUpdate { ... })?;

// Call:
subprocess_manager.send_stations_update_for_port(&port_name)?;
```

This sends the complete station configuration, ensuring full synchronization.

## State Locking

Infrastructure exists but not yet implemented:
- `StateLockRequest` - Request lock before sending update
- `StateLockAck` - Acknowledge lock granted/released

Only implement if E2E tests reveal race conditions.

## Migration Path

For existing code using the old Config format:

1. **Read old config**: Use `Config::from_json()` (still supports reading)
2. **Convert internally**: Runtime uses `ModbusRegisterItem` (unchanged)
3. **IPC uses new format**: `StationsUpdate` with `Vec<StationConfig>`
4. **Conversion automatic**: Done via `config_convert` module

## Troubleshooting

### Serialization Issues
If postcard serialization fails:
- Check all fields have `Serialize`/`Deserialize` derives
- Verify no circular references
- Test with simplified data first

### IPC Communication Issues
If messages don't arrive:
- Check socket creation and connection
- Verify both sides use same struct versions
- Enable debug logging with `log::debug!`

### Conversion Issues
If data doesn't match after conversion:
- Verify station IDs match
- Check register address ranges
- Ensure register modes are correct
- Run conversion tests in isolation

## Performance Considerations

### Postcard vs JSON
- Postcard: ~10x faster, ~50% smaller
- Used for IPC (high frequency)
- JSON still used for config files (human-readable)

### Full Sync vs Incremental
- Current: Full sync on every change
- Overhead: Minimal for typical configs (<100 stations)
- Future: Can optimize if needed

## Code Examples

### Creating a Station Config
```rust
let station = StationConfig {
    id: 1,
    mode: StationMode::Master,
    map: RegisterMap {
        holding: vec![
            RegisterRange {
                address_start: 0,
                length: 10,
                initial_values: vec![100, 200, 300],
            }
        ],
        coils: vec![
            RegisterRange {
                address_start: 100,
                length: 5,
                initial_values: vec![],
            }
        ],
        ..Default::default()
    },
};
```

### Sending via IPC
```rust
// In TUI subprocess manager
subprocess_manager.send_stations_update_for_port("COM1")?;
```

### Receiving in CLI
```rust
// In CLI master (already implemented)
match msg {
    IpcMessage::StationsUpdate { stations_data, .. } => {
        let stations: Vec<StationConfig> = 
            postcard::from_bytes(&stations_data)?;
        // Apply to storage...
    }
}
```

## Future Enhancements

1. **Incremental Updates**: Send only changed stations
2. **Delta Synchronization**: Send register value differences only
3. **Compression**: Add compression for large configurations
4. **Validation**: Add config validation before applying
5. **Rollback**: Support rolling back failed updates

## References

- Original issue: Multi-masters/slaves E2E tests
- IPC protocol: `src/protocol/ipc.rs`
- Conversion layer: `src/cli/config_convert.rs`
- Test examples: `examples/tui_e2e/src/e2e/`
