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
        echo "--mode requires an argument: cli, tui, or tui_multiple" >&2
        exit 2
      fi
      MODE="$1"
      ;;
    --mode=*)
      MODE="${1#*=}"
      ;;
    -h|--help)
      echo "Usage: $0 [--mode cli|tui|tui_multiple]";
      exit 0
      ;;
    *)
      echo "Unknown arg: $1" >&2; exit 2
      ;;
  esac
  shift
done

# Port configuration based on mode
if [ "$MODE" = "tui_multiple" ]; then
  # For tui_multiple mode: create 6 virtual ports with full mesh connectivity
  V1=/tmp/vcom1
  V2=/tmp/vcom2
  V3=/tmp/vcom3
  V4=/tmp/vcom4
  V5=/tmp/vcom5
  V6=/tmp/vcom6
  PIDFILE="/tmp/aoba_socat_${MODE}.pid"
  LOG="/tmp/aoba_socat_${MODE}.log"
  KILL_PATTERNS=("$V1" "$V2" "$V3" "$V4" "$V5" "$V6" "$(basename "$V1")")
else
  # For cli and tui modes: create 2 virtual ports (backward compatible)
  V1=/tmp/vcom1
  V2=/tmp/vcom2
  PIDFILE="/tmp/aoba_socat_${MODE}.pid"
  LOG="/tmp/aoba_socat_${MODE}.log"
  KILL_PATTERNS=("$V1" "$V2" "$(basename "$V1")")
fi

if [ "$MODE" = "tui_multiple" ]; then
  echo "[socat_init] operating entirely without sudo; using 6 ports $V1, $V2, $V3, $V4, $V5, $V6 with full mesh connectivity"
else
  echo "[socat_init] operating entirely without sudo; using static ports $V1 and $V2"
fi

PORT1="$V1"
PORT2="$V2"
if [ "$MODE" = "tui_multiple" ]; then
  PORT3="$V3"
  PORT4="$V4"
  PORT5="$V5"
  PORT6="$V6"
fi

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
if [ "$MODE" = "tui_multiple" ]; then
  rm -f "$V1" "$V2" "$V3" "$V4" "$V5" "$V6" /tmp/aoba_vcom*.* || true
  if [ -e "$V1" ] || [ -e "$V2" ] || [ -e "$V3" ] || [ -e "$V4" ] || [ -e "$V5" ] || [ -e "$V6" ]; then
    echo "[socat_init] unable to remove existing port links (permission denied?)." >&2
    echo "[socat_init] please remove $V1, $V2, $V3, $V4, $V5, $V6 manually, then rerun." >&2
    exit 1
  fi
else
  rm -f "$V1" "$V2" /tmp/aoba_vcom1.* /tmp/aoba_vcom2.* || true
  if [ -e "$V1" ] || [ -e "$V2" ]; then
    echo "[socat_init] unable to remove existing port links (permission denied?)." >&2
    echo "[socat_init] please remove $V1 and $V2 manually, then rerun." >&2
    exit 1
  fi
fi

echo "[socat_init] starting socat with link and mode=0666"
rm -f "$LOG" || true

if [ "$MODE" = "tui_multiple" ]; then
  # For tui_multiple: create 3 pairs of virtual serial ports (6 total)
  # Pair 1: vcom1-vcom2 (Master 1 with Slave 1)
  # Pair 2: vcom3-vcom4 (Master 2 with Slave 2)
  # Pair 3: vcom5-vcom6 (Slave 3 or for interference testing)
  
  setsid socat -d -d PTY,link="$V1",raw,echo=0,mode=0666 PTY,link="$V2",raw,echo=0,mode=0666 2>"${LOG}.12" >/dev/null &
  PID1=$!
  setsid socat -d -d PTY,link="$V3",raw,echo=0,mode=0666 PTY,link="$V4",raw,echo=0,mode=0666 2>"${LOG}.34" >/dev/null &
  PID2=$!
  setsid socat -d -d PTY,link="$V5",raw,echo=0,mode=0666 PTY,link="$V6",raw,echo=0,mode=0666 2>"${LOG}.56" >/dev/null &
  PID3=$!
  
  # Store all PIDs for cleanup
  echo "${PID1} ${PID2} ${PID3}" >"$PIDFILE"
  disown "$PID1" "$PID2" "$PID3" 2>/dev/null || true
  
  # Check if all processes started successfully
  if ! kill -0 "$PID1" 2>/dev/null || ! kill -0 "$PID2" 2>/dev/null || ! kill -0 "$PID3" 2>/dev/null; then
    echo "[socat_init] failed to start one or more socat processes"
    echo "[socat_init] socat logs:"
    tail -n 200 "${LOG}".* 2>&1 || true
    exit 1
  fi
  
  echo "[socat_init] socat pids: ${PID1} ${PID2} ${PID3}, waiting for all 6 ports"
  
  # Wait for all PTYs to be created
  timeout=15
  count=0
  while [ $count -lt $timeout ] && ( [ ! -e "$V1" ] || [ ! -e "$V2" ] || [ ! -e "$V3" ] || [ ! -e "$V4" ] || [ ! -e "$V5" ] || [ ! -e "$V6" ] ); do
    sleep 1
    count=$((count + 1))
  done
else
  # Original 2-port mode
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
fi

if [ "$MODE" = "tui_multiple" ]; then
  # Verify all 6 ports exist
  if [ -e "$V1" ] && [ -e "$V2" ] && [ -e "$V3" ] && [ -e "$V4" ] && [ -e "$V5" ] && [ -e "$V6" ]; then
    echo "[socat_init] created links:"
    ls -la "$V1" "$V2" "$V3" "$V4" "$V5" "$V6" || true
    for port in "$V1" "$V2" "$V3" "$V4" "$V5" "$V6"; do
      P=$(readlink -f "$port" || true)
      if [ -n "$P" ]; then chmod 666 "$P" || true; fi
    done
    echo "[socat_init] underlying pts:"
    for port in "$V1" "$V2" "$V3" "$V4" "$V5" "$V6"; do
      P=$(readlink -f "$port" || true)
      ls -la "$P" || true
    done
  else
    echo "[socat_init] Failed to create all 6 ports within ${timeout}s"
    echo "[socat_init] socat logs:"
    tail -n 200 "${LOG}".* 2>&1 || true
    echo "[socat_init] socat processes:"; ps aux | grep socat | grep -v grep || true
    exit 1
  fi
else
  # Original 2-port verification
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
  if [ "$MODE" = "tui_multiple" ]; then
    echo "PORT3=$PORT3"
    echo "PORT4=$PORT4"
    echo "PORT5=$PORT5"
    echo "PORT6=$PORT6"
  fi
  echo "[socat_init] finished successfully"
  exit 0
else
  echo "[socat_init] connectivity test FAILED"
  echo "[socat_init] contents of $TMP_OUT (if any):"
  sed -n '1,200p' "$TMP_OUT" || true
  if [ "$MODE" = "tui_multiple" ]; then
    echo "[socat_init] socat logs:"
    tail -n 200 "${LOG}".* 2>&1 || true
  else
    echo "[socat_init] socat log ($LOG):"; tail -n 200 "$LOG" || true
  fi
  echo "[socat_init] socat processes:"; ps aux | grep socat | grep -v grep || true
  rm -f "$TMP_OUT" || true
  exit 2
fi
