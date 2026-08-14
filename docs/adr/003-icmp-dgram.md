# ADR 003 — ICMP por `SOCK_DGRAM`, sem `CAP_NET_RAW`

- **Spike:** SPIKE-03 (§3.4 do `roadmap_backend_rust.md`)
- **Status:** aceito — Fase 0
- **Data:** 2026-08-10
- **Protótipo:** [`backend/examples/spikes/icmp_dgram.rs`](../../backend/examples/spikes/icmp_dgram.rs)
- **Ambiente de teste:** [`backend/docker-compose.icmp-spike.yml`](../../backend/docker-compose.icmp-spike.yml)

## Contexto

O `PingChecker` atual executa o binário `ping` do sistema e faz *regex* na
saída. A §3.2 substitui isso por `surge-ping`, o que levanta a questão do
privilégio: socket ICMP **raw** exige `CAP_NET_RAW` (ou root). Existe a
alternativa `SOCK_DGRAM` — o socket ICMP não privilegiado do Linux, liberado
por `net.ipv4.ping_group_range`.

Pergunta do spike: `surge-ping` com `sock_type_hint(DGRAM)` funciona no
container base escolhido, sem capability adicional?

## Decisão

**Sim. `SOCK_DGRAM`, sem `CAP_NET_RAW`, com o processo rodando como usuário
não-root.**

## Evidência

Container `debian:bookworm-slim`, usuário `app` (não-root), **sem** `cap_add`:

```
$ docker compose -f docker-compose.icmp-spike.yml run --rm icmp-dgram
[ok] socket ICMP DGRAM aberto sem privilégio adicional
[1.1.1.1] latência média 25.74 ms, perda 0% — status `up`
[8.8.8.8] latência média 65.67 ms, perda 0% — status `up`
```

### O sysctl já é o default do Docker

```
$ docker run --rm debian:bookworm-slim cat /proc/sys/net/ipv4/ping_group_range
0	2147483647
```

Nesta versão do Docker (29.6.2) o intervalo permissivo já vem por padrão dentro
do container. **Isso não torna o sysctl dispensável no compose:** o default do
kernel Linux puro é `1 0` (intervalo vazio), e em Kubernetes com sysctls
restritos, ou num `dockerd` antigo, o socket não abriria. A linha declarada é a
diferença entre "funciona aqui" e "funciona onde for implantado".

### Contraprova

Mesma imagem, `ping_group_range` fechado:

```
$ docker compose -f docker-compose.icmp-spike.yml run --rm icmp-restrito
[FALHA] não foi possível abrir o socket ICMP DGRAM: Operation not permitted (os error 1)
```

O que libera o socket é o sysctl, não um privilégio herdado por acidente.

### Latência bate com o `ping` do sistema (critério da §3.2)

Dentro do mesmo container, contra `1.1.1.1`:

| Fonte | Média |
| :--- | ---: |
| `ping -c 3` (iputils) | 23,34 ms |
| protótipo (`surge-ping` DGRAM) | 24,07 ms |

Diferença de ~3% — dentro dos ±10% exigidos.

### Windows (desenvolvimento local)

A §3.2.3 previa que o Windows exigiria processo elevado e um *fallback* para
`ping.exe`. **Medido: não exige.** O mesmo protótipo, sem elevação:

```
> cargo run --example spike_icmp_dgram -- 1.1.1.1 8.8.8.8
[ok] socket ICMP DGRAM aberto sem privilégio adicional
[1.1.1.1] latência média 14.41 ms, perda 0% — status `up`
[8.8.8.8] latência média 76.54 ms, perda 0% — status `up`
```

**Consequência para a Fase 3:** o *fallback* `#[cfg(windows)]` para `ping.exe`
previsto na §3.2.3 **não é necessário** e não deve ser escrito. Escrever um
caminho alternativo que nunca é exercitado é dívida garantida — o parsing por
idioma do SO é exatamente o defeito que esta migração remove. Se algum ambiente
Windows falhar, o checker devolve resultado degradado (§1.3.5) e o erro aparece
no log; aí sim se reavalia com um caso concreto.

## Consequências

- A imagem de produção **não** recebe `CAP_NET_RAW` e o processo roda como
  usuário sem privilégio (`Dockerfile`, estágio `runtime`). Uma coisa depende da
  outra: com raw socket, seria root ou `setcap`.
- O `docker-compose.yml` de produção precisa declarar
  `sysctls: net.ipv4.ping_group_range: "0 2147483647"` no serviço do backend —
  não é opcional fora do Docker Desktop.
- O `surge_ping::Client` é criado **uma vez por processo**, no initializer
  `ping_client` (§9.5), e compartilhado entre o monitor e o *sweep* ICMP do
  discovery. Cada medição usa seu próprio `PingIdentifier`; é o `Client` que
  multiplexa as respostas.
- **Correção na §3.1:** `socket2` fica em `0.6`, não `0.5`. O `surge-ping` 0.8
  depende de `socket2 ^0.6`, e `sock_type_hint` recebe o `socket2::Type`
  **daquela** versão. Com as duas majors na árvore, o tipo não unifica e o
  código não compila.
