# Custom Data Source — MQTT

## Quick start — run a small CLI receiver

Start the application's CLI so it subscribes to an MQTT topic and acts as the receiver. Example (run from the repository root):

```bash
# using cargo (recommended during development)
cargo run --bin aoba -- --master-provide-persist /tmp/vcom1 \
  --register-mode holding --register-address 0 --register-length 10 \
  --data-source mqtt://localhost:1883/aoba/data/in

# or, if you built the binary:
./target/debug/aoba --master-provide-persist /tmp/vcom1 --data-source mqtt://localhost:1883/aoba/data/in
```

The `mqtt://.../<topic>` URL includes the topic path (e.g. `aoba/data/in`) and the CLI will subscribe to that topic.

## Overview

 This document describes how to publish messages to the application's MQTT-based custom data source. It includes broker/connection configuration, recommended topic names, and an example `mosquitto_pub` payload to perform a data downlink.

## Broker / connection

- Host: `mqtt.example.com` or `localhost`
- Port: `1883` (plaintext) or `8883` (TLS)
- Username/password: optional — if your broker requires auth, provide them in client configuration
- TLS: if using `8883`, provide CA cert and client cert/key where required

## Recommended topics

- Inbound (to app): `aoba/data/in` — app subscribes here to receive upstream data or commands
- Downlink (to device/vcom): `aoba/data/out/<port>` — app publishes processed downlink messages targeted at a specific port (e.g. `aoba/data/out/tmp_vcom1`)

## Payload format

 The application expects JSON payloads. The exact schema is flexible but the following example is a practical shape for both status updates and downlink commands:

 ```json
 {
   "source": "mqtt",
   "timestamp": "2025-11-15T12:34:56Z",
   "port": "/tmp/vcom1",
   "type": "downlink",
   "body": {
     "command": "write_register",
     "registers": [{"address":0, "value": "1234"}]
   }
 }
 ```

## Example: publish a downlink using mosquitto_pub

 This example publishes a downlink to the inbound topic that the app will process and then perform the physical write to the configured `port`.

 ```bash
 mosquitto_pub -h localhost -p 1883 -t "aoba/data/in" -u "user" -P "pass" -m '{
   "source":"mqtt",
   "timestamp":"2025-11-15T12:34:56Z",
   "port":"/tmp/vcom1",
   "type":"downlink",
   "body":{ "command":"write_register", "registers":[{"address":0,"value":"1234"}] }
 }'
 ```

## Notes and tips

- Use predictable topic names to simplify filtering and permissions.
- When targeting a physical serial port path (e.g. `/tmp/vcom1`) avoid characters that may cause topic parsing issues; you can map port names to topic-safe labels in configuration.
- If your broker supports retained messages, be cautious: retained downlink messages can be re-applied on reconnect.

If you want a sample broker config or an automated test harness (e.g. small script that publishes a sequence of downlinks and waits for CLI/TUI status confirmation), tell me your preferred tooling and I can add it
