# Correção 01 — `apply_monitors` não apaga mais o histórico quando a varredura SNMP falha

- **Data:** 2026-09-09
- **Severidade:** crítica (perda de dados)
- **Arquivos:** `backend/src/services/snmp/service.rs`, `backend/src/services/maintenance/resource_cleanup.rs`

## O defeito

`apply_monitors` interpreta "interface ausente da varredura" como "interface removida
do equipamento" e, com `clear_removed_history = true` (o botão **Apagar Histórico**),
apaga métricas, monitor e a própria linha de `device_interfaces`.

Só que uma coleta que **falhou** também devolve lista vazia. `scan` trata a falha do
coletor de interfaces como não-fatal de propósito — registra em `collector_errors` e
segue com `Ok`, para a tela de descoberta conseguir explicar a seção vazia em vez de
morrer inteira:

```rust
let (interfaces, traffic) = match interfaces_and_traffic {
    Ok(value) => value,
    Err(error) => {
        collector_errors.insert(INTERFACES_COLLECTOR.into(), error.to_string());
        Default::default()          // <- lista vazia
    }
};
```

`apply_monitors` não checava nem `snmp_responded` nem `collector_errors`. Com a lista
vazia, `discovered_indexes` e `discovered_names` ficavam vazios e o laço de limpeza
casava com **todas** as interfaces do equipamento. Nada era recriado depois, porque
`for source in &scan.interfaces` não iterava sobre nada.

O gatilho era barato demais: timeout de 4 s por requisição SNMP e um walk de
`ifTable`/`ifXTable` com muitos PDUs sobre UDP. **Um pacote perdido apagava monitores e
histórico de um equipamento saudável.**

## O que foi feito

### Guarda antes de qualquer escrita

Nova função pura `ensure_scan_can_remove(&SnmpScanResult, known_interfaces) -> AppResult<()>`,
chamada logo após a varredura e **antes** do primeiro `UPDATE`. Recusa com **503** em
três cenários:

1. `!snmp_responded` — agente mudo não é equipamento sem interfaces;
2. erro do coletor `interfaces` em `collector_errors` — o caso real do PDU perdido;
3. lista vazia com interfaces já registradas — rede de segurança para uma falha que
   não vire erro de coletor. Nenhum equipamento perde todas as interfaces de uma vez.

O terceiro teste é deliberadamente condicionado a `known_interfaces > 0`: a primeira
configuração de um equipamento novo, sem interface nenhuma no banco, continua passando.

A leitura de `db_interfaces` subiu para antes da guarda — recusar depois de já ter
mexido no dispositivo não seria recusar.

### Chave do coletor virou constante

`INTERFACES_COLLECTOR` substitui o literal `"interfaces"` nos dois lados (quem escreve
o erro e quem o lê). Com o literal solto, mudar a grafia de um lado só deixaria a
guarda silenciosamente cega.

### Limpeza dentro de uma transação

O laço de remoção passou a rodar em `ctx.db.begin()` / `commit()`. Remover uma interface
são três escritas que só valem juntas (métricas, monitor, linha). Sem a transação, o
pool de uma conexão do SQLite estourando o `connect_timeout` no meio do laço deixava o
equipamento pela metade: interface apagada, monitor de pé.

`ResourceCleanupService::delete_monitor` ficou genérico sobre `ConnectionTrait` para
poder rodar dentro da transação. Os quatro chamadores existentes passam
`&DatabaseConnection` e continuam compilando sem alteração.

O aninhamento do laço também caiu de quatro níveis para um, com `let ... else` e
`continue` no lugar dos `if` encaixados.

## Testes

Cinco testes unitários novos em `services::snmp::service::tests`:

| Teste | Cenário |
|---|---|
| `varredura_sem_resposta_nao_autoriza_remocao` | agente mudo → 503 |
| `falha_do_coletor_de_interfaces_nao_autoriza_remocao` | PDU perdido no walk → 503 |
| `varredura_vazia_com_interfaces_conhecidas_nao_autoriza_remocao` | lista vazia com 21 registradas → 503 |
| `varredura_vazia_sem_interfaces_conhecidas_e_permitida` | cadastro inicial → segue |
| `varredura_com_interfaces_autoriza_remocao` | coleta boa, inclusive com erro de outro coletor → segue |

`SnmpScanResult` ganhou `Default` no derive para os testes construírem cenários sem
repetir dez campos irrelevantes.

## Validação

```
cargo clippy --all-targets -- -D warnings   # limpo
cargo fmt --all
cargo test                                   # 305 passed; 0 failed
```

## O que esta correção **não** resolve

- A duplicata `pppoe-wan` já existente no banco de produção (correção 02).
- O casamento de interface por `snmp_index`, que é o que cria duplicatas em PPPoE
  (correção 03).
- O 500 por esgotamento do pool de uma conexão (correção 04).
