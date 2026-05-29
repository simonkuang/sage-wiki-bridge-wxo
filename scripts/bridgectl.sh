#!/bin/sh
set -eu

DEFAULT_PROJECT_DIR=/data/workspace/sage-wiki-bridge-wxo
PROJECT_DIR="${PROJECT_DIR:-$DEFAULT_PROJECT_DIR}"
ENV_FILE="${ENV_FILE:-${BRIDGE_CONFIG_ENV_FILE:-$PROJECT_DIR/.env}}"

if [ -f "$ENV_FILE" ]; then
  # shellcheck disable=SC1090
  . "$ENV_FILE"
fi

BRIDGE_BIN_PATH="${BRIDGE_BIN_PATH:-${BIN_PATH:-/usr/local/bin/sage-wiki-bridge}}"
BRIDGE_CONFIG_ENV_FILE="${BRIDGE_CONFIG_ENV_FILE:-$ENV_FILE}"

command="${1:-run}"
if [ "$#" -gt 0 ]; then
  shift
fi

case "$command" in
  run)
    set -- "$BRIDGE_BIN_PATH"
    ;;
  -V)
    set -- "$BRIDGE_BIN_PATH" -V
    ;;
  status)
    set -- "$BRIDGE_BIN_PATH" status
    ;;
  version)
    exec "$BRIDGE_BIN_PATH" version
    ;;
  *)
    echo "usage: $0 [run|-V|status|version]" >&2
    exit 2
    ;;
esac

if [ -f "$BRIDGE_CONFIG_ENV_FILE" ]; then
  set -- "$@" --env-file "$BRIDGE_CONFIG_ENV_FILE"
fi

