# CLI Modbus Features

This document describes the new CLI features for Modbus operations added to the aoba project.

## Features

### 1. Enhanced Port Listing

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

## Parameters

| Parameter | Description | Default |
|-----------|-------------|---------|
| `--station-id` | Modbus station ID (slave address) | 1 |
| `--register-address` | Starting register address | 0 |
| `--register-length` | Number of registers | 10 |
| `--register-mode` | Register type: holding, input, coils, discrete | holding |
| `--data-source` | Data source: `file:<path>` or `pipe:<name>` | - |
| `--baud-rate` | Serial port baud rate | 9600 |

## Register Modes

- `holding`: Holding Registers (read/write)
- `input`: Input Registers (read-only)
- `coils`: Coils (read/write bits)
- `discrete`: Discrete Inputs (read-only bits)
