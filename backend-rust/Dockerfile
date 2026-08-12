# Imagem do backend Rust (Loco.rs).
#
# Duas decisões estão materializadas aqui e vêm de spikes da Fase 0:
#
# * **SPIKE-03 (ICMP)** — a imagem final **não** recebe `CAP_NET_RAW`. O ping
#   usa socket ICMP `SOCK_DGRAM`, liberado por
#   `sysctl net.ipv4.ping_group_range` (aplicado no compose, não aqui: sysctl é
#   config de runtime do namespace de rede, não da imagem). Ver
#   `docs/adr/003-icmp-dgram.md`.
# * **Usuário não-root** — o processo roda como `app`. Isso só é possível
#   porque o ping dispensa capability; com raw socket seria preciso root ou
#   `setcap`.
#
# Uma imagem só serve os quatro papéis (`server`, `scheduler`, `probe`,
# `vpn-probe`) — o que muda é o comando. Ver `docker-compose.yml`.

# ---------------------------------------------------------------- builder ---
FROM rust:slim-bookworm AS builder

WORKDIR /usr/src/app

# `pkg-config`/`libssl-dev` para as crates que compilam contra o sistema;
# `ca-certificates` para o DoH do checker de DNS funcionar já no build de teste.
RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY . .

# Os dois `cache` mounts são o que separa "recompilar o projeto" de "recompilar
# 400 crates de terceiros". Sem eles, qualquer alteração de fonte invalida o
# `COPY . .` e o build recomeça do zero — cinco minutos por vez.
#
# O `target/` fica **dentro** do cache mount, que não existe na camada final.
# Por isso o binário é copiado para fora ainda dentro deste mesmo `RUN`: depois
# que ele termina, o diretório some.
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/src/app/target,sharing=locked \
    cargo build --release --bin backend_rust-cli \
    && cp target/release/backend_rust-cli /usr/local/bin/backend_rust-cli

# ------------------------------------------------------------------ spike ---
# Estágio usado só por `docker-compose.icmp-spike.yml`. Compila os protótipos
# da Fase 0 para poderem ser executados dentro do mesmo ambiente da imagem
# final — que é justamente o que SPIKE-03 precisa responder.
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

# ------------------------------------------------------------------- app ----
FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --create-home --shell /usr/sbin/nologin app
WORKDIR /app

COPY --from=builder /usr/local/bin/backend_rust-cli /usr/local/bin/backend_rust-cli
COPY --from=builder /usr/src/app/config /app/config

USER app

ENV LOCO_ENV=production
EXPOSE 3333

# `--production` é obrigatório aqui, não um detalhe de gosto: sem ele o `doctor`
# roda `check_deps()`, que lê o `Cargo.lock` — e a imagem de runtime só recebe o
# binário e o `config/`. O check falha com `VersionCheck(LockfileError(...))` e
# todo container nasce `unhealthy` mesmo com a API respondendo. Com a flag,
# sobra o que interessa em produção: a conexão com o banco.
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ["/usr/local/bin/backend_rust-cli", "doctor", "--production"]

# Padrão da imagem: o servidor HTTP. Os outros papéis sobrescrevem o comando no
# compose. Sem `--server-and-worker`: não há worker de fila registrado, e pedir
# um subiria uma fila vazia para ninguém consumir.
CMD ["/usr/local/bin/backend_rust-cli", "start"]
