# HTTP Server and IPC Channel Features

This document describes the HTTP server and IPC channel features added to support virtual serial port data retrieval and external data sources.

## Overview

The Aoba CLI now supports two new modes for data exchange:

1. **HTTP Server Mode**: HTTP GET/POST endpoints for retrieving and uploading station data
2. **IPC Channel Mode**: Unix socket server with half-duplex JSON request-response protocol

## HTTP Server Mode

### Description

When using `--data-source http://<port>` with `--master-provide-persist`, an HTTP server is started on the specified port. This server accepts both GET and POST requests to the root endpoint `/`.

### Endpoints

#### GET / - Retrieve Station Data

Retrieves current station configurations with live register values from Modbus storage.

**Request:**
```bash
curl http://localhost:8080/
```

**Response:**
```json
{
  "success": true,
  "message": "Retrieved 2 stations",
  "stations": [
    {
      "id": 1,
      "mode": "master",
      "map": {
        "holding": [
          {
            "address_start": 0,
            "length": 10,
            "initial_values": [100, 101, 102, 103, 104, 105, 106, 107, 108, 109]
          }
        ],
        "coils": [],
        "discrete_inputs": [],
        "input": []
      }
    },
    {
      "id": 2,
      "mode": "master",
      "map": {
        "holding": [
          {
            "address_start": 0,
            "length": 5,
            "initial_values": [200, 201, 202, 203, 204]
          }
        ],
        "coils": [],
        "discrete_inputs": [],
        "input": []
      }
    }
  ]
}
```

#### POST / - Upload Station Configuration

Uploads new station configurations and updates the internal storage.

**Request:**
```bash
curl -X POST http://localhost:8080/ \
  -H "Content-Type: application/json" \
  -d '[
    {
      "id": 1,
      "mode": "master",
      "map": {
        "holding": [
          {
            "address_start": 0,
            "length": 10,
            "initial_values": [10, 20, 30, 40, 50, 60, 70, 80, 90, 100]
          }
        ],
        "coils": [],
        "discrete_inputs": [],
        "input": []
      }
    }
  ]'
```

**Response:**
```json
{
  "success": true,
  "message": "Stations queued",
  "stations": [
    {
      "id": 1,
      "mode": "master",
      "map": {
        "holding": [
          {
            "address_start": 0,
            "length": 10,
            "initial_values": [10, 20, 30, 40, 50, 60, 70, 80, 90, 100]
          }
        ],
        "coils": [],
        "discrete_inputs": [],
        "input": []
      }
    }
  ]
}
```

### Usage Example

Start a master in persist mode with HTTP data source:

```bash
cargo run -- \
  --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 10 \
  --register-mode holding \
  --data-source http://8080 \
  --baud-rate 9600
```

In another terminal, upload configuration:

```bash
curl -X POST http://localhost:8080/ \
  -H "Content-Type: application/json" \
  -d '[{"id":1,"mode":"master","map":{"holding":[{"address_start":0,"length":10,"initial_values":[1,2,3,4,5,6,7,8,9,10]}],"coils":[],"discrete_inputs":[],"input":[]}}]'
```

Query current data:

```bash
curl http://localhost:8080/
```

## IPC Channel Mode

### Description

IPC Channel mode creates a Unix domain socket server that accepts multiple concurrent connections. Each connection operates in half-duplex mode: the client sends a JSON request, the server processes one Modbus transaction, and returns a JSON response.

### Protocol

- **Transport**: Unix domain socket (file-based or abstract namespace)
- **Format**: Line-based JSON (one request/response per line, terminated with `\n`)
- **Mode**: Half-duplex (one request → one response)
- **Concurrency**: Multiple clients can connect simultaneously

### Message Format

#### Request

Any valid JSON object. The server validates JSON but doesn't require specific fields:

```json
{"action": "read"}
```

or even just:

```json
{}
```

#### Success Response

```json
{
  "success": true,
  "data": {
    "station_id": 1,
    "register_address": 0,
    "register_mode": "Holding",
    "values": [100, 101, 102, 103, 104, 105, 106, 107, 108, 109],
    "timestamp": "2025-01-15T10:30:45.123Z"
  }
}
```

#### Error Response

```json
{
  "success": false,
  "error": "No data received"
}
```

### Usage Example

Start a slave in persist mode with IPC socket:

```bash
cargo run -- \
  --slave-listen-persist /dev/ttyUSB0 \
  --ipc-socket-path /tmp/modbus.sock \
  --station-id 1 \
  --register-address 0 \
  --register-length 10 \
  --register-mode holding \
  --baud-rate 9600
```

Connect and send requests using `nc` (netcat):

```bash
echo '{"action":"read"}' | nc -U /tmp/modbus.sock
```

Or using `socat`:

```bash
echo '{"action":"read"}' | socat - UNIX-CONNECT:/tmp/modbus.sock
```

### Multi-Connection Support

