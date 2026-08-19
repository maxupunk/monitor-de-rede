# Runbook de diagnóstico

Como responder, em produção, às perguntas que o NetMonitor faz sobre si mesmo.
O servidor é um dispositivo como qualquer outro — então quase tudo aqui é o
mesmo procedimento que você usaria para um roteador, com o Servidor NetMonitor
selecionado no filtro.

## 1. Onde está o Servidor NetMonitor

Ele aparece em `/devices` como um dispositivo comum. **Não procure por ID fixo
nem pelo nome**: o nome é editável e o ID varia por instalação. A identidade
técnica é a coluna `devices.system_key = 'netmonitor'`.

```sql
SELECT id, name, status, last_seen_at FROM devices WHERE system_key = 'netmonitor';
```

Deve haver exatamente **uma** linha. Duas seriam um índice único ausente
(`devices_system_key_unique`); zero significa que o `Initializer` não rodou —
verifique o log de boot por `não foi possível garantir o dispositivo do sistema`.

## 2. A saúde não aparece na Visão Geral

Os cards de saúde vêm das **séries gravadas**, não de configuração. Na ordem:

1. **O monitor existe e está executando?** Aba Monitores do dispositivo: deve
   haver um monitor do tipo `system_health`, ativo, com `last_run_at` recente.
   Ele é gerenciado — não aceita troca de tipo, alvo, probe nem desativação.
2. **Ele produziu métricas?**

   ```sql
   SELECT name, value, unit, recorded_at FROM metrics
    WHERE device_id = <id> ORDER BY recorded_at DESC LIMIT 20;
   ```

3. **A coleta declarou indisponibilidade?** O resultado da última checagem
   carrega o motivo. Na aba Monitores, abra o histórico do monitor; o campo
   `data.unavailable` lista cada série que este sistema não consegue medir e
   por quê.

**Casos normais de indisponibilidade:**

| Sintoma | Causa | O que fazer |
|---|---|---|
| `cpu_usage` e `inBps` ausentes logo após o boot | São **deltas** de contador acumulado: a primeira amostra do processo estabelece a linha de base | Nada. A segunda coleta já mede. |
| Nenhuma métrica, com motivo `/proc ... indisponível` | O processo não roda em Linux | Esperado fora do container; o alvo de produção é Linux. |
| `storage_usage` ausente | `statvfs` indisponível neste sistema | Esperado no Windows de desenvolvimento. |
| `memory_usage` com origem `cgroup` e valor bem maior que o do host | O container tem limite de memória — e é ele que decide o OOM | Correto. É a pergunta que o operador está fazendo. |

A origem de cada valor (`host`, `cgroup`, `process`) vai em `data.sources` do
resultado.

## 3. Os logs internos não aparecem

A aba Logs do dispositivo e a tela `/logs` são **a mesma** consulta, com o
filtro fixado. Se o log interno não aparece:

1. **O banco de logs abriu?** Procure `banco de logs pronto` no boot. Ele **não**
   depende de `SYSLOG_ENABLED`: esse flag governa apenas o listener de rede.
   Com `SYSLOG_ENABLED=false`, o log interno continua gravando e só a escuta
   syslog some.
2. **A fila foi publicada?** O pipeline monta em `Hooks::after_context`. Sem
   ele, os eventos vão só para o stdout.
3. **A linha veio sem dispositivo?** Log emitido **antes** de o dispositivo
   existir (boot, migrations) vai com `device_id` nulo e aparece em `/logs` sem
   filtro de dispositivo. É comportamento explícito, não perda.

   ```sql
   SELECT count(*) FROM device_logs WHERE source = 'application' AND device_id IS NULL;
   ```

4. **O alvo está silenciado?** A política em `syslog/app_layer.rs` ignora o
   próprio escritor (para não realimentar `log → INSERT → log`) e as consultas
   SQL bem-sucedidas. `WARN` e `ERROR` do SQLx **continuam** passando — é o que
   você procura quando o banco trava.

## 4. Retenção: a disputa pelo orçamento de log

`retention::prune` corta o banco de logs por idade **e por tamanho** (4 GB, mais
antigo primeiro). Desde que o log da aplicação grava em `device_logs`, ele
**disputa esse orçamento** com o syslog do parque:

- um nível `DEBUG` ligado no `config.logger` empurra log de roteador para fora;
- um parque ruidoso empurra log da aplicação para fora.

**Esta é uma decisão deliberada**: cota por origem custaria mais complexidade do
que resolve. Para medir a proporção antes de mexer em qualquer coisa:

```sql
SELECT source, count(*), min(received_at), max(received_at)
  FROM device_logs GROUP BY source;
```

Se o log da aplicação estiver dominando, o ajuste é o nível em
`config/production.yaml` (`logger.level`), não uma cota nova.

## 5. Regras de saúde

As regras de CPU, memória e armazenamento são **de dispositivo**, não do
servidor: valem para qualquer equipamento que publique os campos — o NetMonitor
pela coleta local, um roteador pelo SNMP.

- O catálogo por dispositivo (`/devices/{id}?tab=rules`) só oferece o que
  aquele equipamento sabe medir. Um template marcado "indisponível neste
  equipamento" significa que ele não publica o `condition.field` da regra.
- O mesmo template aplicado a dois dispositivos cria **duas** regras, uma por
  escopo. Elas aparecem na Central de Alertas com a coluna Escopo preenchida.
- As regras do servidor são aplicadas **uma vez na vida da instalação**, com um
  marcador em `system_settings` (`alerts.health_defaults_applied.device.<id>`).
  Regra apagada de propósito **não** ressuscita no boot seguinte. Para
  reaplicá-las, remova o marcador:

  ```sql
  DELETE FROM system_settings WHERE key LIKE 'alerts.health_defaults_applied.device.%';
  ```

## 6. Backup e restauração

O Servidor NetMonitor viaja no arquivo de backup como qualquer outro
dispositivo. Ao restaurar:

1. o `wipe` + recarga recoloca as linhas **com os IDs do arquivo**;
2. o cache do resolvedor é invalidado e o serviço de identidade roda de novo;
3. a coleta de saúde é reprovisionada.

Se o arquivo for **anterior** a esta feature, o dispositivo simplesmente não
existe nele — e o serviço o recria após a restauração, com um ID novo. Os logs
internos anteriores ficam apontando para o ID antigo; isso é visível como linhas
sem dispositivo correspondente e sai pela retenção normal.

**Verificação após restaurar:**

```sql
-- Uma única linha, e o log interno seguinte aponta para ela.
SELECT id FROM devices WHERE system_key = 'netmonitor';
SELECT device_id, count(*) FROM device_logs
 WHERE source = 'application' AND received_at > datetime('now', '-5 minutes')
 GROUP BY device_id;
```

## 7. O que este sistema **não** consegue diagnosticar

- **A queda total do próprio processo.** Um processo parado não alerta sobre si;
  isso exige um observador externo, que está fora de escopo.
- **A ausência de coleta.** O motor de alertas é orientado a evento: a avaliação
  nasce quando um resultado chega. Uma regra sobre a *ausência* de resultado
  nunca dispararia. O coletor que falha grava `down`/`unknown` e é coberto por
  uma regra sobre `status`.
