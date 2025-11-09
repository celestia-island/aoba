# CLI Modbus Features

This document describes the new CLI features for Modbus operations added to the aoba project.

## Features

### 1. Port Detection and Listing

#### List All Ports

The `--list-ports` command now provides more detailed information when used with `--json`:

```bash
aoba --list-ports --json
```

Output includes:

- `path`: Port path (e.g., COM1, /dev/ttyUSB0)
- `status`: "Free" or "Occupied"
- `guid`: Windows device GUID (if available)
- `vid`: USB Vendor ID (if available)
- `pid`: USB Product ID (if available)
- `serial`: Serial number (if available)

Example output:

```json
[
  {
    "path": "COM1",
    "status": "Free",
    "guid": "{...}",
    "vid": 1234,
    "pid": 5678
  }
]
```

#### Check Single Port Occupation Status

The `--check-port` command is used to detect whether a specific port is occupied. This is useful for script automation and port status monitoring:

```bash
aoba --check-port COM3
```

**Exit Codes:**

- `0` - Port is free and available
- `1` - Port is occupied by another program

**Plain Output:**

```
Port COM3 is free
```

or

```
Port COM3 is occupied
```

**JSON Format Output:**

```bash
aoba --check-port COM3 --json
```

Example output:

```json
{"port":"COM3","occupied":false,"status":"Free"}
```

or

```json
{"port":"COM3","occupied":true,"status":"Occupied"}
```

**Usage Examples:**

Using in shell scripts:

```bash
# Bash example
if aoba --check-port /dev/ttyUSB0; then
    echo "Port is free, ready to use"
    # Perform your operations
else
    echo "Port is occupied, please close the program using this port"
    exit 1
fi
```

```powershell
# PowerShell example
cargo run --package aoba -- --check-port COM3
if ($LASTEXITCODE -eq 0) {
    Write-Host "Port is free"
} else {
    Write-Host "Port is occupied"
}
```

### 2. Slave Listen Modes

#### Temporary Mode

Listen for one Modbus request, respond, and exit:

```bash
aoba --slave-listen /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 10 \
  --register-mode holding \
  --baud-rate 9600
```

Outputs a single JSON response and exits.

#### Persistent Mode

Continuously listen for requests and output JSONL:

```bash
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 10 \
  --register-mode holding \
  --baud-rate 9600
```

Outputs one JSON line per request processed.

### 3. Master Provide Modes

- Temporary Mode, provide data once and exit:

```bash
aoba --master-provide /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source file:/path/to/data.json \
  --baud-rate 9600
```

Reads one line from the data source, sends it, and exits.

- Persistent Mode, continuously provide data:

```bash
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source file:/path/to/data.json \
  --baud-rate 9600
```

Reads lines from the data source and sends them continuously.

### Data Source Format

For master modes, the data source file should contain JSONL format:

```json
{"values": [10, 20, 30, 40, 50]}
{"values": [15, 25, 35, 45, 55]}
{"values": [20, 30, 40, 50, 60]}
```

Each line represents an update to be sent to the slave.

#### Using Files as Data Source

```bash
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source file:/path/to/data.json \
  --baud-rate 9600
```

#### Using Unix Named Pipes as Data Source

Unix named pipes (FIFOs) can be used for real-time data streaming:

```bash
# Create named pipe
mkfifo /tmp/modbus_input

# Start master in one terminal
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source pipe:/tmp/modbus_input \
  --baud-rate 9600

# Write data in another terminal
echo '{"values": [10, 20, 30, 40, 50]}' > /tmp/modbus_input
```

### Output Destinations

For slave modes, you can specify output destinations:

#### Output to stdout (default)

```bash
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --baud-rate 9600
```

#### Output to File (append mode)

```bash
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --baud-rate 9600 \
  --output file:/path/to/output.jsonl
```

#### Output to Unix Named Pipe

```bash
# Create named pipe
mkfifo /tmp/modbus_output

# Start slave in one terminal
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --baud-rate 9600 \
  --output pipe:/tmp/modbus_output

# Read data in another terminal
cat /tmp/modbus_output
```

## Daemon Mode (Persistent Operation)

The CLI supports daemon-like continuous operation through the **persist modes**:

- **Slave daemon**: Use `--slave-listen-persist` for continuous listening and responding
- **Master daemon**: Use `--master-provide-persist` for continuous data provision

These modes run indefinitely until interrupted (Ctrl+C) and output JSONL (one JSON object per line) for each operation. They are ideal for:

- Long-running monitoring applications
- Data logging systems
- Integration with other tools via pipes or files
- TUI subprocess communication (when combined with `--ipc-channel`)

Example daemon usage:

```bash
# Run as slave daemon with file output logging
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-mode holding \
  --output file:/var/log/modbus-slave.jsonl

# Run as master daemon with pipe input
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-mode holding \
  --data-source pipe:/tmp/modbus_data
```

**Note**: The TUI mode uses these persist modes internally with `--ipc-channel` for bidirectional communication with CLI subprocesses.

## Parameters

| Parameter | Description | Default |
|-----------|-------------|---------|
| `--station-id` | Modbus station ID (slave address) | 1 |
| `--register-address` | Starting register address | 0 |
| `--register-length` | Number of registers | 10 |
| `--register-mode` | Register type: holding, input, coils, discrete | holding |
| `--data-source` | Data source: `file:<path>` or `pipe:<name>` | - |
| `--output` | Output destination: `file:<path>` or `pipe:<name>` (default: stdout) | stdout |
| `--baud-rate` | Serial port baud rate | 9600 |
| `--debounce-seconds` | Debounce window for duplicate JSON output (seconds, float) | 1.0 |
| `--ipc-channel` | IPC channel UUID for TUI communication (internal use) | - |

## Register Modes

- `holding`: Holding Registers (read/write)
- `input`: Input Registers (read-only)
- `coils`: Coils (read/write bits)
- `discrete`: Discrete Inputs (read-only bits)

## Integration Tests

Integration tests are available in `examples/cli_e2e/`. Run them with:

```bash
cd examples/cli_e2e
cargo run
```

### Running Tests in Loop Mode

For stability testing and debugging, you can run tests multiple times using the `--loop-count` command-line argument:

```bash
# Run tests 5 times consecutively
cargo run --example cli_e2e -- --loop-count 5

# Run tests 10 times to verify port cleanup and stability
cargo run --example cli_e2e -- --loop-count 10
```

This is useful for:

- Verifying port cleanup between test runs
- Testing stability and repeatability
- Debugging intermittent issues
- Ensuring socat virtual port reset works correctly

Tests verify:

- Enhanced port listing with status
- Slave listen temporary mode
- Slave listen persistent mode
- Master provide temporary mode
- Master provide persistent mode
- Continuous connection test (file data source and file output)
- Continuous connection test (Unix pipe data source and pipe output)

### Continuous Connection Tests

Continuous connection tests verify long-running data transmission between master and slave:

1. **Files as data source and output**: Master reads data from file and sends, slave receives and appends to file
2. **Unix pipes as data source and output**: Master reads real-time data from named pipe, slave outputs to named pipe
3. **Random data generation**: Each test run generates different random data to ensure test reliability

## Future Enhancements

- Real-time Modbus communication tests with virtual serial ports
- Additional register mode support
