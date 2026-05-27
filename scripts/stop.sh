#!/bin/sh
set -eu

PID_FILE="${PID_FILE:-./tmp/sage-wiki-bridge.pid}"

if [ ! -f "$PID_FILE" ]; then
  echo "sage-wiki-bridge is not running: missing $PID_FILE"
  exit 0
fi

pid="$(cat "$PID_FILE")"
if [ -z "$pid" ] || ! kill -0 "$pid" 2>/dev/null; then
  echo "sage-wiki-bridge is not running: stale pid $pid"
  rm -f "$PID_FILE"
  exit 0
fi

kill "$pid"
for _ in 1 2 3 4 5 6 7 8 9 10; do
  if ! kill -0 "$pid" 2>/dev/null; then
    rm -f "$PID_FILE"
    echo "sage-wiki-bridge stopped: $pid"
    exit 0
  fi
  sleep 1
done

echo "sage-wiki-bridge did not stop within 10 seconds: $pid"
exit 1