The IPC server can handle multiple concurrent connections. Each connection is processed in a separate async task:

**Terminal 1:**
```bash
socat - UNIX-CONNECT:/tmp/modbus.sock
{"action":"read"}
# waits for response...
```

**Terminal 2 (simultaneously):**
```bash
socat - UNIX-CONNECT:/tmp/modbus.sock
{"action":"read"}
# waits for response...
```

Both clients will receive responses independently as Modbus transactions complete.

## Architecture

### HTTP Server

- Runs in a background async task managed by `http_daemon_registry`
- Shares Modbus storage via `Arc<Mutex<ModbusStorageSmall>>`
- Tracks station configurations in `Arc<Mutex<Vec<StationConfig>>>`
- GET reads current values from storage for configured stations
- POST updates both tracked configuration and storage values

### IPC Channel Server

- Runs in the main loop, accepting connections with `listener.accept()`
- Each accepted connection spawned as independent task via `task_manager::spawn_task()`
- Connection handler uses line-based JSON over `BufReader`
- Calls `listen_for_one_request()` to process Modbus transaction per request
- Automatic cleanup of socket file on Unix systems

## Implementation Details

### Code Changes

1. **`src/cli/modbus/master.rs`**:
   - Added `handle_stations_get()` for HTTP GET endpoint
   - Enhanced `HttpServerState` with `stations` field to track configurations
   - Updated `handle_stations_post()` to store received configurations
   - Modified router to support both GET and POST on "/" endpoint

2. **`src/cli/modbus/slave.rs`**:
   - Added `handle_slave_listen_ipc_channel()` for IPC socket server
   - Added `handle_ipc_connection()` for per-connection processing
   - Implements half-duplex JSON request-response protocol
   - Supports multiple concurrent connections via task spawning

3. **`src/cli/mod.rs`**:
   - Added `--ipc-socket-path` CLI argument (requires `--slave-listen-persist`)

4. **`src/cli/actions.rs`**:
   - Updated dispatch logic to check for IPC socket path
   - Routes to IPC channel handler when socket path is provided

### Error Handling

- Invalid JSON requests return error response without closing connection
- Connection errors are logged with connection ID
- Modbus transaction errors wrapped in JSON error response
- Socket binding errors propagated with cleanup

### Cleanup

- HTTP server: Registered with `http_daemon_registry` for shutdown/join
- IPC socket: Existing socket file removed before binding (Unix)
- Serial port: Cleanup handler registered to flush and release port

## Future Enhancements

1. **Request Types**: Define specific JSON request schemas for different operations
2. **Streaming**: Support streaming mode for continuous data updates
3. **Authentication**: Add authentication for HTTP/IPC connections
4. **Compression**: Support compressed payloads for large station configurations
5. **WebSocket**: Add WebSocket support for bi-directional real-time updates

## Testing

### HTTP Server Testing

```bash
# Start server
cargo run -- --master-provide-persist /tmp/vcom1 --data-source http://8080 --station-id 1 --register-address 0 --register-length 10 --register-mode holding

# Test POST
curl -X POST http://localhost:8080/ -H "Content-Type: application/json" -d '[{"id":1,"mode":"master","map":{"holding":[{"address_start":0,"length":10,"initial_values":[1,2,3,4,5,6,7,8,9,10]}],"coils":[],"discrete_inputs":[],"input":[]}}]'

# Test GET
curl http://localhost:8080/
```

### IPC Channel Testing

```bash
# Start server
cargo run -- --slave-listen-persist /tmp/vcom1 --ipc-socket-path /tmp/modbus.sock --station-id 1 --register-address 0 --register-length 10 --register-mode holding

# Test single request
echo '{}' | nc -U /tmp/modbus.sock

# Test multiple concurrent connections
for i in {1..5}; do
  (echo "{\"client\":$i}" | nc -U /tmp/modbus.sock) &
done
wait
```

## Troubleshooting

### HTTP Server

**Issue**: Port already in use
```
Failed to bind HTTP server to 127.0.0.1:8080: Address already in use
```
**Solution**: Choose a different port or kill the process using the port

**Issue**: No data returned on GET
```json
{"success": true, "message": "Retrieved 0 stations", "stations": []}
```
**Solution**: Send a POST request first to configure stations

### IPC Channel

**Issue**: Socket file already exists
```
Socket address already in use: /tmp/modbus.sock
```
**Solution**: The socket file is automatically removed, but if it persists, manually remove it:
```bash
rm /tmp/modbus.sock
```

**Issue**: Permission denied on socket
```
Failed to create listener for /tmp/modbus.sock: Permission denied
```
**Solution**: Ensure write permissions to the socket directory

**Issue**: Connection immediately closed
**Solution**: Check logs for JSON parsing errors or Modbus transaction failures

## References

- Axum HTTP framework: https://docs.rs/axum/
- Interprocess crate (Unix sockets): https://docs.rs/interprocess/
- Modbus protocol: https://modbus.org/
