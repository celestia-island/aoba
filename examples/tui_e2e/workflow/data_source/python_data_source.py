#!/usr/bin/env python3
"""
Python script data source for Modbus master testing.
This script outputs register data in JSON format when called.
Usage: python python_data_source.py [round_number]
"""

import json
import sys


def get_registers(round_num=1):
    """Get register values based on test round."""
    if round_num == 1:
        # Round 1: Sequential 0-9
        return list(range(10))
    elif round_num == 2:
        # Round 2: Reverse 9-0
        return list(range(9, -1, -1))
    elif round_num == 3:
        # Round 3: Custom pattern
        return [0x1111, 0x2222, 0x3333, 0x4444, 0x5555, 0x6666, 0x7777, 0x8888, 0x9999, 0xAAAA]
    else:
        # Default: zeros
        return [0] * 10


def main():
    """Main function to output register data."""
    round_num = 1
    if len(sys.argv) > 1:
        try:
            round_num = int(sys.argv[1])
        except ValueError:
            round_num = 1

    registers = get_registers(round_num)

    # Output in JSON format
    data = {
        "station_id": 1,
        "register_type": "Holding",
        "start_address": 0,
        "registers": registers
    }

    print(json.dumps(data))


if __name__ == "__main__":
    main()
