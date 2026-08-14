# ADR 004 — DNS: `hickory-proto` no formato wire, sem cliente pronto

- **Spike:** SPIKE-04 (§3.4 do `roadmap_backend_rust.md`)
- **Status:** aceito — Fase 0
- **Data:** 2026-08-10
- **Protótipo:** [`backend/examples/spikes/dns_wire.rs`](../../backend/examples/spikes/dns_wire.rs)

## Contexto

O `DnsLatencyCard` e o `POST /api/dns/benchmark` **comparam resolvedores**. O
número que aparece na tela só significa algo se o cronômetro cobrir a etapa de
resolução e nada mais. Um cliente pronto (`hickory-resolver`) esconde dentro da
mesma chamada a criação do cliente, o `connect` e a leitura de configuração do
sistema — tudo isso entraria na medição e mediria a máquina local, não o
servidor DNS.

Pergunta do spike: `hickory-proto` permite montar e ler o pacote à mão,
mantendo o `Instant` só em volta do round-trip — para UDP, TCP e DoH?

## Decisão

**Sim. Usar `hickory-proto` 0.24 como codec e escrever o transporte, com o
cronômetro isolado no round-trip.**

## Evidência

```
$ cargo run --example spike_dns_wire
[ok] round-trip wire preserva id e pergunta (29 bytes)
[ok] UDP  1.1.1.1:53: 18.573 ms — 172.66.147.243, 104.20.23.154
[ok] TCP  1.1.1.1:53: 25.557 ms — 104.20.23.154, 172.66.147.243
[ok] DoH  https://cloudflare-dns.com/dns-query: 125.217 ms — 104.20.23.154, 172.66.147.243
```

Os três transportes funcionam com **um único encoder**. A diferença entre eles
é só o envelope:

| Transporte | Envelope | Onde fica o `Instant` |
| :--- | :--- | :--- |
| UDP | nenhum | em volta de `send` + `recv` |
| TCP | prefixo de 2 bytes com o tamanho (RFC 1035 §4.2.2) | depois do `connect`, em volta do write/read |
| DoH | POST `application/dns-message` (RFC 8484) | em volta do `send()` do `reqwest` |

O `connect` do TCP fica **fora** da medição de propósito: ele mede o
estabelecimento da sessão, não a resolução. Essa é a linha que um cliente
pronto não deixaria traçar.

A prova offline (encode → decode preserva `id` e pergunta) roda sem rede e serve
de teste de CI.

## Consequências

- `hickory-proto` entra com `default-features = false`: só o codec, sem o
  runtime de resolver, sem TLS próprio, sem `resolv.conf`.
- O `id` da consulta é sorteado a cada pergunta. Não é detalhe estético: um
  resolvedor descarta resposta cujo `id` não bate, e `id` fixo abre cache
  poisoning trivial.
- O DoH reaproveita o `reqwest` já presente (rustls). Nenhuma dependência nova.
- O transporte é nosso: timeout, retry e a escolha UDP→TCP no truncamento
  (`TC=1`) ficam em `src/services/network_tools/dns/wire.rs`.
