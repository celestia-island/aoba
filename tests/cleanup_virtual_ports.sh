#!/bin/bash
# Cleanup virtual serial ports
# Usage: ./cleanup_virtual_ports.sh <port1_name>
# Example: ./cleanup_virtual_ports.sh /tmp/vcom1

PORT1=${1:-/tmp/vcom1}
PID_FILE="/tmp/socat_pid_${PORT1##*/}"

echo "Cleaning up virtual serial ports for $PORT1"

if [ -f "$PID_FILE" ]; then
    SOCAT_PID=$(cat "$PID_FILE")
    echo "Killing socat process $SOCAT_PID"
    kill "$SOCAT_PID" 2>/dev/null || true
    rm -f "$PID_FILE"
else
    echo "No PID file found, attempting to kill all socat processes"
    pkill socat || true
fi

# Remove port files if they exist
rm -f "$PORT1" "${PORT1%1}2" 2>/dev/null || true

echo "Cleanup complete"