#!/bin/sh
set -eu

PID_FILE="${PID_FILE:-./tmp/sage-wiki-bridge.pid}"

if [ ! -f "$PID_FILE" ]; then
  echo "sage-wiki-bridge stopped"
  exit 3
fi

pid="$(cat "$PID_FILE")"
if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
  echo "sage-wiki-bridge running: $pid"
else
  echo "sage-wiki-bridge stopped: stale pid $pid"
  exit 3
fi
