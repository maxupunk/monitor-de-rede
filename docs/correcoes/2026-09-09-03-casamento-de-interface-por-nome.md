# Correção 03 — Interface casada por identidade estável, não por `ifIndex`

- **Data:** 2026-09-09
- **Severidade:** alta (corrompia a identidade de portas e era a causa das duplicatas)
- **Arquivos:** `backend/src/services/snmp/service.rs`

## O defeito

`poll_device` e `apply_monitors` casavam a interface da varredura com a linha do banco
pelo **índice primeiro**, com o nome só como plano B:

```rust
let existing = existing_by_index
    .get(&interface.if_index)
    .or_else(|| existing_by_name.get(&interface.if_name.to_lowercase()));
```

O `ifIndex` não é identidade: é um número que o agente reaproveita. Isso quebrava em dois
cenários, ambos rotineiros num roteador OpenWrt:

**1. Índice reaproveitado.** `pppoe-wan` gravada no índice 34; o roteador reinicia, a
PPPoE sobe no 55 e o 34 passa a pertencer à `br-lan`. Ao sincronizar `br-lan@34`, o
casamento por índice encontrava a linha da **`pppoe-wan`** e a renomeava para `br-lan`. O
histórico da porta continuava colado num registro que agora descreve outra coisa —
silenciosamente, sem erro nenhum.

**2. Mapas cegos ao próprio laço.** Os dois mapas eram montados uma vez, antes do laço, e
nunca atualizados com o que o laço criava ou alterava. Duas interfaces da mesma varredura
podiam resolver para a mesma linha e sobrescrever uma à outra: duas portas viravam uma.

Havia ainda a duplicação literal — a mesma lógica escrita duas vezes, em duas funções que
precisam concordar. Foi divergindo delas que a duplicata de produção nasceu.

## O que foi feito

### `InterfaceMatcher`

Uma estrutura única, usada pelas duas funções, com três regras:

**Nome antes do índice.** A identidade estável de uma porta é o `ifName`. PPPoE renumera a
cada reconexão, mas continua se chamando `pppoe-wan`. Invertida a ordem, o cenário 1
resolve certo: `br-lan@34` procura por *nome* e encontra a linha da `br-lan`.

**Nome repetido fica de fora.** Um agente SNMP pode reportar duas portas com o mesmo
`ifName`. Aí o nome não identifica ninguém, e casar por ele faria as duas disputarem a
mesma linha. Nomes ambíguos são removidos do índice por nome e caem no `ifIndex`, que ao
menos as distingue. É também por isso que a correção 02 não criou um `UNIQUE` no banco: o
código tolera esse caso, o esquema não toleraria.

**Cada linha vale por uma varredura.** `claim` **consome** a linha (`rows.remove`). A
segunda pretendente não a encontra e vira registro novo — que é o que ela é. Isso mata o
cenário 2 sem reconstruir mapa nenhum dentro do laço.

### Efeito colateral bem-vindo

O casamento deixou de existir em duas cópias. `poll_device` e `apply_monitors` agora
chamam `conhecidas.claim(source)` e não têm mais como divergir.

## Testes

Cinco testes unitários novos em `services::snmp::service::tests`:

| Teste | O que trava |
|---|---|
| `interface_renumerada_casa_pelo_nome` | o caso PPPoE: idx 34 → 55, mesma linha |
| `indice_reaproveitado_nao_sequestra_a_linha_de_outra_interface` | `br-lan@34` casa com a `br-lan`, não com a `pppoe-wan` |
| `nome_repetido_no_aparelho_cai_para_o_indice` | duas portas `lan` continuam distintas |
| `uma_linha_so_e_reivindicada_uma_vez` | duas portas não colapsam em uma |
| `interface_desconhecida_nao_casa` | porta nova vira registro novo |

## Validação

```
cargo clippy --all-targets -- -D warnings   # limpo
cargo fmt --all
cargo test    # 942 unitários + 307 de requisição, 0 falhas
```

## Relação com as outras correções

Esta é a que **impede a recorrência** da duplicata que a correção 02 limpou. A 02
conserta o dado que já existe; a 03 conserta a causa.
