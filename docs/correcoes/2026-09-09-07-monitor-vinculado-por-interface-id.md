# Correção 07 — Monitor vinculado à interface por `interface_id`

- **Data:** 2026-09-09
- **Severidade:** alta (causa raiz do `is_monitored` errado e do monitor compartilhado)
- **Arquivos:** `backend/migration/src/m20260909_000002_monitors_interface_id.rs`,
  `backend/migration/src/lib.rs`, `backend/src/models/_entities/monitors.rs`,
  `backend/src/services/snmp/service.rs`,
  `backend/tests/requests/monitor_interface_binding.rs`,
  `backend/tests/requests/snmp_collection_integration.rs`

## O defeito

O monitor de uma porta se chama `Interface {ifName}`, e essa **string** era o único elo
entre ele e a linha de `device_interfaces`. Toda pergunta "esta interface é monitorada?"
virava comparação de texto:

```rust
is_monitored: monitored.contains(&interface_monitor_name(&row.name)),
```

Três defeitos saem daí, e nenhum tem conserto sem a coluna:

1. **Homônimas dividem um monitor.** Duas linhas com o mesmo `ifName` — o que sobra
   quando o `ifIndex` de uma PPPoE muda e a antiga fica órfã — resolvem para o mesmo
   `Interface pppoe-wan`. As duas se declaram monitoradas: é literalmente a porta
   duplicada que aparece no painel, uma delas eternamente em 0 bps.
2. **Renomear a porta orfana o monitor.** O operador troca `lan1` por `uplink` no
   equipamento. A cadeia de identidade (correção 06) reencontra a linha, mas o monitor
   continua chamado `Interface lan1` e ninguém mais o acha — um segundo monitor nasce
   para a mesma porta.
3. **A remoção erra o alvo.** A limpeza de interfaces sumidas procurava o monitor pelo
   nome da linha que estava apagando. Com outra porta homônima viva, apagava o monitor
   da porta errada.

## O que foi feito

### Migração `m20260909_000002_monitors_interface_id`

- `ALTER TABLE monitors ADD COLUMN interface_id` (nulo), mais índice.
- **Backfill** casando `monitors.name = 'Interface ' || device_interfaces.name` dentro do
  mesmo dispositivo. Roda **depois** da fusão (`m20260909_000001`), então cada nome
  resolve para uma linha só; há dedupe por `monitor_id` como cinto de segurança.
- Monitores sem interface (`cpu_usage`, `memory_usage`, ping, TCP) ficam com
  `interface_id` nulo — que é o que eles são: monitores do dispositivo.

### Os quatro caminhos que dependiam do nome

| Caminho | Antes | Agora |
|---|---|---|
| `sync_monitor` | busca por `(device_id, name)` | por `interface_id` quando há interface; **renomeia o monitor junto** com a porta |
| `set_monitoring` | passava só o nome | passa `Some(interface.id)` |
| limpeza em `apply_monitors` | achava o monitor pelo nome da linha removida | pelo `interface_id` |
| `list_interfaces` | conjunto de **nomes** de monitor | conjunto de `interface_id` |
| `scan_device` | `is_monitored` por nome sobre o resultado da varredura | resolve a linha pela cadeia de identidade e então consulta o vínculo |

O `scan_device` é o mais sutil: a varredura não conhece o `id` da linha local, então é a
cadeia de identidade que liga a porta recém-lida ao registro — e só então a pergunta
"esta é monitorada?" tem um alvo. Era daí que vinha o diálogo de descoberta acusando
"interface removida" a cada gravação, para uma órfã que o operador não tinha como
selecionar.

### `MonitorSpec`

`sync_monitor` chegou a 8 argumentos e o clippy recusou (limite de 7). Em vez de afrouxar
o lint, os parâmetros viraram uma struct nomeada — o que também elimina a chance de trocar
`enabled` por `up`, dois `bool` adjacentes na assinatura antiga.

## Mudança de contrato: o vínculo agora é explícito

Um monitor com `interface_id` nulo **não** conta mais como monitorando porta nenhuma,
mesmo que se chame `Interface Gi0/3`. Antes, essa coincidência de nome marcava a porta
como monitorada.

Isso quebrou um teste existente
(`a_interface_so_conta_como_monitorada_quando_tem_monitor_habilitado`), que criava o
monitor só pelo nome. **Não foi um teste desatualizado que se ajustou por conveniência** —
é a semântica que mudou de propósito, e o teste foi atualizado para declarar o vínculo,
com o comentário explicando por quê. Instalações existentes são cobertas pelo backfill.

## Testes

`backend/tests/requests/monitor_interface_binding.rs` (novo, 3 casos):

- `o_backfill_liga_cada_monitor_a_sua_interface` — dois monitores de porta ligados
  corretamente; CPU e ping ficam sem vínculo.
- `homonimas_deixam_de_se_declarar_monitoradas_juntas` — o cenário de produção (órfã idx
  34 + viva idx 55, mesmo nome): só a vinculada aparece monitorada, e a órfã continua
  listada, porque quem a remove é a fusão, não esta mudança.
- `o_backfill_e_idempotente` — o `auto_migrate` reexecuta a cada subida.

## Validação

```
cargo clippy --all-targets -- -D warnings   # limpo
cargo fmt --all
cargo test    # 947 unitários + 310 de requisição, 0 falhas
pnpm --prefix frontend run typecheck / test  # limpo, 94 testes
```

## O que fica de fora

A **órfã que não casa com nenhum sinal** (`ppp0` → `ppp1`, onde nome e índice mudam
juntos) continua vivendo para sempre no banco. A opção de retenção por `last_seen_at` no
`data_pruner` — que já tem `retention_days` configurável e não pediria coluna nova — foi
apresentada e ficou de fora do escopo escolhido.

Uma consequência do vínculo explícito: com `interface_id` nulo em monitor de porta criado
fora do fluxo SNMP (pela API genérica de monitores, por exemplo), a porta não se declara
monitorada. É o comportamento pretendido, mas vale saber ao diagnosticar.
