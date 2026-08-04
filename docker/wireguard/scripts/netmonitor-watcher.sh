#!/usr/bin/with-contenv bash
# ---------------------------------------------------------------------------
# NetMonitor · watcher de hot-reload do WireGuard
#
# O container da API escreve /config/<iface>.conf no volume compartilhado e este
# watcher aplica a mudança com `wg syncconf`, que altera apenas o delta de peers
# — os túneis já estabelecidos continuam de pé.
#
# Nenhum acesso ao socket do Docker é necessário: a comunicação é o arquivo.
#
# Também publica /config/<iface>.status (saída de `wg show <iface> dump`) para
# que a API leia handshake e contadores de tráfego sem privilégio de rede.
# ---------------------------------------------------------------------------
set -u

IFACE="${WG_INTERFACE:-wg0}"
CONFIG="/config/${IFACE}.conf"
STATUS="/config/${IFACE}.status"
INTERVAL="${WG_WATCH_INTERVAL:-5}"
LAST_CHECKSUM=""

log() {
  echo "[netmonitor-watcher] $(date -Iseconds) $*"
}

log "monitorando ${CONFIG} a cada ${INTERVAL}s"

while true; do
  if [ -f "${CONFIG}" ]; then
    CHECKSUM="$(md5sum "${CONFIG}" | cut -d' ' -f1)"

    if ! wg show "${IFACE}" >/dev/null 2>&1; then
      log "interface ${IFACE} fora do ar — subindo com wg-quick"
      # Passa o caminho completo do arquivo, não o nome da interface: wg-quick
      # procura por padrão em /etc/wireguard/<iface>.conf, mas nosso config vive
      # no volume compartilhado /config. Com o caminho completo, wg-quick deriva
      # o nome da interface do próprio nome do arquivo (wg0.conf -> wg0).
      if wg-quick up "${CONFIG}"; then
        LAST_CHECKSUM="${CHECKSUM}"
      fi
    elif [ "${CHECKSUM}" != "${LAST_CHECKSUM}" ]; then
      log "configuração alterada — aplicando com syncconf (sem derrubar túneis)"
      if wg syncconf "${IFACE}" <(wg-quick strip "${CONFIG}"); then
        LAST_CHECKSUM="${CHECKSUM}"
      else
        log "falha ao aplicar syncconf — nova tentativa no próximo ciclo"
      fi
    fi
  fi

  if wg show "${IFACE}" >/dev/null 2>&1; then
    if wg show "${IFACE}" dump >"${STATUS}.tmp" 2>/dev/null; then
      mv "${STATUS}.tmp" "${STATUS}"
    fi
  fi

  sleep "${INTERVAL}"
done
