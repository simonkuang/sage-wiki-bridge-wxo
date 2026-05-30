#!/bin/sh
set -eu

exec "$(dirname "$0")/bridgectl.sh" status
