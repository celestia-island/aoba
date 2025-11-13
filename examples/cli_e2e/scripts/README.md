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

### test_with_types.py

Example script demonstrating IDE type hints and autocompletion with the aoba module.

This script uses the `aoba.pyi` stub file to provide:

- Type checking with mypy, pyright, etc.
- IDE autocompletion in VSCode, PyCharm, etc.
- Function signature hints
- Docstring documentation

## Type Hints and IDE Support

### aoba.pyi

Python type stub file that provides type hints and IDE support for the `aoba` module (available in RustPython embedded mode).

The stub file includes:

- `push_stations(stations_json: str) -> None`: Function signature with parameter types
- `set_reboot_interval(interval_ms: int) -> None`: Function signature with parameter types
- Comprehensive docstrings with parameter descriptions, return types, and exceptions
- JSON schema documentation for StationConfig
- Type aliases for better code clarity

### py.typed

Marker file indicating this package supports type checking. This tells Python type checkers (mypy, pyright, etc.) to look for `.pyi` stub files.

### Using Type Hints

1. **Place aoba.pyi in your script directory**: The stub file should be in the same directory as your Python scripts or in your Python path.

2. **Import the aoba module normally**:

   ```python
   import aoba
   ```

3. **Your IDE will now provide**:
   - Autocompletion for `aoba.push_stations()` and `aoba.set_reboot_interval()`
   - Inline documentation from docstrings
   - Type checking for parameters
   - Error highlighting for type mismatches

4. **Type check your scripts**:

   ```bash
   mypy your_script.py
   ```

### Example with Type Hints

```python
import json
import aoba

# IDE shows: push_stations(stations_json: str) -> None
stations_json = json.dumps([{
    "id": 1,
    "mode": "master", 
    "map": {
        "holding": [{
            "address_start": 0,
            "length": 10,
            "initial_values": [100, 200, 300]
        }]
    }
}])

aoba.push_stations(stations_json)  # Type checked!

# IDE shows: set_reboot_interval(interval_ms: int) -> None
aoba.set_reboot_interval(1000)  # Type checked!
```

## Running Scripts

All scripts can be executed standalone:

```bash
# Run script
python3 test_simple.py

# Validate JSON output
python3 test_simple.py | python3 -m json.tool

# Check stderr output
python3 test_simple.py 2>&1 | grep -i "stderr\|error\|warning"

# Type check with mypy (requires mypy: pip install mypy)
mypy test_with_types.py
```

## Using with Aoba

These scripts can be used with Aoba's Python data source:

```bash
# External mode (CPython) - recommended for full Python ecosystem
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 10 \
  --register-mode holding \
  --baud-rate 9600 \
  --data-source python:external:$(pwd)/test_simple.py

# Embedded mode (RustPython) - no external Python dependency
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 10 \
  --register-mode holding \
  --baud-rate 9600 \
  --data-source python:embedded:$(pwd)/test_simple.py

# With type hints example
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 10 \
  --register-mode holding \
  --baud-rate 9600 \
  --data-source python:embedded:$(pwd)/test_with_types.py
```

## Requirements

- Python 3.x
- Standard library only (no external dependencies required)
- Optional: mypy for type checking (`pip install mypy`)
- Optional: IDE with Python support (VSCode with Pylance, PyCharm, etc.)
