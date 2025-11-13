"""
Type stubs for the aoba module (RustPython embedded mode).

This module is only available when using the embedded RustPython mode.
It provides functions to configure Modbus stations and execution intervals.

Example usage:
    import aoba
    import json

    # Define station configuration as JSON
    stations_json = json.dumps([{
        "id": 1,
        "mode": "master",
        "map": {
            "holding": [{
                "address_start": 0,
                "length": 10,
                "initial_values": [100, 200, 300, 400, 500]
            }]
        }
    }])

    # Push station configuration
    aoba.push_stations(stations_json)

    # Set reboot interval (milliseconds)
    aoba.set_reboot_interval(1000)
"""

from typing import List, Dict, Union


def push_stations(stations_json: str) -> None:
    """
    Push station configurations to the Modbus system.

    This function accepts a JSON string containing an array of station configurations.
    Each station must have an id, mode, and map with register configurations.

    Args:
        stations_json: JSON string representing a list of StationConfig objects.
                      Must conform to the StationConfig schema.

    Raises:
        TypeError: If stations_json is not a valid JSON string
        ValueError: If the JSON does not match the expected StationConfig schema

    Example:
        >>> import json
        >>> stations = [{
        ...     "id": 1,
        ...     "mode": "master",
        ...     "map": {
        ...         "holding": [{
        ...             "address_start": 0,
        ...             "length": 10,
        ...             "initial_values": [100, 200, 300]
        ...         }]
        ...     }
        ... }]
        >>> aoba.push_stations(json.dumps(stations))

    StationConfig JSON Schema:
        {
            "id": int,              # Station ID (1-255)
            "mode": str,            # "master" or "slave"
            "map": {
                "coils"?: [{        # Optional coil registers (read/write bits)
                    "address_start": int,
                    "length": int,
                    "initial_values"?: List[int]  # 0 or 1
                }],
                "discrete_inputs"?: [{  # Optional discrete input registers (read-only bits)
                    "address_start": int,
                    "length": int,
                    "initial_values"?: List[int]  # 0 or 1
                }],
                "holding"?: [{       # Optional holding registers (read/write 16-bit)
                    "address_start": int,
                    "length": int,
                    "initial_values"?: List[int]  # 0-65535
                }],
                "input"?: [{         # Optional input registers (read-only 16-bit)
                    "address_start": int,
                    "length": int,
                    "initial_values"?: List[int]  # 0-65535
                }]
            }
        }
    """
    ...


def set_reboot_interval(interval_ms: int) -> None:
    """
    Set the reboot interval for script re-execution.

    This determines how long (in milliseconds) the system will wait before
    executing the Python script again. Lower values mean more frequent updates,
    but higher CPU usage.

    Args:
        interval_ms: Interval in milliseconds (must be non-negative).
                    Recommended minimum: 100ms
                    Default if not called: 1000ms

    Raises:
        ValueError: If interval_ms is negative

    Example:
        >>> # Execute script every 5 seconds
        >>> aoba.set_reboot_interval(5000)
        >>> 
        >>> # Execute script every 100 milliseconds (high frequency)
        >>> aoba.set_reboot_interval(100)

    Note:
        - Values below 100ms may cause high CPU usage
        - The actual execution frequency depends on script execution time
        - If the script takes longer than the interval, the next execution
          will start immediately after the previous one completes
    """
    ...


# Type aliases for better documentation
StationId = int  # Range: 1-255
RegisterAddress = int  # Range: 0-65535
RegisterValue = int  # Range: 0-65535 for registers, 0-1 for coils
RegisterMode = str  # "master" or "slave"


class StationConfig:
    """
    Type hint class for station configuration (for documentation only).

    This class is not actually available at runtime in the aoba module.
    It's provided here for type checking and IDE autocompletion.

    Use json.dumps() to convert a dictionary to the required JSON string format.
    """
    id: StationId
    mode: RegisterMode
    map: Dict[str, List[Dict[str, Union[int, List[int]]]]]
