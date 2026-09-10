# Correção 06 — Cadeia de identidade de interface

- **Data:** 2026-09-09
- **Severidade:** alta (robustez do casamento que sustenta todo o histórico por porta)
- **Arquivos:** `backend/src/services/snmp/service.rs`
- **Substitui:** a solução da [correção 03](2026-09-09-03-casamento-de-interface-por-nome.md)

## Por que revisitar a 03

A correção 03 inverteu a ordem para nome-antes-de-índice e resolveu o caso PPPoE. Mas
resolveu **por sorte do parque**, não por robustez: funciona porque neste equipamento o
`ifName` é único. Três lacunas ficaram:

1. **Nome e índice mudando juntos** (`ppp0` → `ppp1`): nenhum sinal casa.
2. **Unicidade verificada só do lado do banco.** Com duas `lan` chegando na varredura e
   uma no banco, quem ficava com a linha dependia da ordem de iteração.
3. **Resolução interface a interface.** Um casamento fraco consumia a linha que um
   casamento forte reivindicaria depois.

## O que a categoria faz

| Ferramenta | Estratégia |
|---|---|
| **LibreNMS** | `port_association_mode` por dispositivo: `ifIndex` (padrão), `ifName`, `ifDescr`, `ifAlias`; porta sumida vira `deleted = 1` e é reativada se reaparecer |
| **PRTG** | Guarda `ifIndex:ifAlias`; lê `sysUpTime` a cada scan e, ao detectar reset, revarre e **reescreve o índice** pelo `ifAlias` casado unicamente |
| **Cacti** | Re-index disparado por *uptime goes backwards* ou *index count changed*, remapeando por um campo único escolhido |
| **Zabbix** | Chaveia o item por `{#SNMPINDEX}` — **perde o histórico** quando o índice muda: o item velho vira "lost" e o novo nasce vazio |

E a RFC 2863 explica a raiz: o `ifIndex` só é garantido constante *entre
re-inicializações*, não através de reboots. O campo desenhado como identificador
não-volátil é o **`ifAlias`** — o agente é obrigado a preservá-lo através de reboots.

## O que os dados de produção mostraram

Medição sobre os 55 registros de `device_interfaces` do backup:

| Sinal | Preenchido | Únicos dentro do dispositivo |
|---|---|---|
| `ifAlias` | **0/55 (0%)** | — |
| `ifPhysAddress` | 42/55 (76%) | **18/42** |
| `ifName` | 55/55 (100%) | 53/55 |
| `ifDescr` | 55/55 (100%) | 53/55 |

Três consequências de projeto:

- **A solução do PRTG sozinha não serviria:** o OpenWrt não preenche `ifAlias`.
- **MAC é discriminador fraco aqui:** `d6:8c:72:59:65:fc` se repete em `sfp2`, `br-lan`,
  `br-outros` e `lan1`–`lan4`, porque bridges e VLANs herdam o MAC da porta física.
- **`ifName` é o melhor sinal deste parque** — e os 2 valores não-únicos eram exatamente
  o par `pppoe-wan` que a correção 02 funde.

## O que foi feito

`InterfaceMatcher` deixou de ser "um campo com plano B" e virou uma **cadeia resolvida em
duas passadas**.

### A cadeia

`ifAlias` → `ifName` → `ifDescr` → `ifPhysAddress` → `ifIndex`.

A ordem segue LibreNMS e PRTG, com o MAC inserido como desempate tardio pelo que a
medição mostrou. `ifIndex` fica por último, como recurso de quem não tem mais nada.

A diferença para o modo configurável do LibreNMS é que a degradação é **por interface**,
não por equipamento: num parque misto, o Mikrotik com `ifAlias` preenchido e o OpenWrt sem
ele são atendidos pela mesma regra, sem ninguém precisar escolher o modo certo para cada
um.

### Unicidade exigida dos dois lados

