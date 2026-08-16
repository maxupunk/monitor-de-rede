# ADR 008 — Syslog: `syslog_loose` com resgate do `<pri>` e severidade por tópico

- **Spike:** SPIKE-06 (Fase 1 do [`roadmap_syslog_nativo.md`](../roadmap_syslog_nativo.md))
- **Status:** aceito
- **Data:** 2026-08-15
- **Protótipo:** [`backend/examples/spikes/syslog_parse.rs`](../../backend/examples/spikes/syslog_parse.rs)

## Contexto

O servidor de syslog nativo precisa aceitar o que quatro famílias de
equipamento realmente mandam — RouterOS, OpenWRT, Linux/rsyslog e Ubiquiti — sem
transformar em lixo o que não é RFC 3164 nem RFC 5424. Três perguntas:

1. `syslog_loose` (o parser permissivo do Vector) dá conta das amostras reais?
2. Quanto custa gravar 500 linhas em lote no SQLite em WAL?
3. Qual é o IP de origem observado dentro do container? Toda a regra de "fonte
   desconhecida não grava" depende dele ser o IP do roteador.

## Decisão

**`syslog_loose` 0.23, com duas correções próprias por cima: resgate do `<pri>`
e severidade derivada dos tópicos.**

## Evidência

### As quatro famílias parseiam

```
$ cargo run --example spike_syslog_parse
```

| amostra | facility/severity | device_time | hostname | app/pid | topics |
|---|---|---|---|---|---|
| RouterOS `bsd-syslog` login | 16/6 | ✓ | `MikroTik-CCR` | — | `system,info,account` |
| RouterOS `bsd-syslog` firewall | 16/6 | ✓ | `MikroTik-CCR` | — | `firewall,info` |
| RouterOS **cru** (sem `bsd-syslog`) | 16/6 | — | — | — | `system,info,account` |
| OpenWRT kernel | 0/4 | ✓ | `OpenWrt` | `kernel` | — |
| OpenWRT dnsmasq | 3/6 | ✓ | `OpenWrt` | `dnsmasq-dhcp`/1834 | — |
| Linux RFC 5424 | 20/5 | ✓ | `servidor` | `sshd`/4711 | — |
| Linux RFC 3164 | 4/6 | ✓ | `servidor` | `sshd`/4711 | — |
| Ubiquiti EdgeOS | 3/5 | ✓ | `EdgeRouter` | `dhcpd` | — |
| Ubiquiti UniFi AP | 1/6 | ✓ | `U6-LR` | `hostapd` | — |
| linha sem formato | —/— | — | — | — | — |

A última linha é o contrato do parser permissivo: `parse_message_with_year` não
devolve `Result`, e o que não casa vira mensagem inteira com os campos vazios.
Nada é descartado por não ser RFC.

### Os tópicos do RouterOS caem no lugar do `tag` do BSD

Descoberta do spike: o RouterOS manda `<pri>timestamp hostname topics mensagem`,
e o parser RFC 3164 do `syslog_loose` pega `system,info,account` como
`appname` — porque é o primeiro token depois do hostname. Não é preciso parser
próprio para RouterOS; basta reconhecer o formato `palavra,palavra[,palavra]`
(sem espaço, sem `[pid]`) e mover o campo de `app_name` para `topics`.

### Correção 1 — resgate do `<pri>`

Sem `bsd-syslog=yes`, o RouterOS manda formato próprio: `<pri>` colado nos
tópicos, sem timestamp e sem hostname. O `timestamp` é obrigatório no parser
RFC 3164 do `syslog_loose`, então a linha inteira cai no *fallback* — e o
`<134>` fica preso dentro da mensagem, junto com a severidade.

Resgate: quando o parser não determinou severidade **e** a linha começa com
`<n>`, decompor o pri à mão (`decompose_pri`, exportado pelo crate) e reparsear
o resto. Medido, com o resgate ligado:

```
RouterOS cru (sem bsd-syslog)
  bruto     <134>system,info,account user admin logged in from 192.168.88.50 via winbox
  fac/sev   Some(16)/Some(6)
  topics    Some("system,info,account")
  message   user admin logged in from 192.168.88.50 via winbox
```

**Consequência para a Fase 4:** o snippet de configuração continua recomendando
`bsd-syslog=yes`, mas a flag deixa de ser obrigatória. Sem ela perde-se
`device_time` e `hostname` — não a severidade, não os tópicos, não a mensagem.
O texto do roadmap que dizia "não é opcional" está corrigido.

