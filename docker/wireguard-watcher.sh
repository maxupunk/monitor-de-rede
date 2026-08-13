#!/bin/bash
# ---------------------------------------------------------------------------
# NetMonitor · watcher de hot-reload do WireGuard
#
# A aplicação escreve ${WG_CONFIG_DIR}/<iface>.conf e este watcher aplica a
# mudança com `wg syncconf`, que altera apenas o delta de peers — os túneis já
# estabelecidos continuam de pé.
#
# Também publica <iface>.status (saída de `wg show <iface> dump`) para que a
# aplicação leia handshake e contadores de tráfego sem privilégio de rede.
#
# Este é o único processo do container que roda como root, e existe justamente
# para que o outro não precise: o canal entre os dois é o arquivo, nunca uma
# chamada de `wg` feita pela API (§7 do AGENTS.md).
#
# Antes rodava dentro do container `linuxserver/wireguard`, disparado pelo init
# customizado daquela imagem. O `#!/usr/bin/with-contenv bash` de lá não existe
# aqui — o ambiente vem do próprio entrypoint.
# ---------------------------------------------------------------------------
set -u

IFACE="${WG_INTERFACE:-wg0}"
DIR="${WG_CONFIG_DIR:-/data/wg}"
CONFIG="${DIR}/${IFACE}.conf"
STATUS="${DIR}/${IFACE}.status"
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
      # em ${WG_CONFIG_DIR}. Com o caminho completo, wg-quick deriva o nome da
      # interface do próprio nome do arquivo (wg0.conf -> wg0).
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
