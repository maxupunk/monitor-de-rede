#!/bin/sh
# ---------------------------------------------------------------------------
# NetMonitor · entrypoint do container único
#
# Roda como root só o que precisa de root, e larga o privilégio antes de chamar
# a aplicação:
#
#   1. acerta o dono de /data e /config — volume nomeado nasce do root;
#   2. sobe o watcher do WireGuard, se o container tiver NET_ADMIN;
#   3. `exec` na aplicação como `app`, com todas as capabilities removidas.
#
# O passo 3 é o que mantém a fronteira do §7 do AGENTS.md: o processo da API
# continua sem NET_ADMIN e sem executar `wg`. Quem aplica a configuração é o
# watcher, e o canal entre os dois é arquivo — `wg0.conf` de ida, `wg0.status`
# de volta —, exatamente como era quando o WireGuard morava em outro container.
#
# `exec` também é o que faz a aplicação herdar o PID 1 e receber o SIGTERM do
# `docker stop` diretamente, sem intermediário para repassá-lo.
# ---------------------------------------------------------------------------
set -eu

DATA_DIR="${DATA_DIR:-/data}"
WG_CONFIG_DIR="${WG_CONFIG_DIR:-/data/wg}"
APP_USER="${APP_USER:-app}"

log() {
  echo "[netmonitor] $*"
}

# --- 1. diretórios de escrita -----------------------------------------------
mkdir -p "${DATA_DIR}" "${WG_CONFIG_DIR}"
# Bind mount de host pode recusar o chown (Windows/WSL, NFS). Não é fatal: se o
# dono já estiver certo, a aplicação escreve do mesmo jeito — e se não estiver,
# o erro aparece na primeira escrita, com contexto melhor do que aqui.
chown -R "${APP_USER}:${APP_USER}" "${DATA_DIR}" "${WG_CONFIG_DIR}" 2>/dev/null \
  || log "aviso: não foi possível ajustar o dono de ${DATA_DIR}/${WG_CONFIG_DIR}"

# --- 2. watcher do WireGuard ------------------------------------------------
# CAP_NET_ADMIN é o bit 12 do CapEff. Sem ele o `wg-quick` não cria interface
# nenhuma, e subir o watcher só encheria o log de falha a cada 5 s — é o caso
# de quem sobe este mesmo container como probe remoto, sem VPN.
has_net_admin() {
  caps=$(awk '/^CapEff:/ { print $2 }' /proc/self/status 2>/dev/null || echo 0)
  [ $(( 0x${caps} & 0x1000 )) -ne 0 ]
}

case "${WG_ENABLED:-auto}" in
  false | 0 | no | off)
    log "WireGuard desligado por WG_ENABLED"
    ;;
  *)
    if has_net_admin; then
      log "iniciando watcher do WireGuard (${WG_CONFIG_DIR})"
      WG_CONFIG_DIR="${WG_CONFIG_DIR}" /usr/local/bin/wireguard-watcher.sh &
    else
      log "sem CAP_NET_ADMIN — watcher do WireGuard não iniciado"
    fi
    ;;
esac

# --- 3. aplicação, sem privilégio -------------------------------------------
# `--inh-caps=-all` esvazia o conjunto herdável: nem por engano a aplicação
# recebe o NET_ADMIN que o watcher usa.
log "iniciando ${1:-backend_rust-cli} como ${APP_USER}"
exec setpriv --reuid="${APP_USER}" --regid="${APP_USER}" --init-groups \
             --inh-caps=-all -- "$@"
