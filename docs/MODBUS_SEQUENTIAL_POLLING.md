# Modbus Sequential Polling Behavior

## Overview
This document describes the fixed sequential polling behavior for Modbus Slave mode (Master/Client).

## Key Principles

1. **Sequential Processing**: Only ONE station is processed at a time
2. **Wait for Response**: Don't move to next station until current one succeeds OR times out
3. **1-Second Minimum Interval**: Enforce minimum 1 second between consecutive requests
4. **3-Second Timeout**: Wait up to 3 seconds for response before retry
5. **Stay on Timeout**: On timeout, retry the SAME station (don't skip to next)

## Example Timeline

### Scenario: 2 stations, 1-second interval, 3-second timeout

```
Station 0: ID=1, Register=100
Station 1: ID=2, Register=200

Time    Action                          Current Station
----    ------                          ---------------
0.0s    Send request to Station 0       0 (waiting...)
0.5s    Receive response from Station 0 0 → 1 (success!)
1.0s    Send request to Station 1       1 (waiting...)
1.5s    Receive response from Station 1 1 → 0 (success!)
2.0s    Send request to Station 0       0 (waiting...)
2.5s    Receive response from Station 0 0 → 1 (success!)
3.0s    Send request to Station 1       1 (waiting...)
6.0s    TIMEOUT (3s) on Station 1       1 (retry)
6.0s    (no send yet, waiting for next_poll_at)
7.0s    Send request to Station 1       1 (waiting... retry)
7.2s    Receive response from Station 1 1 → 0 (success!)
8.0s    Send request to Station 0       0 (waiting...)
...
```

## State Machine

```
              ┌─────────────┐
              │   IDLE      │
              └──────┬──────┘
                     │ now >= next_poll_at
                     │ && !pending_request
                     ▼
              ┌─────────────┐
              │ SEND REQUEST│
              └──────┬──────┘
                     │ last_request_time = Some(now)
                     │ next_poll_at = now + 1s
                     ▼
              ┌─────────────┐
         ┌───►│   WAITING   │────┐
         │    └─────────────┘    │
         │           │            │
         │ TIMEOUT   │            │ RESPONSE
         │ (3s)      │            │ RECEIVED
         │           ▼            ▼
         │    ┌─────────────┬─────────────┐
         │    │   TIMEOUT   │   SUCCESS   │
         │    └──────┬──────┴──────┬──────┘
         │           │              │
         │           │              │ Move to next station
         │           │              │ (idx+1) % stations.len()
         │           ▼              ▼
         └────  STAY ON SAME    GO TO NEXT
                  STATION         STATION
                     │              │
                     └──────┬───────┘
                            │ Wait for next_poll_at
                            ▼
                     ┌─────────────┐
                     │   IDLE      │
                     └─────────────┘
```

## Master Mode (Slave/Server) Throttling

For Master mode (which acts as Modbus Slave/Server):

1. **Reactive Only**: Never proactively send data
2. **1-Second Throttle**: Same station+register can only be responded to once per second
3. **Cache Last Response Time**: Track `last_response_time` per station/register
4. **Drop Requests Within Throttle Window**: Silently ignore requests that come too fast

Example:
```
Time    Event                           Action
----    -----                           ------
0.0s    Request from Station 1, Reg 100 → Respond
0.3s    Request from Station 1, Reg 100 → THROTTLED (skip)
0.7s    Request from Station 1, Reg 100 → THROTTLED (skip)
1.0s    Request from Station 1, Reg 100 → Respond (1s elapsed)
1.2s    Request from Station 2, Reg 200 → Respond (different station)
```

## Configuration via Environment Variable

Set log output location:
```bash
export AOBA_LOG_FILE=/tmp/aoba_debug.log
./aoba --tui
```

Analyze recent log entries:
```bash
tail -n 100 /tmp/aoba_debug.log  # Last 100 lines
grep "Timeout" /tmp/aoba_debug.log  # Find timeouts
grep "✅" /tmp/aoba_debug.log  # Find successful responses
```
