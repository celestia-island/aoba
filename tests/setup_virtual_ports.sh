#!/bin/bash
# Setup virtual serial ports for testing
# Usage: ./setup_virtual_ports.sh <port1_name> <port2_name>
# Example: ./setup_virtual_ports.sh /tmp/vcom1 /tmp/vcom2

set -e

PORT1=${1:-/tmp/vcom1}
PORT2=${2:-/tmp/vcom2}

echo "Setting up virtual serial port pair: $PORT1 <-> $PORT2"

# Create virtual serial port pair
socat -d -d pty,raw,echo=0,link=$PORT1 pty,raw,echo=0,link=$PORT2 &
SOCAT_PID=$!

# Wait for ports to be created
echo "Waiting for ports to be created..."
timeout 10 bash -c "while [ ! -e $PORT1 ] || [ ! -e $PORT2 ]; do sleep 0.1; done"

# Verify ports were created
echo "Verifying ports:"
ls -la $PORT1 $PORT2

# Make ports accessible
echo "Setting permissions..."
sudo chmod 666 $PORT1 $PORT2 || true

echo "Virtual serial ports setup complete!"
echo "SOCAT_PID=$SOCAT_PID"
echo "To cleanup, run: kill $SOCAT_PID"

# Export PID for caller to use
echo $SOCAT_PID > /tmp/socat_pid_${PORT1##*/}