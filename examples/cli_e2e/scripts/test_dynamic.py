#!/usr/bin/env python3
"""
Test script for external Python data source with dynamic values.
Generates random sensor data for testing.
"""

import json
import random
import sys

def main():
    # Generate random sensor readings
    temperature = random.randint(200, 300)  # 20.0 - 30.0 °C (×10)
    humidity = random.randint(400, 600)     # 40.0 - 60.0 % (×10)
    pressure = random.randint(9800, 10200)  # 980 - 1020 hPa (×10)
    
    # Define station configuration
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
        "reboot_interval": 2000  # Execute every 2 seconds
    }
    
    print(json.dumps(output))
    
    # Debug info
    sys.stderr.write(f"Generated sensor data: T={temperature/10:.1f}°C, H={humidity/10:.1f}%, P={pressure/10:.1f}hPa\n")

if __name__ == "__main__":
    main()
