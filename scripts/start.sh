#!/bin/sh
set -eu

PID_FILE="${PID_FILE:-./tmp/sage-wiki-bridge.pid}"
LOG_FILE="${LOG_FILE:-./logs/sage-wiki-bridge.log}"
ENV_FILE="${ENV_FILE:-${BRIDGE_CONFIG_ENV_FILE:-/data/workspace/sage-wiki-bridge-wxo/.env}}"

if [ -f "$PID_FILE" ]; then
  old_pid="$(cat "$PID_FILE")"
  if [ -n "$old_pid" ] && kill -0 "$old_pid" 2>/dev/null; then
    echo "sage-wiki-bridge already running: $old_pid"
    exit 0
  fi
fi

mkdir -p "$(dirname "$PID_FILE")" "$(dirname "$LOG_FILE")" data

if [ -n "$ENV_FILE" ]; then
  ENV_FILE="$ENV_FILE" nohup "$(dirname "$0")/bridgectl.sh" run >>"$LOG_FILE" 2>&1 &
else
  nohup "$(dirname "$0")/bridgectl.sh" run >>"$LOG_FILE" 2>&1 &
fi
pid="$!"
printf '%s\n' "$pid" >"$PID_FILE"
echo "sage-wiki-bridge started: $pid"
echo "log: $LOG_FILE"
