# Imagem do backend Rust (§13 do roadmap).
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

# ---------------------------------------------------------------- builder ---
FROM rust:slim-bookworm AS builder

WORKDIR /usr/src/app

# `pkg-config`/`libssl-dev` para as crates que compilam contra o sistema;
# `ca-certificates` para o DoH do checker de DNS funcionar já no build de teste.
RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY . .

RUN cargo build --release --bin backend_rust-cli

# ------------------------------------------------------------------ spike ---
# Estágio usado só por `docker-compose.icmp-spike.yml`. Compila os protótipos
# da Fase 0 para poderem ser executados dentro do mesmo ambiente da imagem
# final — que é justamente o que SPIKE-03 precisa responder.
FROM builder AS spike-builder
RUN cargo build --release --examples

FROM debian:bookworm-slim AS spike

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates iputils-ping \
    && rm -rf /var/lib/apt/lists/*

# Usuário sem privilégio: se o ICMP DGRAM funcionar aqui, funciona em produção.
RUN useradd --create-home --shell /usr/sbin/nologin app
COPY --from=spike-builder /usr/src/app/target/release/examples/spike_icmp_dgram /usr/local/bin/
COPY --from=spike-builder /usr/src/app/target/release/examples/spike_dns_wire /usr/local/bin/
COPY --from=spike-builder /usr/src/app/target/release/examples/spike_snmp_v2c /usr/local/bin/
USER app
CMD ["spike_icmp_dgram", "1.1.1.1"]

# ------------------------------------------------------------------- app ----
FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --create-home --shell /usr/sbin/nologin app
WORKDIR /app

COPY --from=builder /usr/src/app/target/release/backend_rust-cli /usr/local/bin/backend_rust-cli
COPY --from=builder /usr/src/app/config /app/config

USER app

ENV LOCO_ENV=production
EXPOSE 3333

# `GET /` é o mesmo health check do backend AdonisJS (§5.6).
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ["/usr/local/bin/backend_rust-cli", "doctor"]

CMD ["/usr/local/bin/backend_rust-cli", "start", "--server-and-worker"]
