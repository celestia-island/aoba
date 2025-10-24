# TUI E2E Test Suite

This directory contains end-to-end tests for the TUI (Terminal User Interface) mode of the aoba application.

## Test Architecture

### Isolated Test Execution

Each test module runs **completely independently** in its own isolated environment:

- ✅ **Separate CI Jobs**: Each test module runs as a separate GitHub Actions job
- ✅ **Fresh socat Initialization**: Each job starts with a clean socat setup
- ✅ **No State Sharing**: Tests cannot interfere with each other
- ✅ **Parallel Execution**: All tests run in parallel for faster CI

This architecture **completely eliminates** socat reset issues between tests because each test gets its own fresh virtual serial port environment.

### CI Workflow Structure

```yaml
tui-e2e:
  strategy:
    matrix:
      module:
        - cli_port_release
        - modbus_tui_slave_cli_master
        - modbus_tui_master_cli_slave
        - modbus_tui_multi_master_mixed_types
```

Each matrix entry runs as a separate job:
1. Fresh checkout
2. Download pre-built binaries
3. Initialize socat (fresh PTYs)
4. Run single test module
5. Cleanup

### Available Test Modules

- **cli_port_release**: Tests that CLI processes properly release serial ports
- **modbus_tui_master_cli_slave**: TUI as Modbus master, CLI as slave (single station)
- **modbus_tui_slave_cli_master**: TUI as Modbus slave, CLI as master (single station)
- **modbus_tui_multi_master_mixed_types**: TUI with multiple master stations (mixed register types)

## Running Tests Locally

### Single Test Module

```bash
# Setup ports first
./scripts/socat_init.sh

# Run a specific test
cargo run --package tui_e2e -- --module modbus_tui_master_cli_slave
```

### Multiple Tests (Sequential)

If you need to run multiple tests locally, reset socat between them:

```bash
# Test 1
./scripts/socat_init.sh
cargo run --package tui_e2e -- --module modbus_tui_master_cli_slave

# Reset before next test
pkill socat; sleep 1
./scripts/socat_init.sh

# Test 2
cargo run --package tui_e2e -- --module modbus_tui_slave_cli_master
```

### Debug Mode

Enable debug mode to see terminal breakpoints and detailed logging:

```bash
cargo run --package tui_e2e -- --module <module_name> --debug
```

## Adding New Test Modules

1. **Create test file** in `src/e2e/` (e.g., `my_test.rs`)
2. **Add to mod.rs** exports
3. **Add to main.rs** match statement
4. **Add to CI workflow** in `.github/workflows/e2e-tests.yml` matrix
5. **Document** in this README

Each new test will automatically run in isolation with fresh socat!

## Test Implementation Guidelines

### Use Status Tree Monitoring

Prefer `CheckStatus` actions over terminal pattern matching:

```rust
CursorAction::CheckStatus {
    description: "Port should be enabled".to_string(),
    path: "ports[0].enabled".to_string(),
    expected: json!(true),
    timeout_secs: Some(10),
    retry_interval_ms: Some(500),
}
```

### Multi-Station Configuration

Follow the documented workflow:
1. Create all stations first (press Enter N times)
2. Verify last station with regex
3. Navigate with Ctrl+PgUp + PgDown
4. Configure each station's fields
5. Save all with single Ctrl+S

### Clean State

Each test automatically cleans:
- TUI config cache (`~/.config/aoba/*.json`)
- Debug status files (`/tmp/ci_*_status.json`)

## Why This Architecture?

The isolated job architecture solves the fundamental issue with socat PTY exclusivity:

**Problem**: The serialport crate opens PTYs with exclusive access. After a test uses a port, subsequent tests cannot reopen it without resetting socat.

**Solution**: Each test runs in its own job with fresh socat initialization, completely avoiding the issue.

**Benefits**:
- ✅ No race conditions between tests
- ✅ Faster parallel execution
- ✅ Easier debugging (each test is independent)
- ✅ No complex cleanup coordination needed
- ✅ Tests can be run in any order

## Troubleshooting

### Port Busy Errors

If you see "Device or resource busy" errors locally, reset socat:

```bash
pkill socat
./scripts/socat_init.sh
```

### Tests Pass in CI but Fail Locally

Ensure you're resetting socat between test runs. In CI, each test gets a fresh environment automatically.

### Subprocess Dies Immediately

Check that:
1. socat is running: `ps aux | grep socat`
2. Ports exist: `ls -la /tmp/vcom*`
3. Permissions are correct: `ls -la /dev/pts/ 2>/dev/null || echo 'No pseudo-terminals found'`
