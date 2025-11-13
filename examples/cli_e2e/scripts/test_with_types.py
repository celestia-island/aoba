#!/usr/bin/env python3
"""
Example script demonstrating IDE type hints and autocompletion with aoba module.

This script shows how the aoba.pyi stub file enables:
- Type checking with mypy, pyright, etc.
- IDE autocompletion in VSCode, PyCharm, etc.
- Function signature hints
- Docstring documentation

To use with type checking:
    mypy examples/cli_e2e/scripts/test_with_types.py

To use with embedded RustPython mode:
    aoba --master-provide-persist /dev/ttyUSB0 \
         --station-id 1 --register-mode holding \
         --data-source python:embedded:$(pwd)/test_with_types.py
"""

import json
# When running in RustPython embedded mode, this import will work
# When type checking, the aoba.pyi stub file provides type hints
try:
    import aoba
except ImportError:
    # This allows the script to be validated with type checkers
    # even when not running in RustPython
    print("Warning: aoba module not available (not in RustPython mode)")
    print("This script is intended to run in embedded mode")

def main() -> None:
    """
    Main function demonstrating aoba module usage with type hints.
    """
    # Define station configuration
    # IDE will show type hints and autocompletion based on aoba.pyi
    stations = [
        {
            "id": 1,  # StationId: 1-255
            "mode": "master",  # RegisterMode: "master" or "slave"
            "map": {
                "holding": [
                    {
                        "address_start": 0,  # RegisterAddress: 0-65535
                        "length": 10,
                        "initial_values": [
                            100, 200, 300, 400, 500,
                            600, 700, 800, 900, 1000
                        ]  # List[RegisterValue]: 0-65535
                    }
                ],
                "coils": [
                    {
                        "address_start": 0,
                        "length": 8,
                        "initial_values": [1, 0, 1, 0, 1, 0, 1, 0]  # Coils: 0 or 1
                    }
                ]
            }
        },
        {
            "id": 2,
            "mode": "master",
            "map": {
                "input": [
                    {
                        "address_start": 100,
                        "length": 5,
                        "initial_values": [111, 222, 333, 444, 555]
                    }
                ]
            }
        }
    ]
    
    # Convert to JSON string
    stations_json = json.dumps(stations)
    
    # Call aoba functions - IDE will show:
    # - Function signature: push_stations(stations_json: str) -> None
    # - Docstring with parameter description
    # - Return type
    # - Possible exceptions
    if 'aoba' in dir():
        aoba.push_stations(stations_json)
        
        # Set reboot interval - IDE will show:
        # - Function signature: set_reboot_interval(interval_ms: int) -> None
        # - Valid range and recommendations from docstring
        aoba.set_reboot_interval(2000)  # 2 seconds
        
        print("Station configuration pushed successfully")
        print(f"Configured {len(stations)} stations")
    else:
        # For testing/type checking outside RustPython
        print("DRY RUN MODE - Would push configuration:")
        print(json.dumps(stations, indent=2))

if __name__ == "__main__":
    main()
