#!/usr/bin/env python3
"""
Test script for external Python data source mode.
Outputs a simple station configuration with holding registers.
"""

import json
import sys

def main():
    # Define a simple station configuration
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
    
    # Print to stdout (one line)
    print(json.dumps(output))
    
    # Optional: Print debug info to stderr
    sys.stderr.write("Test script executed successfully\n")

if __name__ == "__main__":
    main()
