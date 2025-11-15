# Port Forwarding (Transparent Port Forwarding Mode)

## Overview

Port Forwarding mode allows you to transparently forward Modbus data from one port to another within the TUI. This feature enables advanced use cases such as:

1. **Data Forwarding**: Convert a slave station from one port into a master station on another port
2. **Data Replication**: Duplicate master station data to multiple ports for monitoring or testing
3. **Protocol Bridge**: Bridge data between different physical ports or virtual connections

## When to use Port Forwarding

- **Multi-port Scenarios**: When you have multiple serial ports and need to share data between them
- **Testing**: Create test setups where one port simulates another port's behavior
- **Monitoring**: Replicate data from an active port to a monitoring port without disrupting the original connection
- **Data Aggregation**: Combine data from multiple sources by forwarding from different ports

## Configuration in TUI

### Step 1: Ensure Source Port is Running

Before configuring port forwarding, make sure the source port is already configured and running:

1. Navigate to the source port in the Entry page
2. Configure its Modbus stations (master or slave mode)
3. Save the configuration with `Ctrl+S` to enable the port
4. Verify the port shows "Running ●" status

### Step 2: Configure Target Port with Port Forwarding

1. Navigate to the target port (the one that will forward data)
2. Press `Enter` to enter the Config Panel
3. Navigate down to "Enter Business Configuration" and press `Enter`
4. Navigate down to "Data Source" field
5. Press `Enter` to edit the data source
6. Use arrow keys (`←` / `→`) to cycle through options until you reach "Port Forwarding"
7. Press `Enter` to confirm

### Step 3: Select Source Port

After selecting "Port Forwarding" as the data source:

1. Navigate down to "Source Port" field
2. Press `Enter` to open the port selector
3. Use arrow keys (`←` / `→`) to navigate through available ports
4. Press `Enter` to select the desired source port
5. Press `Ctrl+S` to save and enable forwarding

**Note**: If only one port exists (the current port), the "Source Port" field will show a greyed hint "No other ports available" and pressing `Enter` will do nothing.

### Step 4: Configure Station

Even with Port Forwarding enabled, you still need to configure at least one station on the target port:

1. Navigate to "Create Station"
2. Configure Station ID, Register Type, Start Address, and Register Count
3. The register values will be automatically populated from the source port's data

### Step 5: Save and Enable

1. Press `Ctrl+S` to save the configuration
2. The port will start running with "Running ●" status
3. Data from the source port will be periodically forwarded to this port

## How It Works

When Port Forwarding is enabled:

1. **Background Daemon**: TUI spawns a background thread dedicated to this port
2. **Periodic Reading**: The daemon periodically reads register values from the source port's global state
3. **State Synchronization**: The daemon updates the target port's register values via internal IPC
4. **Automatic Updates**: Changes in the source port are automatically reflected in the target port

The forwarding happens entirely within the TUI process, with no external network or serial communication required.

## Example Use Case: Multi-Master Setup

Suppose you have:

- `/tmp/vcom1`: Connected to a physical Modbus device as a slave
- `/tmp/vcom2`: You want to act as a master reading from vcom1

Configuration:

1. Configure `/tmp/vcom1`:
   - Mode: Slave
   - Configure slave stations to respond to Modbus requests

2. Configure `/tmp/vcom2`:
   - Mode: Master
   - Data Source: Port Forwarding
   - Source Port: `/tmp/vcom1`
   - Configure master stations

Result: `/tmp/vcom2` will act as a master, but its data comes from `/tmp/vcom1`'s slave responses, effectively forwarding the data.

## Example Use Case: Data Replication

Suppose you have:

- `/tmp/vcom1`: Main port reading from external IPC data source
- `/tmp/vcom2`: Monitoring port that needs to mirror vcom1's data

Configuration:

1. Configure `/tmp/vcom1`:
   - Mode: Master
   - Data Source: IPC Pipe (e.g., `/tmp/data_feed`)
   - Configure stations

2. Configure `/tmp/vcom2`:
   - Mode: Master
   - Data Source: Port Forwarding
   - Source Port: `/tmp/vcom1`
   - Configure stations with same register layout

Result: Both ports display the same data, with vcom2 mirroring vcom1's register values.

## Limitations

- **Source Port Must Be Running**: The source port must be enabled before forwarding can work
- **No Self-Forwarding**: A port cannot forward from itself
- **Master Mode Only**: Port Forwarding is only available for master stations
- **Internal Only**: Forwarding happens within TUI; external processes cannot directly forward ports

## Troubleshooting

### "No other ports available" message

This appears when:

- Only one port exists in the system (no source port to forward from)
- The current port is the only port
- **Solution**: Add another port first, configure and enable it, then set up forwarding

### Data not updating

Check:

- Source port is running (shows "Running ●" status)
- Source port has configured stations
- Target port is running (shows "Running ●" status)
- Both ports use compatible register types and address ranges

### Port forwarding not appearing in options

Make sure:

- You are configuring a master station (not slave)
- You are in the Modbus Dashboard panel
- You have navigated to the "Data Source" field

## Advanced: Multiple Forwarding Chains

You can create forwarding chains:

> Port A → Port B → Port C

However, be cautious:

- Each link in the chain introduces latency
- Circular forwarding (A → B → A) is prevented by the UI
- Monitor performance if using multiple forwarding levels

## See Also

- [IPC Data Source](DATA_SOURCE_IPC.md) - For external data integration
- [HTTP Data Source](DATA_SOURCE_HTTP.md) - For HTTP-based data sources
- [MQTT Data Source](DATA_SOURCE_MQTT.md) - For MQTT integration
