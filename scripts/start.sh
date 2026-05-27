#!/bin/sh
set -eu

BIN_PATH="${BIN_PATH:-./target/release/sage-wiki-bridge}"
PID_FILE="${PID_FILE:-./tmp/sage-wiki-bridge.pid}"
LOG_FILE="${LOG_FILE:-./logs/sage-wiki-bridge.log}"

if [ -f "$PID_FILE" ]; then
  old_pid="$(cat "$PID_FILE")"
  if [ -n "$old_pid" ] && kill -0 "$old_pid" 2>/dev/null; then
    echo "sage-wiki-bridge already running: $old_pid"
    exit 0
  fi
fi

mkdir -p "$(dirname "$PID_FILE")" "$(dirname "$LOG_FILE")" data

nohup "$BIN_PATH" >>"$LOG_FILE" 2>&1 &
pid="$!"
printf '%s\n' "$pid" >"$PID_FILE"
echo "sage-wiki-bridge started: $pid"
echo "log: $LOG_FILE"
