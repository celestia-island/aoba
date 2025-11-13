#!/usr/bin/env python3
"""
Python script data source for Modbus master testing (RustPython embedded mode).
This script uses the aoba module to push station configurations.
The script cycles through three test patterns on each execution.
"""

import aoba

# State management using a simple counter file
# This allows us to cycle through rounds on each execution
import os
tempdir = "/tmp"
counter_file = os.path.join(tempdir, "modbus_data_source_counter.txt")


def get_round_number():
    """Get current round number from counter file."""
    if os.path.exists(counter_file):
        try:
            with open(counter_file, 'r') as f:
                count = int(f.read().strip())
                # Cycle through rounds 1, 2, 3
                return ((count % 3) + 1)
        except (IOError, ValueError):
            pass
    return 1


def increment_counter():
    """Increment the counter for next execution."""
    current = 0
    if os.path.exists(counter_file):
        try:
            with open(counter_file, 'r') as f:
                current = int(f.read().strip())
        except (IOError, ValueError):
            pass

    with open(counter_file, 'w') as f:
        f.write(str(current + 1))


# Get current round
round_num = get_round_number()

# Define register values based on round
if round_num == 1:
    # Round 1: Sequential 0-9
    values = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
elif round_num == 2:
    # Round 2: Reverse 9-0
    values = [9, 8, 7, 6, 5, 4, 3, 2, 1, 0]
else:  # round_num == 3
    # Round 3: Custom pattern
    values = [0x1111, 0x2222, 0x3333, 0x4444, 0x5555,
              0x6666, 0x7777, 0x8888, 0x9999, 0xAAAA]

# Create station configuration JSON
stations_json = f'''[
    {{
        "id": 1,
        "mode": "master",
        "map": {{
            "holding": [
                {{
                    "address_start": 0,
                    "length": 10,
                    "initial_values": {values}
                }}
            ]
        }}
    }}
]'''

# Push stations using aoba module
aoba.push_stations(stations_json)

# Set reboot interval (1 second)
aoba.set_reboot_interval(1000)

# Increment counter for next run
increment_counter()
