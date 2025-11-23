# API Slave Example

This example demonstrates how to use the Aoba Modbus API to create a custom slave (server) that listens for requests and provides responses.

## Features

- Uses the trait-based Modbus API
- Implements `ModbusHook` for logging operations
- Demonstrates the iterator-like interface for receiving request notifications
- Automatic register value management

## Usage

### Basic Usage

```bash
# Start the API slave on /tmp/vcom1
cargo run --package api_slave /tmp/vcom1

# In another terminal, poll the slave with CLI
cargo run --package aoba -- --slave-poll /tmp/vcom2 --station-id 1 --register-address 0 --register-length 5 --register-mode holding --json
```

### Custom Port

```bash
cargo run --package api_slave -- /path/to/port
```

## How It Works

1. **Automatic Storage**: The slave uses `rmodbus` internal storage for register values. Initial values are all zeros.

2. **Request Processing**: The slave listens for incoming Modbus requests:
   - Read requests: Returns current register values
   - Write requests: Updates register values in storage

3. **Logging Hook**: The `LoggingHook` logs all operations including:
   - Before processing each request
   - After sending responses
   - On errors

4. **Iterator Interface**: The slave provides an iterator-like interface to receive notifications of processed requests. After 10 requests, the example exits.

## API Components Used

- `ModbusBuilder::new_slave()` - Configure slave settings
- `ModbusHook` - Trait for logging and monitoring operations
- `ModbusSlave::start()` - Start the slave listening loop
- `ModbusSlave::recv_timeout()` - Receive request notifications with timeout

## Configuration

The example uses the following default configuration:
- Station ID: 1
- Register Mode: Holding Registers
- Register Address: 0x0000
- Register Length: 5
- Initial Values: `[0x0000, 0x0000, 0x0000, 0x0000, 0x0000]`

## See Also

- [api_master](../api_master) - Corresponding master example
- [Aoba API Documentation](../../src/api/modbus)