if [ "${BRIDGE_BIND_ADDR+x}" = "x" ]; then set -- "$@" --bind-addr "$BRIDGE_BIND_ADDR"; fi
if [ "${BRIDGE_DATABASE_URL+x}" = "x" ]; then set -- "$@" --database-url "$BRIDGE_DATABASE_URL"; fi
if [ "${BRIDGE_DATABASE_MAX_CONNECTIONS+x}" = "x" ]; then set -- "$@" --database-max-connections "$BRIDGE_DATABASE_MAX_CONNECTIONS"; fi
if [ "${BRIDGE_DATABASE_MIN_CONNECTIONS+x}" = "x" ]; then set -- "$@" --database-min-connections "$BRIDGE_DATABASE_MIN_CONNECTIONS"; fi
if [ "${BRIDGE_RAW_ARCHIVE_DIR+x}" = "x" ]; then set -- "$@" --raw-archive-dir "$BRIDGE_RAW_ARCHIVE_DIR"; fi
if [ "${BRIDGE_RAW_ARCHIVE_FULL+x}" = "x" ]; then set -- "$@" --raw-archive-full "$BRIDGE_RAW_ARCHIVE_FULL"; fi
if [ "${BRIDGE_PROCESSED_ARTIFACT_DIR+x}" = "x" ]; then set -- "$@" --processed-artifact-dir "$BRIDGE_PROCESSED_ARTIFACT_DIR"; fi
if [ "${BRIDGE_SAGE_WIKI_SOURCE_DIR+x}" = "x" ]; then set -- "$@" --sage-wiki-source-dir "$BRIDGE_SAGE_WIKI_SOURCE_DIR"; fi
if [ "${BRIDGE_WECHAT_CALLBACK_PATH+x}" = "x" ]; then set -- "$@" --wechat-callback-path "$BRIDGE_WECHAT_CALLBACK_PATH"; fi
if [ "${BRIDGE_WECHAT_ENCRYPTED_CALLBACK_ENABLED+x}" = "x" ]; then set -- "$@" --wechat-encrypted-callback-enabled "$BRIDGE_WECHAT_ENCRYPTED_CALLBACK_ENABLED"; fi
if [ "${BRIDGE_HONEYPOT_REPLY_ENABLED+x}" = "x" ]; then set -- "$@" --honeypot-reply-enabled "$BRIDGE_HONEYPOT_REPLY_ENABLED"; fi
if [ "${BRIDGE_HONEYPOT_REPLY_TEXT+x}" = "x" ]; then set -- "$@" --honeypot-reply-text "$BRIDGE_HONEYPOT_REPLY_TEXT"; fi
if [ "${BRIDGE_WORKER_ENABLED+x}" = "x" ]; then set -- "$@" --worker-enabled "$BRIDGE_WORKER_ENABLED"; fi
if [ "${BRIDGE_WORKER_ID+x}" = "x" ]; then set -- "$@" --worker-id "$BRIDGE_WORKER_ID"; fi
if [ "${BRIDGE_APP_VERSION+x}" = "x" ]; then set -- "$@" --bridge-version "$BRIDGE_APP_VERSION"; fi
if [ "${BRIDGE_WORKER_INTERVAL_MS+x}" = "x" ]; then set -- "$@" --worker-interval-ms "$BRIDGE_WORKER_INTERVAL_MS"; fi
if [ "${BRIDGE_WORKER_PROCESSING_TIMEOUT_SECONDS+x}" = "x" ]; then set -- "$@" --worker-processing-timeout-seconds "$BRIDGE_WORKER_PROCESSING_TIMEOUT_SECONDS"; fi
if [ "${BRIDGE_WORKER_RETRY_BASE_SECONDS+x}" = "x" ]; then set -- "$@" --worker-retry-base-seconds "$BRIDGE_WORKER_RETRY_BASE_SECONDS"; fi
if [ "${BRIDGE_WORKER_RETRY_MAX_SECONDS+x}" = "x" ]; then set -- "$@" --worker-retry-max-seconds "$BRIDGE_WORKER_RETRY_MAX_SECONDS"; fi
if [ "${BRIDGE_HTTP_TIMEOUT_SECONDS+x}" = "x" ]; then set -- "$@" --http-timeout-seconds "$BRIDGE_HTTP_TIMEOUT_SECONDS"; fi
if [ "${BRIDGE_REQUEST_BODY_LIMIT_BYTES+x}" = "x" ]; then set -- "$@" --request-body-limit-bytes "$BRIDGE_REQUEST_BODY_LIMIT_BYTES"; fi
if [ "${BRIDGE_HEALTHZ_PATH+x}" = "x" ]; then set -- "$@" --healthz-path "$BRIDGE_HEALTHZ_PATH"; fi
if [ "${BRIDGE_READYZ_PATH+x}" = "x" ]; then set -- "$@" --readyz-path "$BRIDGE_READYZ_PATH"; fi
if [ "${BRIDGE_WECHAT_API_BASE+x}" = "x" ]; then set -- "$@" --wechat-api-base "$BRIDGE_WECHAT_API_BASE"; fi
if [ "${BRIDGE_WECHAT_TOKEN_REFRESH_SKEW_SECONDS+x}" = "x" ]; then set -- "$@" --wechat-token-refresh-skew-seconds "$BRIDGE_WECHAT_TOKEN_REFRESH_SKEW_SECONDS"; fi
if [ "${BRIDGE_ADMIN_BASE_PATH+x}" = "x" ]; then set -- "$@" --admin-base-path "$BRIDGE_ADMIN_BASE_PATH"; fi
if [ "${BRIDGE_WHITELIST_JOIN_COMMAND+x}" = "x" ]; then set -- "$@" --whitelist-join-command "$BRIDGE_WHITELIST_JOIN_COMMAND"; fi
if [ "${BRIDGE_ADMIN_DEFAULT_PER_PAGE+x}" = "x" ]; then set -- "$@" --admin-default-per-page "$BRIDGE_ADMIN_DEFAULT_PER_PAGE"; fi
if [ "${BRIDGE_ADMIN_MAX_PER_PAGE+x}" = "x" ]; then set -- "$@" --admin-max-per-page "$BRIDGE_ADMIN_MAX_PER_PAGE"; fi
if [ "${BRIDGE_MAX_MEDIA_BYTES+x}" = "x" ]; then set -- "$@" --max-media-bytes "$BRIDGE_MAX_MEDIA_BYTES"; fi
if [ "${BRIDGE_GEMINI_ENDPOINT_BASE+x}" = "x" ]; then set -- "$@" --gemini-endpoint-base "$BRIDGE_GEMINI_ENDPOINT_BASE"; fi
if [ "${BRIDGE_GEMINI_MODEL+x}" = "x" ]; then set -- "$@" --gemini-model "$BRIDGE_GEMINI_MODEL"; fi
if [ "${BRIDGE_GEMINI_MAX_INLINE_BYTES+x}" = "x" ]; then set -- "$@" --gemini-max-inline-bytes "$BRIDGE_GEMINI_MAX_INLINE_BYTES"; fi
if [ "${BRIDGE_LLM_IMAGE_SYSTEM_PROMPT+x}" = "x" ]; then set -- "$@" --llm-image-system-prompt "$BRIDGE_LLM_IMAGE_SYSTEM_PROMPT"; fi
if [ "${BRIDGE_LLM_VOICE_SYSTEM_PROMPT+x}" = "x" ]; then set -- "$@" --llm-voice-system-prompt "$BRIDGE_LLM_VOICE_SYSTEM_PROMPT"; fi
if [ "${BRIDGE_LLM_VIDEO_SYSTEM_PROMPT+x}" = "x" ]; then set -- "$@" --llm-video-system-prompt "$BRIDGE_LLM_VIDEO_SYSTEM_PROMPT"; fi
if [ "${BRIDGE_TENCENT_LBS_ENDPOINT+x}" = "x" ]; then set -- "$@" --tencent-lbs-endpoint "$BRIDGE_TENCENT_LBS_ENDPOINT"; fi
if [ "${BRIDGE_TENCENT_LBS_GET_POI+x}" = "x" ]; then set -- "$@" --tencent-lbs-get-poi "$BRIDGE_TENCENT_LBS_GET_POI"; fi
if [ "${BRIDGE_TENCENT_LBS_RADIUS_METERS+x}" = "x" ]; then set -- "$@" --tencent-lbs-radius-meters "$BRIDGE_TENCENT_LBS_RADIUS_METERS"; fi
if [ "${BRIDGE_JINA_READER_ENDPOINT+x}" = "x" ]; then set -- "$@" --jina-reader-endpoint "$BRIDGE_JINA_READER_ENDPOINT"; fi
if [ "${BRIDGE_RUST_LOG+x}" = "x" ]; then set -- "$@" --rust-log "$BRIDGE_RUST_LOG"; fi

exec "$@"
