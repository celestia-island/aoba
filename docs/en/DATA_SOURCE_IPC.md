# IPC Communication (Custom Data Source)

## Quick start — run a small CLI receiver

For data-source mode `ipc:<path>` the CLI reads JSON lines from a named pipe (FIFO) or regular file. To start a small CLI receiver that reads from a FIFO, do the following:

```bash
# create a FIFO (one-time)
mkfifo /tmp/aoba_ipc.pipe

# start the CLI receiver (it will read lines from the FIFO path)
cargo run --bin aoba -- --master-provide-persist /tmp/vcom1 --data-source ipc:/tmp/aoba_ipc.pipe \
  --register-mode holding --register-address 0 --register-length 10

# then, from another shell, write a JSON line into the pipe:
echo '{"source":"ipc","type":"downlink","body":{"command":"ping"}}' > /tmp/aoba_ipc.pipe
```

Note: the repository also uses Unix domain sockets / named pipes for other IPC (TUI↔CLI). The `ipc:<path>` data-source mode specifically expects a FIFO/file path that the CLI can open and read line-by-line.

## Overview

This document describes how the application accepts custom data via IPC (inter-process communication). In the repository/application design the application acts as the IPC listener (server); third-party integrations or helper programs should act as the client and send JSON messages to the application's socket. Below are client-only examples (Rust/Python/Node) showing how to connect and send a message.

## When to use IPC

- Local integrations where network overhead is unnecessary
- Fast, low-latency communication between processes on the same host
- Test harnesses and E2E setups that spawn helper processes

## Message shape (recommended)

Use JSON for portability. Example message:

```json
{
  "source": "ipc",
  "timestamp": "2025-11-15T12:34:56Z",
  "port": "/tmp/vcom1",
  "type": "downlink",
  "body": { "command": "write_register", "registers": [{"address":0, "value":"1234"}] }
}
```

## Unix domain socket: Rust example (using `interprocess`)

Add dependency in `Cargo.toml`:

```toml
[dependencies]
interprocess = "*"
```

The application listens on a Unix-domain socket (for example `/tmp/aoba_ipc.sock`). The following Rust example shows how a client can connect to that socket and send a single JSON message.

Client (connect & send):

```rust
use std::io::{Read, Write};
use interprocess::local_socket::LocalSocketStream;

fn main() -> std::io::Result<()> {
    let mut stream = LocalSocketStream::connect("/tmp/aoba_ipc.sock")?;
    let msg = r#"{"source":"ipc","type":"downlink","body":{"command":"ping"}}"#;
    stream.write_all(msg.as_bytes())?;
    let mut resp = String::new();
    stream.read_to_string(&mut resp)?;
    println!("Response: {}", resp);
    Ok(())
}
```

Notes:

- The application is expected to create and bind the socket (the listener). Client programs should not try to bind the same path — they only connect.
- If you control both sides for tests, you may run a small listener locally; for production the app provides the socket path.
- On Windows use Named Pipes (path like `\\.\pipe\aoba_ipc`) or use `interprocess` cross-platform APIs.

## Python example (AF_UNIX)

The application creates and binds the Unix-domain socket; the following Python snippet shows how a client connects and sends a JSON message to the application's socket path.

Client:

```python
import socket
PATH = '/tmp/aoba_ipc.sock'
cli = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
cli.connect(PATH)
cli.sendall(b'{"source":"ipc","type":"downlink","body":{"command":"ping"}}')
resp = cli.recv(65536)
print('Response:', resp)
cli.close()
```

## Node.js (ES6) example — UNIX domain socket

The application listens on the socket path; the following Node.js snippet shows a client that connects and sends a JSON message.

Client:

```javascript
import net from 'net';

const PATH = '/tmp/aoba_ipc.sock';
const client = net.createConnection({ path: PATH }, () => {
  client.write(JSON.stringify({ source: 'ipc', type: 'downlink', body: { command: 'ping' } }));
});

client.on('data', (data) => {
  console.log('Response:', data.toString());
  client.end();
});
```

## Cross-platform notes

- On Windows use Named Pipes (`\\.\pipe\<name>`). Node and Python both have libraries to work with named pipes; Rust can use `interprocess` for cross-platform pipes.
- Ensure socket file permissions allow the processes to connect.

If you want, I can provide a small test harness that spawns the server and client and demonstrates end-to-end JSON roundtrip in your preferred language.
