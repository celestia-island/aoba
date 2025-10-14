# TUI Multiple E2E Test

This test validates the reliability of multiple independent Modbus masters communicating with multiple slaves simultaneously, including signal interference handling.

## Overview

The test creates 6 virtual serial ports (vcom1-vcom6) arranged in 3 independent pairs:
- **Pair 1**: vcom1 (Master 1) ↔ vcom2 (Slave 1)
- **Pair 2**: vcom3 (Master 2) ↔ vcom4 (Slave 2)
- **Pair 3**: vcom5 (unused) ↔ vcom6 (Slave 3 - interference test)

## Test Scenarios

### 1. Multiple Independent Masters
- Spawns 2 independent TUI processes as Modbus masters
- Each master operates on its own port (vcom1 and vcom3)
- Masters run concurrently without interfering with each other

### 2. Master-Slave Communication
- Each master provides data to its designated slave via Modbus protocol
- Master 1 → Slave 1: Validates data transfer on vcom1-vcom2 pair
- Master 2 → Slave 2: Validates data transfer on vcom3-vcom4 pair

### 3. Signal Interference Testing
- Slave 3 attempts to poll from vcom6 (no master on vcom5)
- Verifies proper timeout/error handling when no master is present
- Ensures slaves don't receive data from wrong masters

## Running the Test

### Prerequisites
- Linux/Unix system with `socat` installed
- Rust toolchain

### Manual Execution
```bash
# Build the project
cargo build

# Run the test
cargo run --example tui_multiple_e2e
```

### CI Execution
The test is automatically run in GitHub Actions as part of the E2E test matrix.

## Test Flow

1. **Setup**: Creates 6 virtual serial ports using `scripts/socat_init.sh --mode tui_multiple`
2. **Master 1 Configuration**: Spawns TUI on vcom1, configures as Modbus master
3. **Master 2 Configuration**: Spawns second TUI on vcom3, configures as Modbus master
4. **Test Rounds** (5 iterations):
   - Generate random register data for each master
   - Update registers in both TUI processes
   - Poll slaves to verify correct data reception
   - Test interference on isolated port
5. **Cleanup**: Terminate TUI processes and verify clean exit

## Success Criteria

- ✓ Both masters operate independently without conflicts
- ✓ Each slave correctly receives data from its respective master
- ✓ Data from one master doesn't leak to other slaves
- ✓ Interference test properly fails/times out as expected
- ✓ All 5 test rounds pass without errors

## Architecture

### Port Assignment
```
TUI Process 1 (Master 1)  →  vcom1 ←→ vcom2  →  CLI Slave 1
TUI Process 2 (Master 2)  →  vcom3 ←→ vcom4  →  CLI Slave 2
(No Master)                  vcom5 ←→ vcom6  →  CLI Slave 3 (interference test)
```

### Key Components
- `main.rs`: Test orchestration and virtual port setup
- `tests/multiple_masters_slaves.rs`: Core test logic
- `scripts/socat_init.sh`: Virtual serial port creation with `tui_multiple` mode

## Differences from Standard TUI E2E Test

| Aspect | tui_e2e | tui_multiple_e2e |
|--------|---------|------------------|
| Ports | 2 (vcom1-vcom2) | 6 (vcom1-vcom6) |
| Masters | 1 TUI process | 2 independent TUI processes |
| Slaves | 1 CLI slave | 3 CLI slaves |
| Test Focus | Single master-slave pair | Multiple independent pairs + interference |
| Rounds | 10 | 5 (due to increased complexity) |

## Troubleshooting

### Port Creation Failures
If ports fail to create, ensure:
- No existing socat processes: `pkill socat`
- Clean up old port links: `rm -f /tmp/vcom*`
- Run setup script: `bash scripts/socat_init.sh --mode tui_multiple`

### TUI Navigation Issues
The test navigates to specific ports (vcom1, vcom3) in the TUI. If navigation fails:
- Check port ordering in TUI display
- Verify port aliases are correctly detected
- Review debug screenshots in test output

### Communication Failures
If data verification fails:
- Increase IPC propagation delay (currently 1500ms)
- Check that both TUI processes are running
- Verify socat processes are active: `ps aux | grep socat`

## Future Enhancements

Potential improvements for this test:
- [ ] Add more slaves per master (e.g., 2-3 slaves per master)
- [ ] Test cross-pair communication interference
- [ ] Verify signal isolation metrics
- [ ] Add stress testing with rapid data updates
- [ ] Test recovery from port disconnection
