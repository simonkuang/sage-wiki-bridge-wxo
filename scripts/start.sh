#!/bin/sh
set -eu

BIN_PATH="${BIN_PATH:-./target/release/sage-wiki-bridge}"
PID_FILE="${PID_FILE:-./tmp/sage-wiki-bridge.pid}"
LOG_FILE="${LOG_FILE:-./logs/sage-wiki-bridge.log}"
ENV_FILE="${ENV_FILE:-}"
USE_PROCESS_ENV="${USE_PROCESS_ENV:-false}"

APP_BIND_ADDR="${APP_BIND_ADDR:-127.0.0.1:8080}"
DATABASE_URL="${DATABASE_URL:-sqlite://data/bridge.sqlite3}"
RAW_ARCHIVE_DIR="${RAW_ARCHIVE_DIR:-data/raw}"
RAW_ARCHIVE_FULL="${RAW_ARCHIVE_FULL:-true}"
PROCESSED_ARTIFACT_DIR="${PROCESSED_ARTIFACT_DIR:-data/processed}"
SAGE_WIKI_SOURCE_DIR="${SAGE_WIKI_SOURCE_DIR:-source}"
WECHAT_CALLBACK_PATH="${WECHAT_CALLBACK_PATH:-/wechat/callback}"
WECHAT_ENCRYPTED_CALLBACK_ENABLED="${WECHAT_ENCRYPTED_CALLBACK_ENABLED:-false}"
HONEYPOT_REPLY_ENABLED="${HONEYPOT_REPLY_ENABLED:-false}"
HONEYPOT_REPLY_TEXT="${HONEYPOT_REPLY_TEXT:-Message received.}"
WORKER_ENABLED="${WORKER_ENABLED:-true}"
WORKER_INTERVAL_MS="${WORKER_INTERVAL_MS:-1000}"
WORKER_PROCESSING_TIMEOUT_SECONDS="${WORKER_PROCESSING_TIMEOUT_SECONDS:-900}"
HTTP_TIMEOUT_SECONDS="${HTTP_TIMEOUT_SECONDS:-30}"
RUST_LOG="${RUST_LOG:-info,sage_wiki_bridge=debug}"

if [ -f "$PID_FILE" ]; then
  old_pid="$(cat "$PID_FILE")"
  if [ -n "$old_pid" ] && kill -0 "$old_pid" 2>/dev/null; then
    echo "sage-wiki-bridge already running: $old_pid"
    exit 0
  fi
fi

mkdir -p "$(dirname "$PID_FILE")" "$(dirname "$LOG_FILE")" data

set -- "$BIN_PATH"
if [ -n "$ENV_FILE" ]; then
  set -- "$@" --env-file "$ENV_FILE"
fi
if [ "$USE_PROCESS_ENV" = "true" ]; then
  set -- "$@" --use-process-env
fi
set -- "$@" \
  --bind-addr "$APP_BIND_ADDR" \
  --database-url "$DATABASE_URL" \
  --raw-archive-dir "$RAW_ARCHIVE_DIR" \
  --raw-archive-full "$RAW_ARCHIVE_FULL" \
  --processed-artifact-dir "$PROCESSED_ARTIFACT_DIR" \
  --sage-wiki-source-dir "$SAGE_WIKI_SOURCE_DIR" \
  --wechat-callback-path "$WECHAT_CALLBACK_PATH" \
  --wechat-encrypted-callback-enabled "$WECHAT_ENCRYPTED_CALLBACK_ENABLED" \
  --honeypot-reply-enabled "$HONEYPOT_REPLY_ENABLED" \
  --honeypot-reply-text "$HONEYPOT_REPLY_TEXT" \
  --worker-enabled "$WORKER_ENABLED" \
  --worker-interval-ms "$WORKER_INTERVAL_MS" \
  --worker-processing-timeout-seconds "$WORKER_PROCESSING_TIMEOUT_SECONDS" \
  --http-timeout-seconds "$HTTP_TIMEOUT_SECONDS" \
  --rust-log "$RUST_LOG"

nohup "$@" >>"$LOG_FILE" 2>&1 &
pid="$!"
printf '%s\n' "$pid" >"$PID_FILE"
echo "sage-wiki-bridge started: $pid"
echo "log: $LOG_FILE"
