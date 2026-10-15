#!/bin/sh
# ---------------------------------------------------------------------------
# NetMonitor · libera o socket ICMP sem privilégio (ADR 003)
#
# O ping da API usa `SOCK_DGRAM`, que o kernel só abre para os grupos dentro de
# `net.ipv4.ping_group_range`. O padrão do kernel é `1 0` — intervalo vazio,
# ping fechado para todos — e a API roda sem `CAP_NET_RAW`, então sem este
# sysctl o monitor de ping e a varredura ICMP da descoberta não funcionam.
#
# Dois chamadores, a mesma lógica:
#   * o `entrypoint.sh`, como root, no namespace de rede do container, uma vez
#     (vale quando `/proc/sys` é gravável; o compose em bridge já declara o
#     sysctl);
#   * o guardião `icmp-sysctl` do `docker-compose.host.yml`, com `--watch`: com
#     a rede do host o sysctl do compose não se aplica e o valor que manda é o
#     **do host**, que volta a `1 0` a cada reinício do servidor. Por isso ele
#     não grava uma vez e sai — um serviço que sai não é reiniciado pelo Docker
#     no boot do servidor, e foi exatamente um reboot que fechou o ping em
#     produção (2026-10-14). Ele sobe com o Docker e confere a cada intervalo.
#
# Só amplia, nunca restringe: se o GID da aplicação já está no intervalo, não
# toca em nada. Nunca falha: sem permissão, avisa como corrigir e segue — a
# API sobe mesmo assim, e o ping volta sozinho quando o sysctl for liberado
# (`PingClientCell` tenta de novo a cada uso).
#
# Uso: ping-group-range.sh [--watch] [GID]
#   ICMP_SYSCTL_INTERVAL — segundos entre conferências no `--watch` (padrão 300)
# ---------------------------------------------------------------------------
set -u

RANGE_FILE=/proc/sys/net/ipv4/ping_group_range
APP_USER="${APP_USER:-app}"
INTERVAL="${ICMP_SYSCTL_INTERVAL:-300}"

log() {
  echo "[icmp-sysctl] $*"
}

watch=false
if [ "${1:-}" = "--watch" ]; then
  watch=true
  shift
fi
gid="${1:-$(id -g "${APP_USER}" 2>/dev/null || echo 1000)}"

# Garante o intervalo. `$1` = "quiet" cala o "já libera" — no `--watch` ele
# sairia a cada conferência; o log só registra o que mudou ou falhou.
ensure_range() {
  # `cat` e não `read`: o `read` do busybox lê o procfs byte a byte e perde o
  # segundo campo.
  current=$(cat "${RANGE_FILE}" 2>/dev/null || true)
  # shellcheck disable=SC2086 # a separação em campos é o objetivo
  set -- ${current} "${1:-}"
  if [ $# -ne 3 ]; then
    log "aviso: ${RANGE_FILE} ilegível — kernel sem ICMP DGRAM?"
    return 1
  fi
  low=$1
  high=$2
  quiet=$3

  if [ "${gid}" -ge "${low}" ] && [ "${gid}" -le "${high}" ]; then
    [ "${quiet}" = quiet ] \
      || log "net.ipv4.ping_group_range já libera o GID ${gid} (${low} ${high})"
    return 0
  fi

  # Synology DSM e kernels com GID de 16 bits recusam 2147483647 com
  # "invalid argument"; por isso o segundo valor.
  for range in "0 2147483647" "0 65535"; do
    # As chaves levam o erro do próprio redirecionamento ("Read-only file
    # system") junto para o /dev/null; o aviso abaixo já diz o que fazer.
    if { echo "${range}" > "${RANGE_FILE}"; } 2>/dev/null; then
      log "net.ipv4.ping_group_range ajustado de '${low} ${high}' para '${range}'"
      return 0
    fi
  done

  log "aviso: net.ipv4.ping_group_range está '${low} ${high}' e não pôde ser" \
      "ajustado daqui. O ping fica indisponível até liberar no host:" \
      "sysctl -w net.ipv4.ping_group_range=\"0 2147483647\" (e persistir em" \
      "/etc/sysctl.d/). Não é preciso reiniciar o NetMonitor."
  return 1
}

if [ "${watch}" = false ]; then
  ensure_range || true
  exit 0
fi

# PID 1 do container: sem o trap, o `docker stop` esperaria os 10 s do SIGKILL.
# `sleep &` + `wait` deixa o sinal interromper a espera na hora.
trap 'exit 0' TERM INT
log "guardião ativo: confere net.ipv4.ping_group_range a cada ${INTERVAL}s"
ensure_range || true
while true; do
  sleep "${INTERVAL}" &
  wait $!
  ensure_range quiet || true
done
