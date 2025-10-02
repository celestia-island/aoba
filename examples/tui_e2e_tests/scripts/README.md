# Virtual Serial Port Scripts

This directory contains scripts for managing virtual serial ports (socat) in CI and local testing environments.

## Scripts

### `socat_init.sh`

Initial setup script that creates virtual serial port pairs `/dev/vcom1` and `/dev/vcom2`.

**Usage:**
```bash
sudo ./socat_init.sh
```

**What it does:**
1. Stops any existing socat processes
2. Removes old virtual port symlinks
3. Creates new PTY pairs linked to `/dev/vcom1` and `/dev/vcom2`
4. Sets permissions (mode 0666) for accessibility
5. Performs a connectivity test to verify the ports work

**Exit codes:**
- `0`: Success - ports created and tested
- `1`: Failed to create ports within timeout
- `2`: Connectivity test failed

### `socat_reset.sh`

Reset script that tears down and recreates virtual serial ports between tests.

**Usage:**
```bash
sudo ./socat_reset.sh
```

**What it does:**
1. Kills existing socat processes
2. Removes old virtual port symlinks
3. Creates fresh PTY pairs
4. Performs a connectivity test

This script is called automatically by the test framework between test cases to ensure clean state and prevent port reuse issues.

**Exit codes:**
- `0`: Success - ports reset and tested
- `1`: Failed to recreate ports
- `2`: Connectivity test failed

## How Port Reset Works in Tests

The test framework automatically resets virtual ports between test cases:

1. **Test 1 runs** → Uses `/dev/vcom1` and `/dev/vcom2`
2. **Test 1 completes** → Processes exit, releasing ports
3. **Reset triggered** → `socat_reset.sh` runs
4. **Test 2 runs** → Uses freshly created `/dev/vcom1` and `/dev/vcom2`

This ensures each test starts with a clean slate and prevents issues with:
- Port handles not being fully released
- Stale data in port buffers
- Permission issues from previous test runs
- Multiple processes trying to open the same port

## Troubleshooting

### Ports not appearing
```bash
# Check if socat is running
ps aux | grep socat

# Check if symlinks exist
ls -la /dev/vcom*

# Check socat logs
sudo tail -f /tmp/socat_vcom.log
```

### Permission denied when opening ports
```bash
# Ensure correct permissions on underlying PTY
sudo chmod 666 $(readlink -f /dev/vcom1)
sudo chmod 666 $(readlink -f /dev/vcom2)
```

### Ports in use / cannot be opened
```bash
# Reset the ports
sudo ./socat_reset.sh

# Or manually kill and recreate
sudo pkill socat
sudo rm -f /dev/vcom1 /dev/vcom2
sudo ./socat_init.sh
```

## Implementation Details

The reset mechanism uses:
- **Shell scripts** for port lifecycle management
- **Rust helper function** (`reset_vcom_ports()` in `src/ci/utils.rs`) to call scripts from tests
- **Automatic delays** to ensure processes release ports before reset
- **Connectivity tests** to verify ports work after creation/reset

This approach was chosen over alternatives because:
1. ✅ **Simpler than multi-port pairs** - No need to manage multiple sets of ports
2. ✅ **More reliable than concurrent access** - PTY pairs don't support multiple simultaneous connections
3. ✅ **Clean state guaranteed** - Each test gets fresh ports
4. ✅ **Easy to debug** - Scripts can be run manually for testing
