# Correção 02 — Fusão das interfaces duplicadas por (dispositivo, nome)

- **Data:** 2026-09-09
- **Severidade:** alta (dado sujo em produção, com sintoma visível no painel)
- **Arquivos:** `backend/migration/src/m20260909_000001_merge_duplicate_device_interfaces.rs`,
  `backend/migration/src/lib.rs`, `backend/tests/requests/interface_duplicate_merge.rs`

## O defeito

O backup de produção (`netmonitor-backup-20260909-193530.json`) mostra duas linhas em
`device_interfaces` com o mesmo nome no dispositivo 2:

| id | snmp_index | name | last_seen_at |
|----|-----------|------|--------------|
| 17 | 34 | `pppoe-wan` | 2026-08-27T21:40:00Z |
| 21 | 55 | `pppoe-wan` | 2026-09-09T19:35:14Z |

Uma interface PPPoE troca de `ifIndex` a cada reconexão. Até o commit `e4c83f4`
(27/08 23:28 UTC) a sincronização casava a interface do walk com a do banco **só pelo
índice**: o índice novo não encontrava ninguém e uma linha nova era inserida, deixando a
antiga órfã. A linha 17 parou de ser vista às 21:40 UTC daquele mesmo dia — dentro da
janela em que o casamento por nome ainda não existia.

A órfã não é inofensiva:

- **Porta duplicada no painel.** `list_interfaces` calcula `is_monitored` pelo **nome**
  do monitor (`Interface pppoe-wan`), então as duas linhas se dizem monitoradas. Daí as
  duas `pppoe-wan` em "Tráfego por interface monitorada", uma delas eternamente em 0 bps.
- **Aviso de remoção que não sai.** A tela de descoberta marca como "desmarcada do
  monitoramento" toda interface `isMonitored` cujo índice não está na seleção. O índice
  34 não aparece na varredura, então é impossível marcá-lo: o aviso "Remover do
  Monitoramento & Histórico?" reaparecia em toda gravação e nenhuma das duas opções o
  resolvia.
- **A limpeza automática não alcança.** O laço de órfãs em `apply_monitors` é
  condicionado a `!discovered_names.contains(name)` — e o nome **está** entre os
  descobertos, porque a interface viva é homônima. O guarda protegia exatamente a linha
  que deveria remover.

## O que foi feito

Migração `m20260909_000001_merge_duplicate_device_interfaces`, registrada no `Migrator`.

### Quem sobrevive

A de `last_seen_at` mais recente; nulas por último; desempate pelo maior `id`.

```sql
ORDER BY (k2.last_seen_at IS NULL), k2.last_seen_at DESC, k2.id DESC
```

O `(last_seen_at IS NULL)` explícito não é enfeite: `ORDER BY ... DESC` põe nulo por
**último** no SQLite e por **primeiro** no PostgreSQL. Sem ele, uma linha nunca vista
venceria a viva em produção e perderia em teste — o pior tipo de divergência de dialeto.

### O que é repontado antes do `DELETE`

Nenhuma das quatro referências tem FK declarada, então o banco não reclamaria de um
ponteiro órfão: o histórico simplesmente sumiria do gráfico.

- `metrics.interface_id`
- `device_links.source_interface_id`
- `device_links.target_interface_id`
- `devices.link_interface_id` — em produção o dispositivo 2 aponta a interface como
  entrada de link (`link_interface_id = 21`), então este caso é real, não hipotético.

Os ids são interpolados com `format!` em vez de vinculados como parâmetro porque vêm do
próprio banco (`i64`) e porque o marcador de parâmetro diverge entre os dialetos (`?` no
SQLite, `$1` no PostgreSQL). É o mesmo recurso que `m20260819_000002` já usa.

### Validação prévia do SQL contra os dados reais

Antes de escrever a migração, a consulta que escolhe a sobrevivente foi rodada num
SQLite em memória carregado com as 55 linhas de `device_interfaces` do backup de
produção, mais três casos sintéticos. Resultado:

| perdedora | vencedora | por quê |
|---|---|---|
| 17 (`pppoe-wan`, 27/08) | 21 (`pppoe-wan`, 09/09) | dado real: a viva vence |
| 900 (`Wan0`, NULL) | 901 (`wan0`, NULL) | empate em nulo → maior id; casa ignorando caixa |
| 902 (`lan`, NULL) | 903 (`lan`, 01/01) | datada vence a nunca vista |

Nos dados de produção a consulta devolveu **exatamente um** par — só a `pppoe-wan`.

## Testes

`backend/tests/requests/interface_duplicate_merge.rs`, dois casos:

- `a_interface_orfa_e_fundida_na_viva_sem_perder_referencia` — monta o cenário real
  (órfã idx 34, viva idx 55, uma `sfp2` de controle, métrica, enlace nas duas pontas e
  `link_interface_id` apontando a órfã) e afirma que sobra uma linha, que é a viva, que
  a `sfp2` não foi tocada e que as quatro referências passaram a apontar para a viva.
- `a_fusao_e_idempotente` — roda duas vezes sem duplicata; o `auto_migrate` reexecuta o
  conjunto a cada subida.

O helper busca a migração pela lista do `Migrator` em vez de instanciar o tipo. Isso
também afirma que ela está registrada no `lib.rs` — esquecer a linha `Box::new(...)` é o
jeito fácil de escrever uma migração que nunca roda.

## Validação

```
cargo clippy --all-targets -- -D warnings   # limpo
cargo fmt --all
cargo test --test mod interface_duplicate_merge   # 2 passed
```

## Decisão consciente: sem índice único

Um `UNIQUE (device_id, LOWER(name))` impediria a recorrência pelo banco, mas quebraria
a coleta de qualquer agente SNMP que reporte duas interfaces com o mesmo `ifName` — o
que existe no mundo real. Numa ferramenta que conversa com equipamento arbitrário, o
custo de errar aqui é a coleta parar de gravar.

A recorrência é barrada no código, pela correção 03, que passa a casar a interface por
identidade estável em vez de índice.
