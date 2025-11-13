# Python Script Data Source API

## Overview

The Python Script Data Source allows you to provide dynamic data for Modbus master stations using Python scripts. The script is executed periodically, and its output is used to update the Modbus register values.

## Execution Modes

There are two execution modes for Python scripts:

### 1. External CPython Mode (Recommended)

Uses the system's Python interpreter (`python` or `python3`) to execute the script in a separate process.

**Advantages:**
- Works with all Python libraries and modules
- Full Python standard library support
- Compatible with Python 2.7 and Python 3.x
- No additional dependencies required

**Usage:**
```bash
--data-source python:external:/path/to/script.py
```

### 2. Embedded RustPython Mode (Currently Disabled)

Would use RustPython VM to execute scripts within the Aoba process.

**Status:** This mode is currently disabled due to threading compatibility issues with RustPython 0.4. It will be re-enabled in a future release once these issues are resolved.

## JSON Output Format

Your Python script must output JSON to stdout in one of two formats:

### Format 1: Stations Array (Recommended)

```json
{
  "stations": [
    {
      "id": 1,
      "mode": "master",
      "map": {
        "holding": [
          {
            "address_start": 0,
            "length": 10,
            "initial_values": [100, 200, 300, 400, 500, 600, 700, 800, 900, 1000]
          }
        ]
      }
    }
  ],
  "reboot_interval": 1000
}
```

### Format 2: Line-by-Line JSON

Your script can also output one JSON object per line (JSON Lines format):

```json
{"stations": [{"id": 1, "mode": "master", "map": {"holding": [{"address_start": 0, "length": 5, "initial_values": [1, 2, 3, 4, 5]}]}}], "reboot_interval": 2000}
{"stations": [{"id": 1, "mode": "master", "map": {"holding": [{"address_start": 0, "length": 5, "initial_values": [6, 7, 8, 9, 10]}]}}]}
```

## JSON Schema

### Station Object

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | integer | Yes | Station ID (1-247) |
| `mode` | string | Yes | Station mode: `"master"` or `"slave"` |
| `map` | object | Yes | Register map containing register ranges |

### Register Map Object

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `coils` | array | No | Array of coil register ranges |
| `discrete_inputs` | array | No | Array of discrete input register ranges |
| `holding` | array | No | Array of holding register ranges |
| `input` | array | No | Array of input register ranges |

### Register Range Object

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `address_start` | integer | Yes | Starting register address (0-65535) |
| `length` | integer | Yes | Number of registers (1-65536) |
| `initial_values` | array | No | Array of initial register values (u16) |

### Root Output Object

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `stations` | array | Yes | Array of station configurations |
| `reboot_interval` | integer | No | Time in milliseconds before script is executed again |

## Standard Error (stderr)

Any output written to stderr will be captured and logged by Aoba as warnings. This is useful for debugging your script:

```python
import sys
sys.stderr.write("Debug: Processing station 1\n")
```

## Example Scripts

### Example 1: Simple Static Data

```python
#!/usr/bin/env python3
import json

# Define station configuration
stations = [
    {
        "id": 1,
        "mode": "master",
        "map": {
            "holding": [
                {
                    "address_start": 0,
                    "length": 10,
                    "initial_values": [100, 200, 300, 400, 500, 600, 700, 800, 900, 1000]
                }
            ]
        }
    }
]

# Output JSON
output = {
    "stations": stations,
    "reboot_interval": 1000  # Execute every 1 second
}

print(json.dumps(output))
```

### Example 2: Dynamic Data from Sensors

```python
#!/usr/bin/env python3
import json
import random
import time

# Simulate reading sensor data
temperature = random.randint(20, 30)
humidity = random.randint(40, 60)
pressure = random.randint(1000, 1020)

# Create station with sensor readings
stations = [
    {
        "id": 1,
        "mode": "master",
        "map": {
            "holding": [
                {
                    "address_start": 0,
                    "length": 3,
                    "initial_values": [temperature, humidity, pressure]
                }
            ]
        }
    }
]

# Output JSON
output = {
    "stations": stations,
    "reboot_interval": 5000  # Update every 5 seconds
}

print(json.dumps(output))
```

### Example 3: Multiple Stations

```python
#!/usr/bin/env python3
import json

# Define multiple stations
stations = [
    {
        "id": 1,
        "mode": "master",
        "map": {
            "holding": [
                {
                    "address_start": 0,
                    "length": 5,
                    "initial_values": [1, 2, 3, 4, 5]
                }
            ]
        }
    },
    {
        "id": 2,
        "mode": "master",
        "map": {
            "coils": [
                {
                    "address_start": 0,
                    "length": 8,
                    "initial_values": [1, 0, 1, 0, 1, 0, 1, 0]
                }
            ]
        }
    }
]

# Output JSON
print(json.dumps({"stations": stations, "reboot_interval": 2000}))
```

### Example 4: Reading from Database

