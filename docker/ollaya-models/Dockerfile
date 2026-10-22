# Instalador dos modelos de decisão que vêm com o NetMonitor (serviço
# `ollaya-models` do compose). Leva o pacote de cada modelo; os pesos entram
# na imagem só se estiverem em `<modelo>/blobs/` (instalação sem internet) —
# senão o `install.sh` os baixa de `DECIDER_MODELS_URL`.
FROM alpine:3.20
COPY . /bundle
ENTRYPOINT ["sh", "/bundle/install.sh"]
