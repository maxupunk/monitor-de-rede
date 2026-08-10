# ADR 001 — Cliente SNMP: `rasn-snmp` com transporte próprio

- **Spike:** SPIKE-01 (§3.4 do `roadmap_backend_rust.md`)
- **Status:** aceito — Fase 0
- **Data:** 2026-08-10
- **Protótipo:** [`backend-rust/examples/spikes/snmp_v2c.rs`](../../backend-rust/examples/spikes/snmp_v2c.rs)

## Contexto

O backend AdonisJS fala SNMP v1/v2c/v3 com 6 coletores (`system`, `interface`,
`traffic`, `cpu`, `memory`, `lldp`). A migração precisa de um cliente Rust que
faça `get` e `walk` (GETNEXT/GETBULK) sem bloquear o runtime `tokio`, já que o
poll SNMP roda dentro do mesmo processo do scheduler.

Duas opções foram consideradas:

1. **`rasn` + `rasn-snmp`** — só o codec ASN.1/BER. O transporte é nosso, sobre
   `tokio::net::UdpSocket`.
2. **`snmp2`** — cliente completo, porém **síncrono**; exigiria envolver cada
   chamada em `spawn_blocking`.

## Decisão

**`rasn-snmp` 0.18 com transporte próprio em `tokio::net::UdpSocket`.**

## Evidência

O protótipo roda em dois modos. Offline (o que roda em CI):

```
$ cargo run --example spike_snmp_v2c
[ok] GetRequest de sysDescr.0 codificado em 40 bytes
[ok] round-trip BER preserva request_id, community e OID
[ok] GetNextRequest (base do walk) também fecha o ciclo
```

Ao vivo (`SNMP_TARGET=host:161`), o mesmo binário lê `sysDescr.0` e percorre a
coluna `ifDescr` da `ifTable` por GETNEXT, parando quando o OID devolvido sai de
baixo do prefixo — que é exatamente o laço do `walk` do cliente definitivo.

Os tipos necessários existem e são completos: `rasn_snmp::v2c::Message<T>`,
`v2::{Pdus, GetRequest, GetNextRequest, GetBulkRequest, Pdu, VarBind,
VarBindValue}` e `v3` com USM. Decodificar em `Message<Pdus>` (o `choice`)
funciona sem saber de antemão qual PDU chega — requisito de um cliente real.

## Consequências

**Positivas**

- Sem `spawn_blocking`: o poll SNMP compartilha o runtime com os demais
  checkers, sem uma thread pool paralela dimensionada no escuro.
- Controle total de timeout, retry e concorrência por alvo — o `snmp2` impõe os
  dele.
- `rasn` cobre v1, v2c e v3/USM no mesmo codec, então as três versões do §7.9
  usam um caminho só.

**Negativas / trabalho extra que isto cria**

- **O transporte é nosso.** Socket, `request_id`, correlação de resposta,
  timeout e retry precisam ser escritos e testados. Vai em
  `src/services/snmp/client.rs`.
- **Achado do spike:** `EncodeError` e `DecodeError` do `rasn` 0.18 **não**
  implementam `std::error::Error`. O `?` não converte para `anyhow` nem para
  `Box<dyn Error>`. O cliente precisa de um `SnmpError` (`thiserror`) com
  `From` explícito para cada um — não é opcional, é a única forma de propagar.
- `version: 1` significa SNMP**v2c** (RFC 1901 desloca a numeração em um).
  Trocar isso por engano faz o agente descartar o pacote **em silêncio**, sem
  erro. Está comentado no protótipo e deve ser comentado no cliente.

## Alternativa recusada

`snmp2` em `spawn_blocking`: cada `get` ocuparia uma thread do pool bloqueante
pelo tempo do timeout de rede (até 5 s). Com dezenas de dispositivos SNMP em
poll simultâneo, o pool vira o gargalo — e a §1.3.5 exige que nenhum caminho de
rede derrube ou trave a task.