Um sinal só identifica quando aponta para **uma** linha e vem de **uma** interface.
Ambíguo de qualquer um dos lados, é descartado e a decisão desce para o próximo elo. Isso
torna determinístico o que antes dependia da ordem de iteração.

### Duas passadas

A resolução acontece de uma vez, sinal por sinal sobre o conjunto inteiro: **todos** os
`ifAlias` casam antes de qualquer `ifName` ser considerado. O teste
`a_ancora_forte_e_resolvida_antes_da_fraca` fixa o cenário que só isso resolve — uma
`eth0` que, resolvida interface a interface, roubaria pelo nome a linha que o `ifAlias` da
`wan` renomeada reivindica depois.

### Sinais que não são sinais

- Texto vazio ou só espaço é descartado.
- **MAC zerado** (`00:00:00:00:00:00`) é descartado: é o que o agente devolve para
  interface sem endereço — loopback, túnel, PPP. Tratá-lo como sinal juntaria todas numa
  linha só.

## Testes

Onze testes unitários (os 5 da correção 03 reescritos para a nova API, mais 6):

| Teste | O que trava |
|---|---|
| `interface_renumerada_casa_pelo_nome` | o caso PPPoE: idx 34 → 55 |
| `indice_reaproveitado_nao_sequestra_a_linha_de_outra_interface` | `br-lan@34` casa com a `br-lan` |
| `alias_vence_o_nome_quando_a_porta_foi_renomeada` | `ifAlias` acima do nome |
| `a_ancora_forte_e_resolvida_antes_da_fraca` | a ordem das passadas |
| `nome_repetido_no_aparelho_cai_para_o_indice` | ambiguidade no banco |
| `nome_repetido_na_varredura_tambem_descarta_o_sinal` | ambiguidade na varredura |
| `mac_compartilhado_entre_bridges_nao_identifica` | MAC de bridge não decide |
| `mac_zerado_nao_e_sinal` | loopback e túnel não colapsam |
| `uma_linha_so_e_reivindicada_uma_vez` | duas portas não viram uma |
| `interface_desconhecida_nao_casa` | porta nova vira registro novo |

Uma nota sobre o processo: o teste `a_ancora_forte_e_resolvida_antes_da_fraca` falhou na
primeira execução com a expectativa errada minha, não do código — eu havia montado o
cenário com a linha de fallback num índice que não batia. O cenário foi refeito para
exercitar de fato a ordem das passadas.

## Validação

```
cargo clippy --all-targets -- -D warnings   # limpo
cargo fmt --all
cargo test    # 947 unitários + 307 de requisição, 0 falhas
```

## O que ainda fica de fora

O vínculo entre monitor e interface continua sendo a **string do nome**
(`Interface {name}`), não o `interface_id`. É daí que vêm o `is_monitored` errado da
órfã e o monitor compartilhado entre homônimas. É a correção 07.

## Fontes

- [LibreNMS — Devices API / port_association_mode](https://docs.librenms.org/API/Devices/)
- [LibreNMS — includes/discovery/ports.inc.php](https://github.com/librenms/librenms/blob/master/includes/discovery/ports.inc.php)
- [Paessler — Automatically update port name and number for SNMP Traffic sensors](https://helpdesk.paessler.com/en/support/solutions/articles/76000063099-automatically-update-port-name-and-number-for-snmp-traffic-sensors-when-the-device-changes-them)
- [Cacti — Re-Indexing Difference Between Methods](https://forums.cacti.net/viewtopic.php?t=50933)
- [Zabbix — Discovery of SNMP OIDs](https://www.zabbix.com/documentation/current/en/manual/discovery/low_level_discovery/examples/snmp_oids_walk)
- [RFC 2863 — The Interfaces Group MIB](https://www.rfc-editor.org/rfc/rfc2863.html)
- [Cisco — Interface Index (ifIndex) Persistence](https://www.cisco.com/c/en/us/support/docs/ip/simple-network-management-protocol-snmp/28420-ifIndex-Persistence.html)
