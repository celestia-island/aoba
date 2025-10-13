#!/usr/bin/env bash

# Unified socat init script for both tui and cli E2E tests
# Usage: socat_init.sh [--mode cli|tui]

if [ -z "${BASH_VERSION:-}" ]; then
  if command -v bash >/dev/null 2>&1; then
    exec bash "$0" "$@"
  else
    echo "This script requires bash; please install bash or run with bash." >&2
    exit 1
  fi
fi

set -euo pipefail

# defaults
MODE="tui"
while [ "$#" -gt 0 ]; do
  case "$1" in
    --mode)
      shift
      if [ $# -eq 0 ]; then
        echo "--mode requires an argument: cli or tui" >&2
        exit 2
      fi
      MODE="$1"
      ;;
    --mode=*)
      MODE="${1#*=}"
      ;;
    -h|--help)
      echo "Usage: $0 [--mode cli|tui]";
      exit 0
      ;;
    *)
      echo "Unknown arg: $1" >&2; exit 2
      ;;
  esac
  shift
done

V1=/tmp/vcom1
V2=/tmp/vcom2
PIDFILE="/tmp/aoba_socat_${MODE}.pid"
LOG="/tmp/aoba_socat_${MODE}.log"
KILL_PATTERNS=("$V1" "$V2" "$(basename "$V1")")

echo "[socat_init] operating entirely without sudo; using static ports $V1 and $V2"

PORT1="$V1"
PORT2="$V2"

echo "[socat_init] mode=$MODE stopping existing socat (pidfile if present)"
if [ -f "$PIDFILE" ]; then
  if OLD_PID=$(cat "$PIDFILE" 2>/dev/null); then
    if [ -n "$OLD_PID" ] && kill "$OLD_PID" 2>/dev/null; then
      echo "[socat_init] stopped previous socat pid $OLD_PID"
    elif [ -n "$OLD_PID" ] && kill -0 "$OLD_PID" 2>/dev/null; then
      echo "[socat_init] unable to stop socat pid $OLD_PID (insufficient permissions?)" >&2
      echo "[socat_init] please terminate the existing socat process manually" >&2
      exit 1
    fi
  fi
  rm -f "$PIDFILE" || true
fi

echo "[socat_init] killing lingering socat processes that reference vcom links (if any)"
for pattern in "${KILL_PATTERNS[@]}"; do
  pkill -f "$pattern" 2>/dev/null || true
done
if pgrep -f "socat.*${V1}" >/dev/null 2>&1 || pgrep -f "socat.*${V2}" >/dev/null 2>&1; then
  echo "[socat_init] detected existing socat processes that could not be terminated automatically." >&2
  echo "[socat_init] please run 'sudo pkill socat' (or manually stop them) and retry." >&2
  exit 1
fi
sleep 0.5

echo "[socat_init] removing existing links: $V1 $V2"
rm -f "$V1" "$V2" /tmp/aoba_vcom1.* /tmp/aoba_vcom2.* || true
if [ -e "$V1" ] || [ -e "$V2" ]; then
  echo "[socat_init] unable to remove existing port links (permission denied?)." >&2
  echo "[socat_init] please remove $V1 and $V2 manually, then rerun." >&2
  exit 1
fi

echo "[socat_init] starting socat with link and mode=0666"
rm -f "$LOG" || true
setsid socat -d -d PTY,link="$V1",raw,echo=0,mode=0666 PTY,link="$V2",raw,echo=0,mode=0666 2>"$LOG" >/dev/null &
SOCAT_PID=$!
echo "$SOCAT_PID" >"$PIDFILE"
disown "$SOCAT_PID" 2>/dev/null || true
if ! kill -0 "$SOCAT_PID" 2>/dev/null; then
  echo "[socat_init] failed to start socat"
  echo "[socat_init] socat log ($LOG):"; tail -n 200 "$LOG" || true
  exit 1
fi

SOCAT_PID=$(cat "$PIDFILE" 2>/dev/null || echo "")
echo "[socat_init] socat pid: ${SOCAT_PID:-unknown}, waiting for $V1 and $V2"

timeout=15
count=0
while [ $count -lt $timeout ] && ( [ ! -e "$V1" ] || [ ! -e "$V2" ] ); do
  sleep 1
  count=$((count + 1))
done

if [ -e "$V1" ] && [ -e "$V2" ]; then
  echo "[socat_init] created links:"
  ls -la "$V1" "$V2" || true
  P1=$(readlink -f "$V1" || true)
  P2=$(readlink -f "$V2" || true)
  if [ -n "$P1" ]; then chmod 666 "$P1" || true; fi
  if [ -n "$P2" ]; then chmod 666 "$P2" || true; fi
  echo "[socat_init] underlying pts:"; ls -la "$P1" "$P2" || true
else
  echo "[socat_init] Failed to create $V1 and $V2 within ${timeout}s"
  echo "[socat_init] socat log ($LOG):"; tail -n 200 "$LOG" || true
  echo "[socat_init] socat processes:"; ps aux | grep socat | grep -v grep || true
  exit 1
fi

echo "[socat_init] performing connectivity test: write to $V2, read from $V1"
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
  echo "[socat_init] connectivity test passed: data written to $V2 was received on $V1"
  rm -f "$TMP_OUT" || true
  echo "PORT1=$PORT1"
  echo "PORT2=$PORT2"
  echo "[socat_init] finished successfully"
  exit 0
else
  echo "[socat_init] connectivity test FAILED"
  echo "[socat_init] contents of $TMP_OUT (if any):"
  sed -n '1,200p' "$TMP_OUT" || true
  echo "[socat_init] socat log ($LOG):"; tail -n 200 "$LOG" || true
  echo "[socat_init] socat processes:"; ps aux | grep socat | grep -v grep || true
  rm -f "$TMP_OUT" || true
  exit 2
fi
