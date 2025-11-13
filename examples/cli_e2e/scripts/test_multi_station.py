#!/usr/bin/env python3
"""
Test script for external Python data source with multiple stations.
Tests handling of multiple Modbus stations with different register types.
"""

import json
import sys

def main():
    # Define multiple stations with different register types
    stations = [
        {
            "id": 1,
            "mode": "master",
            "map": {
                "holding": [
                    {
                        "address_start": 0,
                        "length": 5,
                        "initial_values": [1000, 2000, 3000, 4000, 5000]
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
        },
        {
            "id": 3,
            "mode": "master",
            "map": {
                "input": [
                    {
                        "address_start": 10,
                        "length": 4,
                        "initial_values": [111, 222, 333, 444]
                    }
                ]
            }
        }
    ]
    
    # Output JSON
    output = {
        "stations": stations,
        "reboot_interval": 1500
    }
    
    print(json.dumps(output))
    
    sys.stderr.write(f"Multi-station script executed: {len(stations)} stations\n")

if __name__ == "__main__":
    main()
