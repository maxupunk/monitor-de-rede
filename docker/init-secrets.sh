#!/bin/sh
# ---------------------------------------------------------------------------
# NetMonitor · segredos da instalação (serviço `netmonitor-init` do compose)
#
# Roda antes do PostgreSQL e da API, a cada `docker compose up`, e sai. Grava
# no volume `netmonitor-secrets`:
#
#   postgres_password   senha do banco  (lida pelo `postgres`, uid 70)
#   pgpass              a mesma senha no formato do libpq (lida pela API)
#   jwt_secret          assinatura dos logins (base64, HS512)
#   encryption_key      cifra dos segredos guardados no banco
#
# Cada valor vem do `.env`, se definido lá; senão fica o que já está no volume;
# senão é sorteado. Assim `docker compose up -d` sobe sem `.env` nenhum, e
# quem provisiona por automação continua podendo fixar os valores.
#
# ⚠ A `encryption_key` cifra senhas de equipamentos, chaves da VPN e destinos
# de backup: o backup do banco sem ela é ilegível. Para vê-la:
#   docker compose run --rm netmonitor-init show
# ---------------------------------------------------------------------------
set -eu

DIR="${SECRETS_DIR:-/secrets}"
APP_USER="${APP_USER:-app}"
# Usuário `postgres` da imagem `postgres:18-alpine` (na Debian é o 999).
POSTGRES_UID="${POSTGRES_UID:-70}"

umask 077
mkdir -p "${DIR}"

if [ "${1:-}" = "show" ]; then
  for name in postgres_password jwt_secret encryption_key; do
    printf '%s=%s\n' "${name}" "$(cat "${DIR}/${name}" 2>/dev/null || echo '(ainda não gerado)')"
  done
  exit 0
fi

random_base64() { head -c "$1" /dev/urandom | base64 | tr -d '\n'; }
# Só letras e números: a senha também vai no `pgpass`.
random_hex() { head -c "$1" /dev/urandom | od -An -tx1 | tr -d ' \n'; }

# put <arquivo> <valor do .env> <gerador> <dono>
put() {
  file="${DIR}/$1"
  if [ -n "$2" ]; then
    printf '%s' "$2" > "${file}.tmp"
    mv "${file}.tmp" "${file}"
  elif [ ! -s "${file}" ]; then
    "$3" > "${file}.tmp"
    mv "${file}.tmp" "${file}"
    echo "[netmonitor-init] $1 gerado"
  fi
  chown "$4" "${file}"
  chmod 0400 "${file}"
}

gen_postgres() { random_hex 24; }
gen_jwt() { random_base64 64; }
gen_key() { random_base64 32; }

put postgres_password "${POSTGRES_PASSWORD:-}" gen_postgres "${POSTGRES_UID}"
put jwt_secret "${JWT_SECRET:-}" gen_jwt "${APP_USER}"
put encryption_key "${ENCRYPTION_KEY:-}" gen_key "${APP_USER}"

# `host:porta:banco:usuário:senha`; `\` e `:` escapados como pede o libpq. O
# `sqlx` ignora o arquivo se ele não for 0600.
escaped=$(sed 's/\\/\\\\/g; s/:/\\:/g' "${DIR}/postgres_password")
printf '*:*:*:netmonitor:%s\n' "${escaped}" > "${DIR}/pgpass.tmp"
mv "${DIR}/pgpass.tmp" "${DIR}/pgpass"
chown "${APP_USER}" "${DIR}/pgpass"
chmod 0600 "${DIR}/pgpass"