```python
#!/usr/bin/env python3
import json
import sqlite3
import sys

try:
    # Connect to database
    conn = sqlite3.connect('/path/to/sensors.db')
    cursor = conn.cursor()
    
    # Query latest sensor readings
    cursor.execute('''
        SELECT station_id, register_address, value 
        FROM sensor_readings 
        WHERE timestamp > datetime('now', '-1 minute')
        ORDER BY station_id, register_address
    ''')
    
    # Group readings by station
    stations_data = {}
    for row in cursor.fetchall():
        station_id, address, value = row
        if station_id not in stations_data:
            stations_data[station_id] = []
        stations_data[station_id].append((address, value))
    
    # Build station configurations
    stations = []
    for station_id, readings in stations_data.items():
        min_addr = min(addr for addr, _ in readings)
        max_addr = max(addr for addr, _ in readings)
        length = max_addr - min_addr + 1
        
        # Fill values array
        values = [0] * length
        for addr, value in readings:
            values[addr - min_addr] = value
        
        stations.append({
            "id": station_id,
            "mode": "master",
            "map": {
                "holding": [{
                    "address_start": min_addr,
                    "length": length,
                    "initial_values": values
                }]
            }
        })
    
    conn.close()
    
    # Output JSON
    print(json.dumps({
        "stations": stations,
        "reboot_interval": 60000  # Update every minute
    }))

except Exception as e:
    # Log error to stderr
    sys.stderr.write(f"Error reading from database: {e}\n")
    sys.exit(1)
```

## Best Practices

1. **Always output valid JSON** - Invalid JSON will cause the script to be ignored
2. **Use stderr for debugging** - All stderr output is logged as warnings
3. **Set appropriate reboot_interval** - Balance between update frequency and system load
4. **Handle errors gracefully** - Use try-except blocks and exit with non-zero status on fatal errors
5. **Keep scripts fast** - Long-running scripts will block Modbus communication
6. **Validate your output** - Test your script standalone before using it with Aoba

## Testing Your Script

You can test your Python script independently before using it with Aoba:

```bash
# Run the script
python3 /path/to/script.py

# Validate JSON output
python3 /path/to/script.py | python3 -m json.tool

# Check for errors
python3 /path/to/script.py 2>&1 | grep -i error
```

## Troubleshooting

### Script Not Executing

1. **External Mode**: Verify Python is installed and in PATH
   ```bash
   which python3  # Unix
   Get-Command python3  # Windows
   ```

2. **Embedded Mode**: Check RustPython compatibility
   - Some Python features may not be supported in RustPython 0.4
   - Use external mode for full Python compatibility

### Invalid JSON Output

- Ensure JSON is printed to stdout, not stderr
- Use `json.dumps()` to generate valid JSON
- Avoid extra print statements that would corrupt JSON output

### Type Checking Errors

If using the `aoba.pyi` stub file:
- Ensure `aoba.pyi` and `py.typed` are in the same directory as your script
- Use try/except ImportError to handle when aoba module is not available
- Run `mypy your_script.py` to validate types

## IDE Support and Type Hints

### Type Stub Files

The aoba module (available in RustPython embedded mode) includes type stub files for IDE support:

- **`aoba.pyi`**: Python type stub file with function signatures, parameter types, and docstrings
- **`py.typed`**: Marker file indicating the package supports type checking

### Benefits

With these stub files, your IDE will provide:

1. **Autocompletion**: Function names and parameters
2. **Type Checking**: Static type analysis with mypy, pyright, etc.
3. **Inline Documentation**: Function docstrings and parameter descriptions
4. **Error Detection**: Type mismatches highlighted before runtime

### Setup

1. Copy `aoba.pyi` and `py.typed` to your script directory:
   ```bash
   cp examples/cli_e2e/scripts/aoba.pyi /path/to/your/scripts/
   cp examples/cli_e2e/scripts/py.typed /path/to/your/scripts/
   ```

2. Import the aoba module normally:
   ```python
   import aoba
   ```

3. Your IDE will now show type hints and documentation!

### Example with Type Hints

```python
import json
import aoba

# IDE shows: push_stations(stations_json: str) -> None
# Hovering shows the docstring with parameter details
stations = [{
    "id": 1,
    "mode": "master",
    "map": {
        "holding": [{
            "address_start": 0,
            "length": 10,
            "initial_values": [100, 200, 300]
        }]
    }
}]

# Type checked: ensures stations_json is a string
aoba.push_stations(json.dumps(stations))

# IDE shows: set_reboot_interval(interval_ms: int) -> None
# Type checked: ensures interval_ms is an integer
aoba.set_reboot_interval(1000)
```

### Type Checking with mypy

```bash
# Install mypy
pip install mypy

# Type check your script
mypy your_script.py

# Example output:
# your_script.py:10: error: Argument 1 to "push_stations" has incompatible type "dict"; expected "str"
# Found 1 error in 1 file (checked 1 source file)
```

### Supported IDEs

The type stub files work with:
- **VSCode** with Pylance extension
- **PyCharm** (Professional and Community)
- **Sublime Text** with LSP-pyright
- **Vim/Neovim** with coc-pyright or vim-lsp
- **Emacs** with lsp-pyright
- Any editor with Python Language Server support

## Troubleshooting

### Script not found

Make sure the script path is absolute and the file exists:
```bash
ls -l /path/to/script.py
```

### Python not found

Ensure Python is installed and in your PATH:
```bash
which python3
python3 --version
```

### Permission denied

Make the script executable (Linux/macOS):
```bash
chmod +x /path/to/script.py
```

### Invalid JSON output

Test your JSON output:
```bash
python3 /path/to/script.py | jq .
```

### Script execution fails

Check stderr output in Aoba logs for error messages. Enable debug logging for more details.
