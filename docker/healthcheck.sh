#!/bin/sh
# Healthcheck do container: API saudável + watcher vivo (quando presente).
set -eu

# A API precisa responder independentemente do watcher.
if ! curl -fsS "http://localhost:${APP_PORT:-3333}/_health" >/dev/null; then
  exit 1
fi

# Se o watcher publicou heartbeat, ele não pode estar parado há mais de 30s.
HEARTBEAT="/tmp/wireguard-watcher.heartbeat"
if [ -f "${HEARTBEAT}" ]; then
  now=$(date +%s)
  last=$(stat -c %Y "${HEARTBEAT}" 2>/dev/null || echo 0)
  if [ "$(( now - last ))" -gt 30 ]; then
    exit 1
  fi
fi

exit 0
