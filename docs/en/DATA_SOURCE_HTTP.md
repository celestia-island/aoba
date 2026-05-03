# Custom Data Source — HTTP

## Quick start — run a small CLI receiver

Start the application's CLI in a mode that will act as the receiver (the CLI will host an HTTP endpoint and apply posted JSON). Example (run from the repository root):

```bash
# using cargo (recommended during development)
cargo run --bin aoba -- --master-provide-persist /tmp/vcom1 \
  --register-mode holding --register-address 0 --register-length 10 \
  --data-source http://8080

# or, if you built the binary:
./target/debug/aoba --master-provide-persist /tmp/vcom1 --data-source http://8080
```

The command above starts an HTTP server bound to `127.0.0.1:8080` and accepts `POST` requests to `/` (root). Use the `curl` example below to POST data.

## Overview

This document describes the HTTP custom data source used by the application. It shows the expected request shape, common headers, and a simple `curl` example you can use to quickly validate the integration.

## Endpoint

- Method: `POST`
- URL: `http://<host>:<port>/` (example: `http://localhost:8080/`)
- Content-Type: `application/json`

## Request format

The service accepts a JSON body. A minimal example payload looks like:

```json
{
  "source": "http",
  "timestamp": "2025-11-15T12:34:56Z",
  "port": "/tmp/vcom1",
  "payload": {
    "type": "register_update",
    "registers": [
      {"address": 0, "value": "1234"},
      {"address": 1, "value": "abcd"}
    ]
  }
}
```

Notes:

- Use ISO 8601 for `timestamp` when available.
- `payload` content is application-specific; the example above shows a common register-style update.

## Example curl test

Replace `<host>` and `<port>` with your running server. This `curl` command sends the JSON payload above:

```bash
curl -v -X POST "http://localhost:8080/" \
  -H "Content-Type: application/json" \
  -d '{
    "source":"http",
    "timestamp":"2025-11-15T12:34:56Z",
    "port":"/tmp/vcom1",
    "payload":{
      "type":"register_update",
      "registers":[{"address":0,"value":"1234"}]
    }
  }'
```

## Expected behavior

- HTTP `200 OK` (or `202 Accepted`) for accepted/queued messages.
- If the server returns an error (4xx/5xx), inspect the response body for details.

## Tips and troubleshooting

- Ensure `Content-Type: application/json` header is present.
- If your server requires authentication, add the appropriate `Authorization` header (e.g. `Bearer <token>`).
- For large payloads, consider testing with `--data-binary` and increasing server timeouts.

If you need a tailored example matching an internal schema, paste a sample JSON here and the developers will adapt the endpoint handler accordingly.
If you need a tailored example matching an internal schema, paste a sample JSON here and the developers will adapt the endpoint handler accordingly.
