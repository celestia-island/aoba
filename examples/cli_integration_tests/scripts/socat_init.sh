#!/usr/bin/env bash

# Script to setup virtual serial ports for CLI E2E tests
# Similar to TUI E2E but for CLI testing

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
PIDFILE=/var/run/socat_vcom_cli.pid
LOG=/tmp/socat_vcom_cli.log

echo "[cli_socat_init] stopping existing socat (pidfile if present)"
if [ -f "$PIDFILE" ]; then
  sudo bash -lc "kill \$(cat $PIDFILE) 2>/dev/null || true; rm -f $PIDFILE || true"
fi

echo "[cli_socat_init] killing lingering socat processes that reference vcom links (if any)"
sudo pkill -f "/dev/vcom" || true
sleep 0.5

echo "[cli_socat_init] removing existing links: $V1 $V2"
sudo rm -f "$V1" "$V2" || true

echo "[cli_socat_init] starting socat with link and mode=0666"
sudo rm -f "$LOG" || true
sudo bash -lc "setsid socat -d -d PTY,link=$V1,raw,echo=0,mode=0666 PTY,link=$V2,raw,echo=0,mode=0666 2> $LOG >/dev/null & echo \$! > $PIDFILE" || {
  echo "[cli_socat_init] failed to start socat"
  exit 1
}

SOCAT_PID=$(cat "$PIDFILE" 2>/dev/null || echo "")
echo "[cli_socat_init] socat pid: ${SOCAT_PID:-unknown}, waiting for $V1 and $V2"

timeout=15
count=0
while [ $count -lt $timeout ] && ( [ ! -e "$V1" ] || [ ! -e "$V2" ] ); do
  sleep 1
  count=$((count + 1))
done

if [ -e "$V1" ] && [ -e "$V2" ]; then
  echo "[cli_socat_init] created links:" 
  ls -la "$V1" "$V2" || true
  # ensure underlying pts are also 666
  P1=$(readlink -f "$V1" || true)
  P2=$(readlink -f "$V2" || true)
  if [ -n "$P1" ]; then sudo chmod 666 "$P1" || true; fi
  if [ -n "$P2" ]; then sudo chmod 666 "$P2" || true; fi
  echo "[cli_socat_init] underlying pts:"; ls -la "$P1" "$P2" || true
else
  echo "[cli_socat_init] Failed to create /dev/vcom1 and /dev/vcom2 within ${timeout}s"
  echo "[cli_socat_init] socat log ($LOG):"; sudo tail -n 200 "$LOG" || true
  echo "[cli_socat_init] socat processes:"; ps aux | grep socat | grep -v grep || true
  exit 1
fi

# Connectivity test
echo "[cli_socat_init] performing connectivity test: write to $V2, read from $V1"
TMP_OUT=$(mktemp /tmp/socat_test.XXXXXX) || TMP_OUT="/tmp/socat_test.$$"
TEST_STR="socat-test-$$-$(date +%s)"

timeout 5 bash -c "cat '$V1' > '$TMP_OUT'" &
READER_PID=$!
sleep 0.2
printf "%s" "$TEST_STR" > "$V2" || true
sleep 0.6
kill $READER_PID 2>/dev/null || true
wait $READER_PID 2>/dev/null || true

if grep -q "$TEST_STR" "$TMP_OUT" 2>/dev/null; then
  echo "[cli_socat_init] connectivity test passed: data written to $V2 was received on $V1"
  rm -f "$TMP_OUT" || true
  echo "[cli_socat_init] finished successfully"
  exit 0
else
  echo "[cli_socat_init] connectivity test FAILED"
  echo "[cli_socat_init] contents of $TMP_OUT (if any):"
  sed -n '1,200p' "$TMP_OUT" || true
  echo "[cli_socat_init] socat log ($LOG):"; sudo tail -n 200 "$LOG" || true
  echo "[cli_socat_init] socat processes:"; ps aux | grep socat | grep -v grep || true
  rm -f "$TMP_OUT" || true
  exit 2
fi
