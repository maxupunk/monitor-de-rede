#!/bin/sh
# Instala no volume do Ollaya os modelos de decisão que vêm com o NetMonitor.
#
# Cada pasta aqui é um modelo, como o Laya-tuning o empacota (`laya-tuning bundle`):
#   model          nome no Ollaya (`ollaya.dev/netmonitor/decima-network:v5`)
#   manifest.json  o manifesto, gravado por último — o Ollaya só lista modelo completo
#   layers.tsv     `digest<TAB>tamanho<TAB>url` de cada blob (config + camadas)
#   blobs/         opcional: os blobs sem URL (os pesos ajustados), para instalar
#                  sem internet; sem ela, vêm de `$DECIDER_MODELS_URL/sha256-<hex>`
#
# Idempotente e seguro de rodar a cada `docker compose up`: modelo já instalado
# com o mesmo manifesto não baixa nada; todo blob é conferido pelo sha256.
set -eu

STORE=/ollaya/models
OWNER=${OLLAYA_UID:-1000}:${OLLAYA_GID:-1000}
# Vazio ou ausente: a release do projeto, onde os pesos de cada versão ficam.
MODELS_URL=${DECIDER_MODELS_URL:-https://github.com/maxupunk/monitor-de-rede/releases/download/decider-models}
TAB=$(printf '\t')

sha256_of() { sha256sum "$1" | cut -c1-64; }

fetch() { # url destino
  wget -q -T 60 -O "$2" "$1"
}

install_blob() { # pasta digest url
  hex=${2#sha256:}
  blob="$STORE/blobs/sha256-$hex"
  [ -f "$blob" ] && [ "$(sha256_of "$blob")" = "$hex" ] && return 0
  partial="$blob-partial"
  if [ -f "$1/blobs/sha256-$hex" ]; then
    cp "$1/blobs/sha256-$hex" "$partial"
  else
    source=${3:-${MODELS_URL%/}/sha256-$hex}
    echo "  baixando sha256-$(printf %.12s "$hex") de $source"
    fetch "$source" "$partial" || { rm -f "$partial"; echo "  falhou: $source" >&2; return 1; }
  fi
  if [ "$(sha256_of "$partial")" != "$hex" ]; then
    rm -f "$partial"
    echo "  sha256 não confere: $2" >&2
    return 1
  fi
  mv "$partial" "$blob"
}

install_model() { # pasta
  name=$(cat "$1/model")
  manifest="$STORE/manifests/${name%:*}/${name##*:}"
  if [ -f "$manifest" ] && cmp -s "$manifest" "$1/manifest.json"; then
    echo "$name: já instalado"
    return 0
  fi
  echo "$name: instalando"
  mkdir -p "$STORE/blobs" "$(dirname "$manifest")"
  while IFS="$TAB" read -r digest _size url; do
    [ -n "$digest" ] || continue
    install_blob "$1" "$digest" "$url" || return 1
  done < "$1/layers.tsv"
  cp "$1/manifest.json" "$manifest"
  echo "$name: pronto"
}

mkdir -p "$STORE"
status=0
for dir in /bundle/*/; do
  [ -f "$dir/model" ] || continue
  install_model "${dir%/}" || status=1
done
# O Ollaya roda como usuário comum: ele precisa continuar gravando os próprios
# downloads nas mesmas pastas.
chown -R "$OWNER" "$STORE"
exit "$status"
