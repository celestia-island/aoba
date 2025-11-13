# Python Test Scripts for CLI E2E Tests

This directory contains Python test scripts used in CLI E2E tests for the Python data source feature.

## Scripts

### test_simple.py

Basic test script that outputs a simple station configuration with holding registers.

- Station ID: 1
- Register type: Holding
- Address range: 0-9
- Values: 100, 200, 300, ... 1000
- Reboot interval: 1000ms

### test_dynamic.py

Test script that generates random sensor data simulating dynamic values.

- Station ID: 1
- Register type: Holding
- Address range: 0-2
- Values: Random temperature, humidity, pressure
- Reboot interval: 2000ms

### test_multi_station.py

Test script demonstrating multiple stations with different register types.

- Station 1: Holding registers (address 0-4)
- Station 2: Coil registers (address 0-7)
- Station 3: Input registers (address 10-13)
- Reboot interval: 1500ms

## Running Scripts

All scripts can be executed standalone:

```bash
# Run script
python3 test_simple.py

# Validate JSON output
python3 test_simple.py | python3 -m json.tool

# Check stderr output
python3 test_simple.py 2>&1 | grep -i "stderr\|error\|warning"
```

## Using with Aoba

These scripts can be used with Aoba's Python data source:

```bash
# Simple test
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 10 \
  --register-mode holding \
  --baud-rate 9600 \
  --data-source python:external:$(pwd)/test_simple.py

# Dynamic data test
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 3 \
  --register-mode holding \
  --baud-rate 9600 \
  --data-source python:external:$(pwd)/test_dynamic.py
```

## Requirements

- Python 3.x
- Standard library only (no external dependencies)
