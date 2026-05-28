#!/bin/sh
set -eu

BIN_PATH="${BIN_PATH:-./target/release/sage-wiki-bridge}"
PID_FILE="${PID_FILE:-./tmp/sage-wiki-bridge.pid}"
LOG_FILE="${LOG_FILE:-./logs/sage-wiki-bridge.log}"
ENV_FILE="${ENV_FILE:-}"
USE_PROCESS_ENV="${USE_PROCESS_ENV:-false}"

BRIDGE_BIND_ADDR="${BRIDGE_BIND_ADDR:-127.0.0.1:8080}"
BRIDGE_DATABASE_URL="${BRIDGE_DATABASE_URL:-sqlite://data/bridge.sqlite3}"
BRIDGE_DATABASE_MAX_CONNECTIONS="${BRIDGE_DATABASE_MAX_CONNECTIONS:-4}"
BRIDGE_DATABASE_MIN_CONNECTIONS="${BRIDGE_DATABASE_MIN_CONNECTIONS:-1}"
BRIDGE_RAW_ARCHIVE_DIR="${BRIDGE_RAW_ARCHIVE_DIR:-data/raw}"
BRIDGE_RAW_ARCHIVE_FULL="${BRIDGE_RAW_ARCHIVE_FULL:-true}"
BRIDGE_PROCESSED_ARTIFACT_DIR="${BRIDGE_PROCESSED_ARTIFACT_DIR:-data/processed}"
BRIDGE_SAGE_WIKI_SOURCE_DIR="${BRIDGE_SAGE_WIKI_SOURCE_DIR:-source}"
BRIDGE_WECHAT_CALLBACK_PATH="${BRIDGE_WECHAT_CALLBACK_PATH:-/wechat/callback}"
BRIDGE_WECHAT_ENCRYPTED_CALLBACK_ENABLED="${BRIDGE_WECHAT_ENCRYPTED_CALLBACK_ENABLED:-false}"
BRIDGE_HONEYPOT_REPLY_ENABLED="${BRIDGE_HONEYPOT_REPLY_ENABLED:-false}"
BRIDGE_HONEYPOT_REPLY_TEXT="${BRIDGE_HONEYPOT_REPLY_TEXT:-Message received.}"
BRIDGE_WORKER_ENABLED="${BRIDGE_WORKER_ENABLED:-true}"
BRIDGE_WORKER_ID="${BRIDGE_WORKER_ID:-worker-main}"
BRIDGE_APP_VERSION="${BRIDGE_APP_VERSION:-0.1.0}"
BRIDGE_WORKER_INTERVAL_MS="${BRIDGE_WORKER_INTERVAL_MS:-1000}"
BRIDGE_WORKER_PROCESSING_TIMEOUT_SECONDS="${BRIDGE_WORKER_PROCESSING_TIMEOUT_SECONDS:-900}"
BRIDGE_WORKER_RETRY_BASE_SECONDS="${BRIDGE_WORKER_RETRY_BASE_SECONDS:-10}"
BRIDGE_WORKER_RETRY_MAX_SECONDS="${BRIDGE_WORKER_RETRY_MAX_SECONDS:-300}"
BRIDGE_HTTP_TIMEOUT_SECONDS="${BRIDGE_HTTP_TIMEOUT_SECONDS:-30}"
BRIDGE_REQUEST_BODY_LIMIT_BYTES="${BRIDGE_REQUEST_BODY_LIMIT_BYTES:-2097152}"
BRIDGE_HEALTHZ_PATH="${BRIDGE_HEALTHZ_PATH:-/healthz}"
BRIDGE_READYZ_PATH="${BRIDGE_READYZ_PATH:-/readyz}"
BRIDGE_RUST_LOG="${BRIDGE_RUST_LOG:-info,sage_wiki_bridge=debug}"
BRIDGE_ADMIN_BASE_PATH="${BRIDGE_ADMIN_BASE_PATH:-/admin}"
BRIDGE_WECHAT_API_BASE="${BRIDGE_WECHAT_API_BASE:-https://api.weixin.qq.com}"
BRIDGE_WECHAT_TOKEN_REFRESH_SKEW_SECONDS="${BRIDGE_WECHAT_TOKEN_REFRESH_SKEW_SECONDS:-300}"
BRIDGE_WHITELIST_JOIN_COMMAND="${BRIDGE_WHITELIST_JOIN_COMMAND:-}"
BRIDGE_ADMIN_DEFAULT_PER_PAGE="${BRIDGE_ADMIN_DEFAULT_PER_PAGE:-20}"
BRIDGE_ADMIN_MAX_PER_PAGE="${BRIDGE_ADMIN_MAX_PER_PAGE:-100}"
BRIDGE_MAX_MEDIA_BYTES="${BRIDGE_MAX_MEDIA_BYTES:-20971520}"
BRIDGE_GEMINI_ENDPOINT_BASE="${BRIDGE_GEMINI_ENDPOINT_BASE:-https://generativelanguage.googleapis.com}"
BRIDGE_GEMINI_MODEL="${BRIDGE_GEMINI_MODEL:-gemini-2.5-flash}"
BRIDGE_GEMINI_MAX_INLINE_BYTES="${BRIDGE_GEMINI_MAX_INLINE_BYTES:-18874368}"
BRIDGE_LLM_IMAGE_SYSTEM_PROMPT="${BRIDGE_LLM_IMAGE_SYSTEM_PROMPT:-Describe this image for a personal knowledge base.}"
BRIDGE_LLM_VOICE_SYSTEM_PROMPT="${BRIDGE_LLM_VOICE_SYSTEM_PROMPT:-Transcribe and summarize this voice message.}"
BRIDGE_LLM_VIDEO_SYSTEM_PROMPT="${BRIDGE_LLM_VIDEO_SYSTEM_PROMPT:-Summarize this video for a personal knowledge base.}"
BRIDGE_TENCENT_LBS_ENDPOINT="${BRIDGE_TENCENT_LBS_ENDPOINT:-https://apis.map.qq.com/ws/geocoder/v1/}"
BRIDGE_TENCENT_LBS_GET_POI="${BRIDGE_TENCENT_LBS_GET_POI:-true}"
BRIDGE_TENCENT_LBS_RADIUS_METERS="${BRIDGE_TENCENT_LBS_RADIUS_METERS:-500}"
BRIDGE_JINA_READER_ENDPOINT="${BRIDGE_JINA_READER_ENDPOINT:-https://r.jina.ai}"

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
  --bind-addr "$BRIDGE_BIND_ADDR" \
  --database-url "$BRIDGE_DATABASE_URL" \
  --database-max-connections "$BRIDGE_DATABASE_MAX_CONNECTIONS" \
  --database-min-connections "$BRIDGE_DATABASE_MIN_CONNECTIONS" \
  --raw-archive-dir "$BRIDGE_RAW_ARCHIVE_DIR" \
  --raw-archive-full "$BRIDGE_RAW_ARCHIVE_FULL" \
  --processed-artifact-dir "$BRIDGE_PROCESSED_ARTIFACT_DIR" \
  --sage-wiki-source-dir "$BRIDGE_SAGE_WIKI_SOURCE_DIR" \
  --wechat-callback-path "$BRIDGE_WECHAT_CALLBACK_PATH" \
  --wechat-encrypted-callback-enabled "$BRIDGE_WECHAT_ENCRYPTED_CALLBACK_ENABLED" \
  --honeypot-reply-enabled "$BRIDGE_HONEYPOT_REPLY_ENABLED" \
  --honeypot-reply-text "$BRIDGE_HONEYPOT_REPLY_TEXT" \
  --worker-enabled "$BRIDGE_WORKER_ENABLED" \
  --worker-id "$BRIDGE_WORKER_ID" \
  --bridge-version "$BRIDGE_APP_VERSION" \
  --worker-interval-ms "$BRIDGE_WORKER_INTERVAL_MS" \
  --worker-processing-timeout-seconds "$BRIDGE_WORKER_PROCESSING_TIMEOUT_SECONDS" \
  --worker-retry-base-seconds "$BRIDGE_WORKER_RETRY_BASE_SECONDS" \
  --worker-retry-max-seconds "$BRIDGE_WORKER_RETRY_MAX_SECONDS" \
  --http-timeout-seconds "$BRIDGE_HTTP_TIMEOUT_SECONDS" \
  --request-body-limit-bytes "$BRIDGE_REQUEST_BODY_LIMIT_BYTES" \
  --healthz-path "$BRIDGE_HEALTHZ_PATH" \
  --readyz-path "$BRIDGE_READYZ_PATH" \
  --wechat-api-base "$BRIDGE_WECHAT_API_BASE" \
  --wechat-token-refresh-skew-seconds "$BRIDGE_WECHAT_TOKEN_REFRESH_SKEW_SECONDS" \
  --admin-base-path "$BRIDGE_ADMIN_BASE_PATH" \
  --whitelist-join-command "$BRIDGE_WHITELIST_JOIN_COMMAND" \
  --admin-default-per-page "$BRIDGE_ADMIN_DEFAULT_PER_PAGE" \
  --admin-max-per-page "$BRIDGE_ADMIN_MAX_PER_PAGE" \
  --max-media-bytes "$BRIDGE_MAX_MEDIA_BYTES" \
  --gemini-endpoint-base "$BRIDGE_GEMINI_ENDPOINT_BASE" \
  --gemini-model "$BRIDGE_GEMINI_MODEL" \
  --gemini-max-inline-bytes "$BRIDGE_GEMINI_MAX_INLINE_BYTES" \
  --llm-image-system-prompt "$BRIDGE_LLM_IMAGE_SYSTEM_PROMPT" \
  --llm-voice-system-prompt "$BRIDGE_LLM_VOICE_SYSTEM_PROMPT" \
  --llm-video-system-prompt "$BRIDGE_LLM_VIDEO_SYSTEM_PROMPT" \
  --tencent-lbs-endpoint "$BRIDGE_TENCENT_LBS_ENDPOINT" \
  --tencent-lbs-get-poi "$BRIDGE_TENCENT_LBS_GET_POI" \
  --tencent-lbs-radius-meters "$BRIDGE_TENCENT_LBS_RADIUS_METERS" \
  --jina-reader-endpoint "$BRIDGE_JINA_READER_ENDPOINT" \
  --rust-log "$BRIDGE_RUST_LOG"

nohup "$@" >>"$LOG_FILE" 2>&1 &
pid="$!"
printf '%s\n' "$pid" >"$PID_FILE"
echo "sage-wiki-bridge started: $pid"
echo "log: $LOG_FILE"
