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
HEARTBEAT="/tmp/wireguard-watcher.heartbeat"
INTERVAL="${WG_WATCH_INTERVAL:-5}"
LAST_CHECKSUM=""
LAST_IFACE_CHECKSUM=""

log() {
  echo "[netmonitor-watcher] $(date -Iseconds) $*"
}

# Escolhe uma vez o backend de iptables compatível com o kernel/host e o coloca
# na frente do PATH apenas para o watcher (e filhos como o wg-quick), sem
# wrapper global em /usr/sbin nem arquivo em /run compartilhado com a API (§7).
select_iptables_backend() {
  local sbin_dir="${IPTABLES_SBIN_DIR:-/usr/sbin}"
  local backend_base="${IPTABLES_BACKEND_BASE:-/usr/lib/netmonitor}"
  local nft_ok=0
  local legacy_ok=0
  local nft_dump=""
  local legacy_dump=""
  local nft_has_rules=0
  local legacy_has_rules=0
  local backend=""

  if "${sbin_dir}/iptables-nft" -L -n >/dev/null 2>&1; then
    nft_ok=1
  fi
  if "${sbin_dir}/iptables-legacy" -L -n >/dev/null 2>&1; then
    legacy_ok=1
  fi

  if [ "${nft_ok}" -eq 1 ] && [ "${legacy_ok}" -eq 0 ]; then
    backend="nft"
  elif [ "${legacy_ok}" -eq 1 ] && [ "${nft_ok}" -eq 0 ]; then
    backend="legacy"
  elif [ "${nft_ok}" -eq 1 ] && [ "${legacy_ok}" -eq 1 ]; then
    nft_dump="$("${sbin_dir}/iptables-nft-save" 2>/dev/null || true)"
    legacy_dump="$("${sbin_dir}/iptables-legacy-save" 2>/dev/null || true)"
    if printf '%s\n' "${nft_dump}" | grep -qE '^(-A |:DOCKER)'; then
      nft_has_rules=1
    fi
    if printf '%s\n' "${legacy_dump}" | grep -qE '^(-A |:DOCKER)'; then
      legacy_has_rules=1
    fi
    if [ "${legacy_has_rules}" -eq 1 ] && [ "${nft_has_rules}" -eq 0 ]; then
      backend="legacy"
    else
      backend="nft"
    fi
  fi

  if [ -n "${backend}" ] && [ -d "${backend_base}/iptables-${backend}" ]; then
    export PATH="${backend_base}/iptables-${backend}:${PATH}"
    log "backend iptables selecionado: ${backend}"
  elif [ -z "${backend}" ]; then
    log "aviso: nem iptables-nft nem iptables-legacy respondem — as regras PostUp da ${IFACE} vão falhar"
  fi
}

ensure_tun_device() {
  if [ ! -c /dev/net/tun ]; then
    mkdir -p /dev/net 2>/dev/null || true
    mknod -m 0600 /dev/net/tun c 10 200 2>/dev/null || true
  fi
}

bring_up_interface() {
  # Quando o kernel do host não traz o módulo `wireguard` (ex.: Synology DSM),
  # o próprio `wg-quick` cai automaticamente para `wireguard-go` em userspace
  # usando `/dev/net/tun`.
  ensure_tun_device
  # Passa o caminho completo do arquivo, não o nome da interface: wg-quick
  # procura por padrão em /etc/wireguard/<iface>.conf, mas nosso config vive
  # em ${WG_CONFIG_DIR}. Com o caminho completo, wg-quick deriva o nome da
  # interface do próprio nome do arquivo (wg0.conf -> wg0).
  if wg-quick up "${CONFIG}"; then
    LAST_CHECKSUM="${CHECKSUM}"
    LAST_IFACE_CHECKSUM="${IFACE_CHECKSUM}"
    return 0
  fi

  rm -f "${STATUS}"
  LAST_CHECKSUM=""
  LAST_IFACE_CHECKSUM=""
  if ! (exec 3<>/dev/net/tun) 2>/dev/null; then
    log "aviso: falha ao subir ${IFACE} — /dev/net/tun indisponível (no Synology DSM agende 'insmod /lib/modules/tun.ko' como root no boot; via docker run sem compose adicione --device-cgroup-rule='c 10:200 rwm')"
  else
    log "falha ao subir ${IFACE} com wg-quick — nova tentativa no próximo ciclo"
  fi
  return 1
}

cleanup() {
  # Ignora sinais repetidos durante o encerramento (ex.: SIGTERM simultâneo do
  # tini -g e do supervisor) para concluir a remoção da interface.
  trap '' TERM INT
  log "encerrando watcher — derrubando interface ${IFACE}"
  wg-quick down "${CONFIG}" >/dev/null 2>&1 || true
  rm -f "${STATUS}" "${STATUS}.tmp" "${HEARTBEAT}"
  exit 0
}
trap cleanup TERM INT

select_iptables_backend
log "monitorando ${CONFIG} a cada ${INTERVAL}s"

while true; do
  if [ -f "${CONFIG}" ]; then
    CHECKSUM="$(md5sum "${CONFIG}" | cut -d' ' -f1)"
    IFACE_CHECKSUM="$(awk '/^\[Interface\]/{f=1; next} /^\[/{f=0} f && !/^[[:space:]]*#/ && NF' "${CONFIG}" | md5sum | cut -d' ' -f1)"

    if ! wg show "${IFACE}" >/dev/null 2>&1; then
      log "interface ${IFACE} fora do ar — subindo com wg-quick"
      bring_up_interface || true
    elif [ "${CHECKSUM}" != "${LAST_CHECKSUM}" ]; then
      if [ "${IFACE_CHECKSUM}" != "${LAST_IFACE_CHECKSUM}" ]; then
        log "seção [Interface] alterada — recriando ${IFACE} com wg-quick"
        wg-quick down "${CONFIG}" >/dev/null 2>&1 || ip link delete dev "${IFACE}" >/dev/null 2>&1 || true
        bring_up_interface || true
      else
        log "configuração alterada — aplicando com syncconf (sem derrubar túneis)"
        if wg syncconf "${IFACE}" <(wg-quick strip "${CONFIG}"); then
          LAST_CHECKSUM="${CHECKSUM}"
        else
          log "falha ao aplicar syncconf — nova tentativa no próximo ciclo"
        fi
      fi
    fi
  fi

  if wg show "${IFACE}" >/dev/null 2>&1; then
    if wg show "${IFACE}" dump >"${STATUS}.tmp" 2>/dev/null; then
      mv "${STATUS}.tmp" "${STATUS}"
    fi
  fi

  touch "${HEARTBEAT}" 2>/dev/null || true
  sleep "${INTERVAL}" &
  wait $! 2>/dev/null || true
done
