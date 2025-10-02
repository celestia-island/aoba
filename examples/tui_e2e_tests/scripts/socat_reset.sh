#!/usr/bin/env bash

# Script to reset/recreate socat virtual serial ports between tests
# This ensures clean state and prevents port reuse issues

# If the script is launched with /bin/sh (dash) it will not support
# 'set -o pipefail'. Re-exec using bash to ensure bash-specific features work.
if [ -z "${BASH_VERSION:-}" ]; then
  if command -v bash >/dev/null 2>&1; then
    exec bash "$0" "$@"
  else
    echo "This script requires bash; please install bash or run with bash." >&2
    exit 1
  fi
fi

set -euo pipefail

V1=/dev/vcom1
V2=/dev/vcom2
PIDFILE=/var/run/socat_vcom.pid
LOG=/tmp/socat_vcom.log

echo "[socat_reset] resetting virtual serial ports"

# Stop existing socat process
if [ -f "$PIDFILE" ]; then
  SOCAT_PID=$(cat "$PIDFILE" 2>/dev/null || echo "")
  if [ -n "$SOCAT_PID" ]; then
    echo "[socat_reset] killing existing socat process: $SOCAT_PID"
    sudo kill "$SOCAT_PID" 2>/dev/null || true
    sleep 0.5
  fi
  sudo rm -f "$PIDFILE" || true
fi

# Kill any lingering socat processes
echo "[socat_reset] killing lingering socat processes"
sudo pkill -f "/dev/vcom" || true
sleep 0.5

# Remove existing symlinks
echo "[socat_reset] removing existing links: $V1 $V2"
sudo rm -f "$V1" "$V2" || true
sleep 0.2

# Start new socat process
echo "[socat_reset] starting fresh socat instance"
sudo rm -f "$LOG" || true
sudo bash -lc "setsid socat -d -d PTY,link=$V1,raw,echo=0,mode=0666 PTY,link=$V2,raw,echo=0,mode=0666 2> $LOG >/dev/null & echo \$! > $PIDFILE" || {
  echo "[socat_reset] failed to start socat"
  exit 1
}

SOCAT_PID=$(cat "$PIDFILE" 2>/dev/null || echo "")
echo "[socat_reset] socat pid: ${SOCAT_PID:-unknown}, waiting for $V1 and $V2"

# Wait for ports to be created
timeout=15
count=0
while [ $count -lt $timeout ] && ( [ ! -e "$V1" ] || [ ! -e "$V2" ] ); do
  sleep 1
  count=$((count + 1))
done

if [ -e "$V1" ] && [ -e "$V2" ]; then
  echo "[socat_reset] successfully recreated virtual ports"
  ls -la "$V1" "$V2" || true
  
  # Ensure underlying pts are accessible
  P1=$(readlink -f "$V1" || true)
  P2=$(readlink -f "$V2" || true)
  if [ -n "$P1" ]; then sudo chmod 666 "$P1" || true; fi
  if [ -n "$P2" ]; then sudo chmod 666 "$P2" || true; fi
  echo "[socat_reset] underlying pts:"; ls -la "$P1" "$P2" || true
  
  # Quick connectivity test
  TMP_OUT=$(mktemp /tmp/socat_reset_test.XXXXXX) || TMP_OUT="/tmp/socat_reset_test.$$"
  TEST_STR="reset-test-$$-$(date +%s)"
  
  timeout 5 bash -c "cat '$V1' > '$TMP_OUT'" &
  READER_PID=$!
  sleep 0.2
  printf "%s" "$TEST_STR" > "$V2" || true
  sleep 0.6
  kill $READER_PID 2>/dev/null || true
  wait $READER_PID 2>/dev/null || true
  
  if grep -q "$TEST_STR" "$TMP_OUT" 2>/dev/null; then
    echo "[socat_reset] connectivity test passed"
    rm -f "$TMP_OUT" || true
    exit 0
  else
    echo "[socat_reset] connectivity test FAILED"
    rm -f "$TMP_OUT" || true
    exit 2
  fi
else
  echo "[socat_reset] Failed to recreate virtual ports within ${timeout}s"
  echo "[socat_reset] socat log ($LOG):"; sudo tail -n 50 "$LOG" || true
  exit 1
fi
