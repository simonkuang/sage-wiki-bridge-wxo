#!/bin/sh
set -eu

PROJECT_DIR="${PROJECT_DIR:-/data/workspace/sage-wiki-bridge-wxo}"
ENV_FILE="${ENV_FILE:-${BRIDGE_CONFIG_ENV_FILE:-$PROJECT_DIR/.env}}"
BRIDGE_BIN_PATH="${BRIDGE_BIN_PATH:-${BIN_PATH:-/usr/local/bin/sage-wiki-bridge}}"
BRIDGE_SYSTEMD_UNIT="${BRIDGE_SYSTEMD_UNIT:-sage-wiki-bridge.service}"

command="${1:-run}"
if [ "$#" -gt 0 ]; then
  shift
fi

case "$command" in
  run)
    set -- "$BRIDGE_BIN_PATH" "$@"
    ;;
  -V)
    set -- "$BRIDGE_BIN_PATH" -V "$@"
    ;;
  status|version|doctor|health|ready)
    set -- "$BRIDGE_BIN_PATH" "$command" "$@"
    ;;
  logs)
    exec journalctl -u "$BRIDGE_SYSTEMD_UNIT" "$@"
    ;;
  tail)
    exec journalctl -u "$BRIDGE_SYSTEMD_UNIT" -f "$@"
    ;;
  service-status)
    exec systemctl status --no-pager "$BRIDGE_SYSTEMD_UNIT"
    ;;
  argv)
    set -- "$BRIDGE_BIN_PATH" "$@"
    ;;
  *)
    echo "usage: $0 [run|-V|status|version|doctor|health|ready|logs|tail|service-status|argv]" >&2
    exit 2
    ;;
esac

if [ -f "$ENV_FILE" ]; then
  set -- "$@" --env-file "$ENV_FILE"
fi

if [ "$command" = "argv" ]; then
  printf '%s\n' "$@"
  exit 0
fi

exec "$@"
