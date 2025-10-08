# CI Virtual Serial Port Setup

This document describes how the CI pipeline sets up virtual serial ports for end-to-end testing on different platforms.

## Overview

The E2E tests require a pair of virtual serial ports that are connected to each other. When data is written to one port, it can be read from the other port. This simulates a real serial connection without requiring physical hardware.

## Platform-Specific Implementations

### Linux (socat)

**Script:** `scripts/socat_init.sh`

**Virtual Ports:** `/tmp/vcom1` and `/tmp/vcom2`

**How it works:**
- Uses `socat` to create a pair of pseudo-terminal devices (PTY)
- Creates symbolic links at `/tmp/vcom1` and `/tmp/vcom2`
- Sets permissions to allow non-root access (mode 0666)
- Performs connectivity test by writing to one port and reading from the other

**Usage:**
```bash
chmod +x scripts/socat_init.sh
sudo scripts/socat_init.sh
```

**Cleanup:**
```bash
sudo pkill socat
sudo rm -f /tmp/vcom1 /tmp/vcom2
```

### Windows (com0com)

**Script:** `scripts/com0com_init.ps1`

**Virtual Ports:** `COM1` and `COM2`

**How it works:**
- Uses com0com, a null-modem emulator for Windows
- Downloads and installs com0com from SourceForge (version 3.0.0.0 signed)
- Creates a virtual port pair using `setupc.exe` command-line tool
- Configures ports with emulated baud rate and overrun settings
- Performs connectivity test using .NET SerialPort class

**Usage:**
```powershell
powershell -ExecutionPolicy Bypass -File scripts\com0com_init.ps1
```

**Cleanup:**
```powershell
# Find setupc.exe location
$setupcPath = "C:\Program Files (x86)\com0com\setupc.exe"
if (-not (Test-Path $setupcPath)) {
    $setupcPath = "C:\Program Files\com0com\setupc.exe"
}

# Remove port pair
& $setupcPath remove 0
```

## CI Configuration

### Environment Variables

Both platforms use environment variables to configure port detection:

- `AOBATEST_PORT1`: Name of the first virtual port
  - Linux: `/tmp/vcom1`
  - Windows: `COM1`
- `AOBATEST_PORT2`: Name of the second virtual port
  - Linux: `/tmp/vcom2`
  - Windows: `COM2`
- `CI_FORCE_VCOM`: Set to `"1"` on Windows to enable virtual port tests
  - Windows requires this because `cfg!(unix)` returns false
  - Linux enables virtual port tests by default

### GitHub Actions Workflow

The workflow file `.github/workflows/e2e-tests.yml` defines two jobs:

#### `e2e-test-linux`
```yaml
- name: Setup virtual serial ports
  run: |
    chmod +x scripts/socat_init.sh
    sudo scripts/socat_init.sh

- name: Run TUI E2E tests
  env:
    AOBATEST_PORT1: /tmp/vcom1
    AOBATEST_PORT2: /tmp/vcom2
  run: |
    cargo run --example ${{ matrix.example }}
```

#### `e2e-test-windows`
```yaml
- name: Download and install com0com
  shell: powershell
  run: |
    # Download from SourceForge
    # Extract and install silently

- name: Setup virtual serial ports
  shell: powershell
  run: |
    $env:CI_FORCE_VCOM = "1"
    powershell -ExecutionPolicy Bypass -File scripts\com0com_init.ps1

- name: Run E2E tests
  env:
    CI_FORCE_VCOM: "1"
  run: |
    cargo run --example ${{ matrix.example }}
```

## Port Detection in Code

### `src/ci/utils.rs`

The `vcom_matchers()` function returns platform-appropriate port names and regex matchers:

```rust
pub fn vcom_matchers() -> VcomMatchers {
    let (p1, p2) = if cfg!(windows) {
        ("COM1".to_string(), "COM2".to_string())
    } else {
        ("/dev/vcom1".to_string(), "/dev/vcom2".to_string())
    };
    // ... builds regex matchers
}
```

The `should_run_vcom_tests()` function determines if virtual port tests should run:

```rust
pub fn should_run_vcom_tests() -> bool {
    if !cfg!(unix) {
        return std::env::var("CI_FORCE_VCOM")
            .map(|v| v == "1")
            .unwrap_or(false);
    }
    true
}
```

### `src/protocol/tty/tty_unix.rs`

The `detect_virtual_ports()` function checks for virtual ports in multiple locations:

```rust
let virtual_port_paths = [
    "/dev/vcom1", "/dev/vcom2", "/dev/vcom3", "/dev/vcom4",
    "/tmp/vcom1", "/tmp/vcom2", "/tmp/vcom3", "/tmp/vcom4",
];
```

### `src/protocol/tty/tty_windows.rs`

The `available_ports_sorted()` function includes virtual ports detected by the serialport crate. com0com virtual ports appear automatically in `serialport::available_ports()` on Windows.

## Testing Locally

### Linux

1. Install socat:
   ```bash
   sudo apt-get install socat
   ```

2. Run the initialization script:
   ```bash
   chmod +x scripts/socat_init.sh
   sudo scripts/socat_init.sh
   ```

3. Run the E2E tests:
   ```bash
   export AOBATEST_PORT1=/tmp/vcom1
   export AOBATEST_PORT2=/tmp/vcom2
   cargo run --example cli_e2e
   ```

### Windows

1. Download and install com0com from:
   https://sourceforge.net/projects/com0com/files/com0com/3.0.0.0/

2. Run the initialization script:
   ```powershell
   powershell -ExecutionPolicy Bypass -File scripts\com0com_init.ps1
   ```

3. Run the E2E tests:
   ```powershell
   $env:CI_FORCE_VCOM = "1"
   cargo run --example cli_e2e
   ```

## Troubleshooting

### Linux

**Problem:** Permission denied when accessing `/tmp/vcomX`

**Solution:** Ensure the script runs with sudo and sets mode 0666:
```bash
sudo chmod 666 /tmp/vcom1 /tmp/vcom2
# Also check the underlying PTY devices
sudo chmod 666 /dev/pts/X
```

**Problem:** Ports not detected

**Solution:** Check if socat is running and links exist:
```bash
ps aux | grep socat
ls -la /tmp/vcom*
```

### Windows

**Problem:** com0com not installed

**Solution:** Install com0com manually or check the installation step in CI logs.

**Problem:** COM ports already in use

**Solution:** Remove existing port pairs:
```powershell
& "C:\Program Files\com0com\setupc.exe" list
& "C:\Program Files\com0com\setupc.exe" remove 0
```

**Problem:** Tests skip virtual port checks

**Solution:** Ensure `CI_FORCE_VCOM=1` is set:
```powershell
$env:CI_FORCE_VCOM = "1"
```

## References

- socat: http://www.dest-unreach.org/socat/
- com0com: http://com0com.sourceforge.net/
- GitHub Actions Windows runners: https://github.com/actions/runner-images
