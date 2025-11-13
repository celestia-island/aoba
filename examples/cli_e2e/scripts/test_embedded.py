#!/usr/bin/env python3
"""
Test script for RustPython embedded mode using aoba module.
This script uses the aoba.push_stations() and aoba.set_reboot_interval() functions.
"""

import aoba

# Define station configuration
stations_json = '''[
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
]'''

# Push stations using aoba module
aoba.push_stations(stations_json)

# Set reboot interval (1 second)
aoba.set_reboot_interval(1000)

print("Embedded mode test script executed successfully")
