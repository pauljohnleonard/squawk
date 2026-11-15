#!/usr/bin/env bash
set -euo pipefail

# Usage:
#   PI_HOST=pi.local PI_USER=pi ./scripts/pi_deploy.sh
# Optional:
#   PI_PATH=/home/pi/squawk

: "${PI_HOST:?Set PI_HOST (e.g. pi.local)}"
: "${PI_USER:=pi}"
: "${PI_PATH:=/home/${PI_USER}/squawk}"

BIN="target/aarch64-unknown-linux-gnu/release/squawk-app"

if [[ ! -f "$BIN" ]]; then
  echo "Binary not found: $BIN" >&2
  echo "Build it first (see README.md)." >&2
  exit 1
fi

ssh "${PI_USER}@${PI_HOST}" "mkdir -p '${PI_PATH}'"
scp "$BIN" "${PI_USER}@${PI_HOST}:${PI_PATH}/squawk-app"
ssh "${PI_USER}@${PI_HOST}" "chmod +x '${PI_PATH}/squawk-app'"

echo "Deployed to ${PI_USER}@${PI_HOST}:${PI_PATH}/squawk-app"