### Correção 2 — a severidade verdadeira do RouterOS está nos tópicos

Apareceu ao rodar o modo `listen` com pacotes reais. Uma falha de login chega
assim:

```
<131>Aug 15 10:24:01 MikroTik-CCR system,error,critical login failure for user admin
```

O `<pri>` carrega o `syslog-severity` configurado na *action*, que é fixo — e
nas versões sem `auto` vale `info` para tudo. A severidade real está nos
tópicos. **Sem tratar isso, filtrar por severidade não separaria nada num parque
MikroTik: todo log seria "info".**

Regra adotada: quando os tópicos contêm palavra de severidade (`emergency`,
`alert`, `critical`, `error`, `warning`, `info`, `debug`), vence a mais grave
delas; senão vale o `<pri>`. Quando o RouterOS já mandou certo, os dois
concordam e nada muda. Medido: a falha de login acima passa de severidade 3
(`<131>` = err) para 2 (`critical`, o mais grave dos tópicos).

### Resolvedor de ano do RFC 3164

O RFC 3164 não manda o ano. Assumir o corrente erra na virada nas duas direções.
Testar os três anos candidatos e ficar com a data mais próxima de `received_at`
resolve:

```
31/dez 23:59:50 recebido em 2027-01-01T00:00:30Z -> 2026
01/jan 00:00:10 recebido em 2026-12-31T23:59:30Z -> 2027
```

### Inserção em lote

```
$ cargo run --example spike_syslog_parse -- bench

  20000 linhas em 40 lotes de 500
  tempo total     234.6656ms
  por lote        5.86664ms
  linhas/s        85228
  arquivo         5791744 bytes (290 B/linha, com 3 índices)
```

Build **debug**, SQLite em WAL com `auto_vacuum=INCREMENTAL` e os três índices
já criados. A 200 msg/s de pico o escritor ocupa **0,23 %** do tempo — margem de
três ordens de grandeza. Confirma a recusa de stack de log externa.

**Correção de dimensionamento:** 290 B/linha é bem menos que os ~400 B
estimados no roadmap. A 12 msg/s isso dá ~301 MB/dia e ~2,1 GB em 7 dias, então
`RETENTION_LOGS_MAX_MB=2048` entregaria quase exatamente os 7 dias — a
contradição apontada na análise é menor do que parecia. O padrão fica em **4096**
mesmo assim: a amostra medida é a mensagem de login (48 caracteres), e linha de
firewall passa de 100. Teto folgado só custa disco quando o disco é usado.

### Pergunta 3 — o IP de origem no container: **não medida**

O modo `listen` foi validado localmente e o caminho socket → parse funciona
ponta a ponta:

```
$ cargo run --example spike_syslog_parse -- listen 5514
origem 127.0.0.1:56138
  sev Some(6) topics Some("system,info,account")
  user admin logged in via winbox
```

Isso prova o listener, **não** o comportamento do Docker. A pergunta que importa
— se a publicação `514:5514/udp` preserva o IP de origem ou o reescreve para o
gateway da bridge — exige a imagem de produção e um pacote vindo de outra
máquina, e continua **em aberto**. Procedimento, para rodar na primeira
implantação:

```sh
# dentro do container
cargo run --example spike_syslog_parse -- listen 5514
# de outra máquina da LAN
logger -n <ip-do-host> -P 514 -d "teste do spike"
```

Se o IP impresso for `172.x.0.1` em vez do IP da máquina de origem, a publicação
está mascarando a origem e o arranjo precisa de `network_mode: host`.

## Consequências

- `syslog_loose = "0.23"` entra no `Cargo.toml`. Traz `nom 8` como transitiva;
  nenhum tipo dela cruza a fronteira do crate, então não repete o problema de
  duas majors do `socket2` da ADR 003.
- `services/syslog/parser.rs` carrega as duas correções (resgate do `<pri>`,
  severidade por tópico) e o resolvedor de ano. As três são código próprio, não
  configuração do crate.
- O snippet do RouterOS recomenda `bsd-syslog=yes` sem exigi-la.
- A verificação do IP de origem atrás do Docker fica como item aberto da
  primeira implantação — é o único critério de saída do spike ainda não medido,
  e é o de maior risco (ver §3 do roadmap).
