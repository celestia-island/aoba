# API Master Example

This example demonstrates how to use the Aoba Modbus API to create a custom master (client) that polls a slave for data.

## Features

- Uses the trait-based Modbus API
- Implements `ModbusDataSource` for providing fixed test data
- Implements `ModbusHook` for logging operations
- Demonstrates the iterator-like interface for receiving responses

## Usage

### Basic Usage

```bash
# Start a slave on /tmp/vcom2
cargo run --package aoba -- --slave-listen-persist /tmp/vcom2 --station-id 1 --register-address 0 --register-length 5 --register-mode holding

# In another terminal, run the API master
cargo run --package api_master /tmp/vcom1
```

### Custom Port

```bash
cargo run --package api_master -- /path/to/port
```

## How It Works

1. **Fixed Test Data Source**: The example includes a `FixedTestDataSource` that cycles through predefined test patterns:
   - Pattern 1: `[0x0001, 0x0002, 0x0003, 0x0004, 0x0005]`
   - Pattern 2: `[0x0010, 0x0020, 0x0030, 0x0040, 0x0050]`
   - Pattern 3: `[0x0100, 0x0200, 0x0300, 0x0400, 0x0500]`
   - Pattern 4: `[0x1000, 0x2000, 0x3000, 0x4000, 0x5000]`

2. **Logging Hook**: The `LoggingHook` logs all operations including:
   - Before each poll request
   - After receiving responses
   - On errors

3. **Response Handling**: The master polls the slave every second and logs the received values. After 10 successful responses, the example exits.

## API Components Used

- `ModbusBuilder::new_master()` - Configure master settings
- `ModbusDataSource` - Trait for providing data to write (optional)
- `ModbusHook` - Trait for logging and monitoring operations
- `ModbusMaster::start()` - Start the master polling loop
- `ModbusMaster::recv_timeout()` - Receive responses with timeout

## See Also

- [api_slave](../api_slave) - Corresponding slave example
- [Aoba API Documentation](../../src/api/modbus)
