# Imagem única do NetMonitor: SPA, API, scheduler e WireGuard.
#
# O que antes eram oito containers cabe aqui porque quase nada daquilo era
# separação de verdade — era o mesmo binário com comandos diferentes, mais um
# nginx para servir arquivos estáticos e reescrever `/api`. O que sobrou de
# processo separado é o único que precisa mesmo: o watcher do WireGuard, que
# roda como root enquanto a aplicação roda como `app`.
#
# Três decisões estão materializadas aqui:
#
# * **SPIKE-03 (ICMP)** — a imagem não recebe `CAP_NET_RAW`. O ping usa socket
#   ICMP `SOCK_DGRAM`, liberado por `sysctl net.ipv4.ping_group_range`
#   (aplicado no compose: sysctl é runtime do namespace de rede, não da
#   imagem). Ver `docs/adr/003-icmp-dgram.md`.
# * **Usuário não-root** — a aplicação roda como `app`, sem capability alguma.
#   O `NET_ADMIN` que o compose concede fica com o watcher, no processo de
#   root; o `--inh-caps=-all` do entrypoint garante que ele não vaze para a
#   aplicação. A fronteira do §7 do AGENTS.md continua de pé: o processo da API
#   não executa `wg` — ele escreve `wg0.conf` e lê `wg0.status`.
# * **Estáticos pré-comprimidos** — o `.gz` de cada arquivo é gerado aqui, no
#   build. Servir o mesmo byte comprimido a cada request custaria CPU para
#   produzir sempre o mesmo resultado.

# ------------------------------------------------------------------- web ----
FROM node:24-alpine AS web

WORKDIR /web
COPY frontend/package*.json ./
RUN npm ci

COPY frontend/ ./
RUN npm run build

# `-k` mantém o original: o `ServeDir` só entrega o `.gz` a quem anunciar
# `Accept-Encoding: gzip`, e ainda precisa do arquivo cru para os demais.
# `-9` porque isto roda uma vez por build, não uma vez por request.
RUN find dist -type f \
      \( -name '*.js' -o -name '*.css' -o -name '*.html' \
         -o -name '*.svg' -o -name '*.json' -o -name '*.webmanifest' \) \
      -size +1k -exec gzip -9 -k {} \;

# --------------------------------------------------------------- builder ----
FROM rust:slim-bookworm AS builder

WORKDIR /usr/src/app

# `pkg-config`/`libssl-dev` para as crates que compilam contra o sistema;
# `ca-certificates` para o DoH do checker de DNS funcionar já no build de teste.
RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY backend/ .

# Os dois `cache` mounts são o que separa "recompilar o projeto" de "recompilar
# 400 crates de terceiros". Sem eles, qualquer alteração de fonte invalida o
# `COPY` e o build recomeça do zero — cinco minutos por vez.
#
# O `target/` fica **dentro** do cache mount, que não existe na camada final.
# Por isso o binário é copiado para fora ainda dentro deste mesmo `RUN`: depois
# que ele termina, o diretório some.
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/src/app/target,sharing=locked \
    cargo build --release --bin backend-cli \
    && cp target/release/backend-cli /usr/local/bin/backend-cli

# ----------------------------------------------------------------- spike ----
# Estágio usado só por `backend/docker-compose.icmp-spike.yml`. Compila os
# protótipos da Fase 0 para poderem ser executados dentro do mesmo ambiente da
# imagem final — que é justamente o que SPIKE-03 precisa responder.
FROM builder AS spike-builder
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/src/app/target,sharing=locked \
    cargo build --release --examples \
    && cp target/release/examples/spike_icmp_dgram \
          target/release/examples/spike_dns_wire \
          target/release/examples/spike_snmp_v2c \
          /usr/local/bin/

FROM debian:bookworm-slim AS spike

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates iputils-ping \
    && rm -rf /var/lib/apt/lists/*

# Usuário sem privilégio: se o ICMP DGRAM funcionar aqui, funciona em produção.
RUN useradd --create-home --shell /usr/sbin/nologin app
COPY --from=spike-builder /usr/local/bin/spike_icmp_dgram /usr/local/bin/
COPY --from=spike-builder /usr/local/bin/spike_dns_wire /usr/local/bin/
COPY --from=spike-builder /usr/local/bin/spike_snmp_v2c /usr/local/bin/
USER app
CMD ["spike_icmp_dgram", "1.1.1.1"]

# --------------------------------------------------------------- runtime ----
FROM debian:bookworm-slim AS runtime

# `wireguard-tools` traz `wg` e `wg-quick`; `iproute2` e `iptables` são o que o
# `wg-quick` chama para criar a interface e aplicar as regras de `PostUp`. Não
# há módulo de kernel aqui: o WireGuard vem do kernel do host (Linux ≥ 5.6,
# incluindo o do WSL2).
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
       ca-certificates wireguard-tools iproute2 iptables \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --create-home --shell /usr/sbin/nologin app
WORKDIR /app

COPY --from=builder /usr/local/bin/backend-cli /usr/local/bin/backend-cli
COPY --from=builder /usr/src/app/config /app/config
COPY --from=web /web/dist /app/web
COPY docker/entrypoint.sh docker/wireguard-watcher.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/entrypoint.sh /usr/local/bin/wireguard-watcher.sh

# Banco (SQLite) e configuração do túnel, no mesmo volume — um só. O entrypoint
# acerta o dono antes de largar o privilégio: volume nomeado nasce do root.
RUN mkdir -p /data/wg && chown -R app:app /data
ENV LOCO_ENV=production \
    WEB_ROOT=/app/web \
    WG_CONFIG_DIR=/data/wg

EXPOSE 3333
EXPOSE 51820/udp

# `--production` é obrigatório aqui, não um detalhe de gosto: sem ele o `doctor`
# roda `check_deps()`, que lê o `Cargo.lock` — e a imagem de runtime só recebe o
# binário, o `config/` e a SPA. O check falha com
# `VersionCheck(LockfileError(...))` e todo container nasce `unhealthy` mesmo
# com a API respondendo. Com a flag, sobra o que interessa em produção: a
# conexão com o banco.
HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
    CMD ["/usr/local/bin/backend-cli", "doctor", "--production"]

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
CMD ["backend-cli", "start"]
