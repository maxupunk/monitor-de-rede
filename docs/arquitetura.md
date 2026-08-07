# Arquitetura Técnica Inicial

## Sistema de Monitoramento de Redes com AdonisJS

## 1. Objetivo

Este documento define a arquitetura técnica inicial da plataforma de monitoramento de redes residenciais e de pequenas empresas.

O backend será desenvolvido integralmente com AdonisJS, utilizando o framework para:

* API.
* Autenticação.
* Persistência.
* Filas.
* Workers.
* Agendamentos.
* Probe local.
* Probe remoto.
* Comunicação em tempo real.
* Comandos administrativos.
* Configuração e logs.

Os módulos responsáveis pelas operações de rede também serão escritos em TypeScript dentro do projeto, mas organizados de forma independente das camadas HTTP e de persistência.

## 2. Tecnologias

### Backend

* Node.js.
* TypeScript.
* AdonisJS.
* Lucid ORM.
* PostgreSQL.
* Sistema de filas do AdonisJS.
* Redis, quando necessário.
* SSE para comunicação com o frontend.
* WebSocket ou HTTPS para comunicação com probes.

### Frontend

* Vue 3.
* TypeScript.
* Vite.
* Vuetify.
* Pinia.
* Vue Router.
* PWA.

### Implantação

* Docker.
* Docker Compose.
* Linux como ambiente principal.
* Instalação standalone.
* Instalação centralizada com probes remotos.

## 3. Visão geral da arquitetura

O sistema utilizará o mesmo projeto AdonisJS para executar diferentes tipos de processo.

```text
Aplicação AdonisJS
├── API
├── Worker
├── Scheduler
├── Probe local
└── Comandos administrativos
```

Cada processo será iniciado separadamente.

```bash
node bin/server.js
node ace queue:work   # ⚠️ não implementado — ver §4.2
node ace scheduler:run
node ace probe:run
```

Todos os processos poderão utilizar:

* Configuração centralizada.
* Logger.
* Injeção de dependências.
* Validação.
* Criptografia.
* Eventos.
* Providers.
* Repositórios.
* Serviços compartilhados.
* Lifecycle da aplicação.

## 4. Processos principais

## 4.1 Servidor API

O servidor principal será responsável por:

* Autenticação.
* Usuários e permissões.
* Sites.
* Redes.
* Dispositivos.
* Interfaces.
* Monitores.
* Credenciais.
* Alertas.
* Eventos.
* Topologia.
* Recebimento de resultados dos probes.
* Atualizações em tempo real para o frontend.

Execução:

```bash
node bin/server.js
```

O servidor HTTP não deverá executar scans, ping, traceroute ou consultas SNMP diretamente.

Operações demoradas deverão ser encaminhadas para uma fila ou para um probe.

## 4.2 Worker

> 🔴 **Não implementado.** Esta seção descreve o desenho pretendido, não o que
> existe hoje. O comando `queue:work` chegou a ser criado no commit inicial, mas
> nunca passou de um esqueleto que registrava um log e encerrava — foi removido,
> junto com `bullmq`, `@adonisjs/redis` e o container `redis`, que estavam
> instalados e nunca foram importados.
>
> Na prática as responsabilidades abaixo foram absorvidas por outros processos:
> o **scheduler** executa os monitores inline (`executeMonitorAsync`) e drena a
> `DiscoveryQueue`, os **probes** executam checagens remotas, e o
> `ResultProcessor` cuida de métricas, alertas e notificações.
>
> A dívida que justifica retomar este desenho é o **backpressure** — ver a
> Fase 2 do [roadmap](roadmap.md).

O worker será responsável por executar jobs assíncronos.

Execução:

```bash
node ace queue:work
```

Tipos de jobs:

* Ping.
* HTTP e HTTPS.
* TCP.
* DNS.
* SNMP.
* Descoberta de dispositivos.
* Traceroute.
* Processamento de métricas.
* Avaliação de alertas.
* Envio de notificações.
* Limpeza de dados.
* Agregação de histórico.

O worker poderá executar checks quando estiver localizado na mesma rede dos dispositivos monitorados.

## 4.3 Scheduler

O scheduler será responsável por identificar tarefas que precisam ser executadas.

Execução:

```bash
node ace scheduler:run
```

Responsabilidades:

* Encontrar monitores vencidos.
* Criar jobs de verificação.
* Agendar scans periódicos.
* Agendar coletas SNMP.
* Executar limpeza de histórico.
* Executar agregação de métricas.
* Verificar probes desconectados.
* Atualizar estados de alertas.

O scheduler não deverá executar o monitoramento diretamente.

Fluxo:

```text
Scheduler
   ↓
Cria job
   ↓
Fila
   ↓
Worker ou probe
   ↓
Resultado
```

> A **fila precisa ser persistente**: quem enfileira é o `scheduler:run` e quem
> entrega é o processo HTTP, que responde ao `GET /api/probes/tasks`. Ela vive na
> tabela `probe_tasks` ([`probe_task_dispatcher.ts`](../modules/probes/probe_task_dispatcher.ts)) —
> pelo mesmo motivo que os eventos SSE vivem em `event_outbox`. Uma fila em
> memória funciona nos testes e nunca em produção: o probe consultaria uma fila
> sempre vazia e todo monitor atribuído a probe ficaria parado em `unknown`.

Um monitor tem no máximo **uma** tarefa pendente, e tarefa parada há mais de
`TASK_TTL_SECONDS` é descartada: probe que volta depois de um tempo fora executa
uma checagem atual por monitor, não uma avalanche de checagens vencidas.

Probe sem heartbeat há mais de `PROBE_OFFLINE_AFTER_SECONDS` é marcado `offline`
pelo `ProbeWatchdog` ([`probe_liveness.ts`](../modules/probes/probe_liveness.ts)), e
o scheduler passa a registrar as checagens dele como `unknown` com o motivo — em
vez de despachar em silêncio para um agente que não vai buscar nada.

## 4.4 Probe

O probe será um processo AdonisJS executado dentro da rede monitorada.

Execução:

```bash
node ace probe:run
```

Responsabilidades:

* Registrar-se no servidor.
* Manter conexão com o servidor.
* Receber tarefas.
* Executar verificações.
* Realizar scans.
* Consultar equipamentos SNMP.
* Executar traceroute.
* Enviar resultados.
* Manter fila local em caso de perda de conexão.
* Informar sua própria disponibilidade.

O probe poderá ser executado:

* No mesmo servidor.
* Em um computador da rede.
* Em um mini-PC.
* Em um Raspberry Pi.
* Em um container.
* Em um servidor de filial.

## 5. Modos de instalação

## 5.1 Standalone

Todos os serviços são executados no mesmo ambiente.

```text
Servidor
├── API
├── Scheduler
├── Worker
├── Probe
├── PostgreSQL
└── Redis
```

Indicado para:

* Residências.
* Pequenos escritórios.
* Uma única rede.
* Instalação simples.

## 5.2 Servidor central com probe local

```text
Servidor central
├── API
├── Scheduler
├── Worker
└── Probe local
```

O probe local monitora a rede onde o servidor está instalado.

## 5.3 Servidor central com probes remotos

```text
Servidor central
├── API
├── Scheduler
└── Worker
       │
       ├── Probe residência
       ├── Probe escritório
       └── Probe filial
```

Cada probe terá acesso apenas ao site e às redes para as quais foi autorizado.

## 6. Organização do projeto

```text
network-monitor/
├── app/
│   ├── controllers/
│   ├── middleware/
│   ├── models/
│   ├── validators/
│   ├── services/
│   ├── repositories/
│   ├── jobs/
│   ├── events/
│   ├── listeners/
│   ├── policies/
│   ├── exceptions/
│   └── probes/
│
├── commands/
│   ├── probe_run.ts
│   ├── scheduler_run.ts
│   ├── monitor_test.ts
│   └── network_scan.ts
│
├── modules/
│   ├── monitoring/
│   ├── discovery/
│   ├── snmp/
│   ├── topology/
│   ├── alerts/
│   ├── notifications/
│   └── probes/
│
├── config/
├── database/
│   ├── migrations/
│   ├── seeders/
│   └── factories/
│
├── providers/
├── start/
├── tests/
├── frontend/
└── docker/
```

## 7. Módulos principais

## 7.1 Monitoring

Responsável por executar verificações de disponibilidade.

```text
modules/monitoring/
├── contracts/
├── checkers/
│   ├── ping_checker.ts
│   ├── http_checker.ts
│   ├── tcp_checker.ts
│   └── dns_checker.ts
├── monitor_runner.ts
├── result_processor.ts
└── status_calculator.ts
```

Contrato base:

```ts
export interface MonitorChecker<TConfig, TResult> {
  execute(config: TConfig): Promise<TResult>
}
```

Cada tipo de monitor terá uma implementação própria.

```ts
export class PingChecker
  implements MonitorChecker<PingConfig, PingResult>
{
  async execute(config: PingConfig): Promise<PingResult> {
    // Executa ping e retorna resultado normalizado
  }
}
```

## 7.2 Discovery

Responsável pela descoberta de dispositivos.

```text
modules/discovery/
├── scanners/
│   ├── icmp_scanner.ts
│   ├── arp_scanner.ts
│   ├── mdns_scanner.ts
│   ├── ssdp_scanner.ts
│   └── port_scanner.ts
├── device_identifier.ts
├── discovery_merger.ts
└── discovery_service.ts
```

O módulo deverá consolidar resultados provenientes de diferentes métodos.

Exemplo:

```text
IP: 192.168.1.10
MAC: AA:BB:CC:DD:EE:FF
Hostname: notebook.local
mDNS: Notebook de João
Fabricante: Dell
```

Esses dados deverão formar um único resultado de descoberta.

## 7.3 SNMP

Responsável por inventário e métricas de equipamentos.

```text
modules/snmp/
├── clients/
├── profiles/
├── mibs/
├── collectors/
│   ├── system_collector.ts
│   ├── interface_collector.ts
│   ├── traffic_collector.ts
│   └── lldp_collector.ts
├── snmp_session_factory.ts
└── snmp_service.ts
```

O módulo deverá suportar:

* SNMPv1.
* SNMPv2c.
* SNMPv3.
* Get.
* GetNext.
* GetBulk.
* Walk.
* Contadores de 32 e 64 bits.

As consultas devem ser agrupadas por dispositivo.

```text
Uma coleta SNMP
├── Informações do sistema
├── Lista de interfaces
├── Estado das interfaces
├── Contadores de tráfego
└── Vizinhos LLDP
```

## 7.4 Topology

Responsável pelas relações entre dispositivos.

```text
modules/topology/
├── topology_service.ts
├── link_resolver.ts
├── route_resolver.ts
├── confidence_calculator.ts
└── topology_builder.ts
```

Tipos de ligação:

* Manual.
* LLDP.
* CDP.
* SNMP.
* Inferida.
* Traceroute.

Cada ligação deverá possuir:

* Origem.
* Destino.
* Interface de origem.
* Interface de destino.
* Método de descoberta.
* Nível de confiança.
* Data da descoberta.
* Data da última confirmação.

## 7.5 Alerts

Responsável por avaliar e controlar alertas.

```text
modules/alerts/
├── rule_evaluator.ts
├── alert_manager.ts
├── recovery_manager.ts
└── silence_manager.ts
```

Exemplos de regras:

* Dispositivo offline por mais de dois minutos.
* Latência maior que 200 ms.
* Interface com uso superior a 90%.
* Certificado expirando.
* Probe desconectado.
* Novo dispositivo descoberto.

## 7.6 Notifications

Responsável por enviar notificações.

```text
modules/notifications/
├── channels/
│   ├── email_channel.ts
│   ├── telegram_channel.ts
│   ├── discord_channel.ts
│   └── webhook_channel.ts
├── notification_service.ts
└── message_formatter.ts
```

Cada canal deverá implementar um contrato comum.

```ts
export interface NotificationChannel {
  send(message: NotificationMessage): Promise<void>
}
```

## 7.7 Probes

Responsável pelo gerenciamento e comunicação com probes.

```text
modules/probes/
├── probe_agent.ts
├── probe_connection.ts
├── probe_authenticator.ts
├── probe_task_dispatcher.ts
├── probe_result_receiver.ts
└── probe_buffer.ts
```

O módulo deverá tratar:

* Registro do probe.
* Autenticação.
* Heartbeat.
* Recebimento de tarefas.
* Cancelamento de tarefas.
* Retorno de resultados.
* Reconexão.
* Atualização de versão.
* Revogação.

## 8. Models principais

## 8.1 User

Representa um usuário do sistema.

Campos iniciais:

```text
id
name
email
password
active
created_at
updated_at
```

## 8.2 Site

Representa um local monitorado.

```text
id
name
description
location
active
created_at
updated_at
```

## 8.3 Network

Representa uma rede cadastrada.

```text
id
site_id
probe_id
name
cidr
gateway
vlan
dns_servers
scan_enabled
scan_interval
active
created_at
updated_at
```

## 8.4 Probe

Representa um agente de monitoramento.

```text
id
site_id
name
token_hash
status
version
last_seen_at
registered_at
revoked_at
configuration
```

## 8.5 Device

Representa um equipamento.

```text
id
site_id
network_id
name
type
vendor
model
serial_number
description
status
last_seen_at
created_at
updated_at
```

## 8.6 DeviceAddress

```text
id
device_id
address
family
hostname
is_primary
last_seen_at
```

## 8.7 DeviceMac

```text
id
device_id
address
vendor
interface_name
last_seen_at
```

## 8.8 DeviceInterface

```text
id
device_id
snmp_index
name
description
alias
mac_address
type
speed
admin_status
oper_status
last_seen_at
```

## 8.9 DeviceLink

```text
id
source_device_id
target_device_id
source_interface_id
target_interface_id
link_type
discovery_method
confidence
confirmed
last_seen_at
created_at
```

## 8.10 Monitor

```text
id
device_id
probe_id
type
name
configuration
interval_seconds
timeout_seconds
retry_count
enabled
next_run_at
last_run_at
status
created_at
updated_at
```

A configuração específica deverá ser armazenada inicialmente em JSON.

Exemplo de monitor ping:

```json
{
  "host": "192.168.1.1",
  "packetCount": 3,
  "packetSize": 56
}
```

Exemplo de monitor HTTP:

```json
{
  "url": "https://192.168.1.1",
  "method": "GET",
  "acceptedStatusCodes": [200, 301, 302],
  "validateCertificate": false
}
```

## 8.11 MonitorResult

```text
id
monitor_id
probe_id
status
started_at
finished_at
duration_ms
latency_ms
message
data
created_at
```

## 8.12 Metric

```text
id
device_id
interface_id
monitor_id
name
value
unit
recorded_at
```

## 8.13 DiscoveryRun

```text
id
network_id
probe_id
status
started_at
finished_at
configuration
error
```

## 8.14 DiscoveryResult

```text
id
discovery_run_id
ip_address
mac_address
hostname
mdns_name
vendor
device_type
confidence
status
data
first_seen_at
last_seen_at
```

## 8.15 AlertRule

```text
id
site_id
device_id
monitor_id
name
type
condition
severity
duration_seconds
enabled
created_at
updated_at
```

## 8.16 AlertEvent

```text
id
alert_rule_id
device_id
monitor_id
status
severity
started_at
resolved_at
message
data
```

## 9. Filas

Filas iniciais:

```text
monitoring
snmp
discovery
alerts
notifications
maintenance
```

### Monitoring

* Ping.
* HTTP.
* HTTPS.
* TCP.
* DNS.

### SNMP

* Consulta básica.
* Descoberta de interfaces.
* Coleta de tráfego.
* Descoberta LLDP.

### Discovery

* Scan de rede.
* mDNS.
* SSDP.
* Identificação de equipamentos.

### Alerts

* Avaliação de regras.
* Abertura de alertas.
* Recuperação de alertas.

### Notifications

* E-mail.
* Telegram.
* Discord.
* Webhook.

### Maintenance

* Limpeza.
* Agregação.
* Retenção.
* Verificação de integridade.

## 10. Jobs iniciais

```text
ExecuteMonitorJob
PollSnmpDeviceJob
ScanNetworkJob
DiscoverMdnsJob
DiscoverSsdpJob
RunTracerouteJob
ProcessMonitorResultJob
EvaluateAlertRulesJob
SendNotificationJob
AggregateMetricsJob
CleanupMetricsJob
CheckProbeStatusJob
```

Exemplo de fluxo de monitoramento:

```text
Scheduler
   ↓
ExecuteMonitorJob
   ↓
PingChecker
   ↓
MonitorResult
   ↓
ProcessMonitorResultJob
   ↓
Atualiza estado
   ↓
EvaluateAlertRulesJob
```

## 11. Execução de checks

Todos os resultados deverão utilizar uma estrutura normalizada.

```ts
export interface CheckResult {
  success: boolean
  status: 'up' | 'down' | 'warning' | 'unknown'
  startedAt: Date
  finishedAt: Date
  durationMs: number
  message?: string
  metrics?: CheckMetric[]
  data?: Record<string, unknown>
}
```

Exemplo de métrica:

```ts
export interface CheckMetric {
  name: string
  value: number
  unit: string
}
```

Exemplo de resultado de ping:

```json
{
  "success": true,
  "status": "up",
  "durationMs": 34,
  "metrics": [
    {
      "name": "latency",
      "value": 12.5,
      "unit": "ms"
    },
    {
      "name": "packet_loss",
      "value": 0,
      "unit": "percent"
    }
  ]
}
```

## 12. Agendamento de monitores

Cada monitor possuirá o campo:

```text
next_run_at
```

O scheduler buscará monitores vencidos.

```sql
SELECT *
FROM monitors
WHERE enabled = true
  AND next_run_at <= NOW()
ORDER BY next_run_at
LIMIT 100;
```

Para evitar execução duplicada, o sistema deverá aplicar bloqueio durante a seleção.

Após criar o job:

```text
next_run_at = next_run_at + intervalo
```

O próximo horário deverá ser calculado com base no horário originalmente previsto, reduzindo o deslocamento acumulado.

## 13. Comunicação com probes

## 13.1 Registro

O probe será instalado com um token temporário.

```bash
node ace probe:register --token=TOKEN
```

O probe enviará:

* Nome.
* Identificador local.
* Versão.
* Sistema operacional.
* Arquitetura.
* Endereços locais.
* Capacidades.

O servidor retornará uma credencial permanente.

## 13.2 Autenticação

Cada probe deverá possuir:

* Identificador próprio.
* Token exclusivo.
* Possibilidade de revogação.
* Permissões limitadas.
* Associação a um site.

As credenciais deverão ser armazenadas com segurança.

## 13.3 Heartbeat

O probe enviará periodicamente:

```json
{
  "probeId": "probe-01",
  "version": "1.0.0",
  "status": "online",
  "runningTasks": 3,
  "timestamp": "2026-08-01T15:00:00Z"
}
```

## 13.4 Recebimento de tarefas

Estrutura básica:

```json
{
  "id": "task-123",
  "type": "ping",
  "timeout": 10000,
  "payload": {
    "host": "192.168.1.1"
  }
}
```

## 13.5 Retorno de resultados

```json
{
  "taskId": "task-123",
  "success": true,
  "startedAt": "2026-08-01T15:00:00Z",
  "finishedAt": "2026-08-01T15:00:01Z",
  "result": {
    "status": "up",
    "latencyMs": 10
  }
}
```

## 14. Funcionamento offline do probe

Caso o probe perca conexão com o servidor:

* Os checks essenciais poderão continuar.
* Os resultados serão armazenados localmente.
* O probe tentará se reconectar.
* Os resultados pendentes serão enviados depois.
* Tarefas expiradas serão descartadas.
* O buffer deverá possuir limite de tamanho.

O armazenamento local poderá utilizar SQLite.

```text
Probe
├── Fila de tarefas
├── Resultados pendentes
├── Configuração
└── Credenciais
```

## 15. API inicial

## 15.1 Autenticação

```text
POST /api/auth/login
POST /api/auth/logout
GET  /api/auth/me
```

## 15.2 Sites

```text
GET    /api/sites
POST   /api/sites
GET    /api/sites/:id
PUT    /api/sites/:id
DELETE /api/sites/:id
```

## 15.3 Redes

```text
GET    /api/networks
POST   /api/networks
GET    /api/networks/:id
PUT    /api/networks/:id
DELETE /api/networks/:id
POST   /api/networks/:id/scan
```

## 15.4 Dispositivos

```text
GET    /api/devices
POST   /api/devices
GET    /api/devices/:id
PUT    /api/devices/:id
DELETE /api/devices/:id

GET    /api/devices/:id/interfaces
GET    /api/devices/:id/monitors
GET    /api/devices/:id/metrics
GET    /api/devices/:id/events
```

## 15.5 Monitores

```text
GET    /api/monitors
POST   /api/monitors
GET    /api/monitors/:id
PUT    /api/monitors/:id
DELETE /api/monitors/:id

POST   /api/monitors/:id/run
POST   /api/monitors/:id/enable
POST   /api/monitors/:id/disable
```

## 15.6 Descoberta

```text
GET  /api/discovery/runs
GET  /api/discovery/runs/:id
GET  /api/discovery/results
POST /api/discovery/results/:id/accept
POST /api/discovery/results/:id/ignore
POST /api/discovery/results/:id/merge
```

## 15.7 Topologia

```text
GET    /api/topology
POST   /api/topology/links
PUT    /api/topology/links/:id
DELETE /api/topology/links/:id
```

## 15.8 Probes

```text
GET    /api/probes
POST   /api/probes
GET    /api/probes/:id
PUT    /api/probes/:id
DELETE /api/probes/:id

POST /api/probes/:id/revoke
POST /api/probes/:id/test
```

## 15.9 Alertas

```text
GET    /api/alert-rules
POST   /api/alert-rules
PUT    /api/alert-rules/:id
DELETE /api/alert-rules/:id

GET  /api/alerts
POST /api/alerts/:id/acknowledge
POST /api/alerts/:id/silence
```

## 16. Tempo real

O frontend utilizará SSE para receber atualizações.

Canal principal:

```text
GET /api/events/stream
```

Eventos iniciais:

```text
device.status.changed
monitor.result.created
alert.opened
alert.resolved
probe.connected
probe.disconnected
discovery.device.found
scan.progress.updated
topology.updated
```

Exemplo:

```json
{
  "event": "device.status.changed",
  "data": {
    "deviceId": 10,
    "previousStatus": "online",
    "currentStatus": "offline"
  }
}
```

## 17. Frontend

O frontend será desenvolvido em Vue com Vuetify.

Estrutura:

```text
frontend/
├── src/
│   ├── components/
│   ├── layouts/
│   ├── pages/
│   ├── stores/
│   ├── services/
│   ├── composables/
│   ├── router/
│   ├── types/
│   └── plugins/
```

Stores iniciais:

```text
auth
sites
networks
devices
monitors
discovery
topology
alerts
probes
events
```

## 18. Segurança

O sistema deverá aplicar:

* Autenticação.
* Autorização por policy.
* Validação em todas as entradas.
* Rate limit.
* Proteção contra tentativas de login.
* Criptografia de credenciais.
* Hash de tokens.
* Registro de auditoria.
* Isolamento por site.
* Revogação de probes.
* Restrição de scans.
* Timeout de operações.
* Limite de concorrência.
* Proteção contra comandos arbitrários.

O probe não receberá comandos de shell livres.

As tarefas deverão ser baseadas em tipos previamente definidos.

Permitido:

```json
{
  "type": "ping",
  "payload": {
    "host": "192.168.1.1"
  }
}
```

Não permitido:

```json
{
  "type": "shell",
  "command": "qualquer comando"
}
```

## 19. Docker Compose inicial

> ℹ️ Esboço original. O arquivo real é o [`docker-compose.yml`](../docker-compose.yml)
> na raiz, que já divergiu deste: não há `worker` nem `redis` (ver §4.2), e
> foram acrescentados `migration`, `wireguard`, `vpn-probe` e `frontend`.

```yaml
services:
  server:
    build: .
    command: node bin/server.js
    depends_on:
      - postgres
      - redis

  worker:
    build: .
    command: node ace queue:work
    depends_on:
      - postgres
      - redis

  scheduler:
    build: .
    command: node ace scheduler:run
    depends_on:
      - postgres
      - redis

  probe:
    build: .
    command: node ace probe:run
    network_mode: host
    depends_on:
      - server

  postgres:
    image: postgres

  redis:
    image: redis
```

O modo de rede do probe dependerá do ambiente.

Para funcionalidades como mDNS, ARP e descoberta local, poderá ser necessário:

```yaml
network_mode: host
```

ou executar o probe diretamente no sistema operacional.

## 20. MVP técnico

A primeira versão deverá implementar:

### Backend

* Aplicação AdonisJS.
* PostgreSQL.
* Autenticação.
* Sites.
* Redes.
* Dispositivos.
* Monitores.
* Worker.
* Scheduler.
* Probe local.
* SSE.

### Monitoramento

* Ping.
* HTTP e HTTPS.
* TCP.
* DNS.

### Descoberta

* Scan IPv4.
* Ping.
* DNS reverso.
* mDNS.
* Identificação por MAC.

### Interface

* Dashboard.
* Sites.
* Redes.
* Dispositivos.
* Monitores.
* Descoberta.
* Topologia manual.
* Alertas básicos.

### Alertas

* Dispositivo offline.
* Serviço indisponível.
* Recuperação.
* Notificação por e-mail ou Telegram.

## 21. Segunda etapa

* Probes remotos.
* Buffer offline.
* SNMP.
* Interfaces.
* Tráfego.
* LLDP.
* Traceroute.
* Topologia automática.
* Múltiplos usuários.
* Permissões.
* Retenção configurável.
* Agregação de métricas.

## 22. Decisões técnicas principais

### Todo o backend utilizará AdonisJS

Isso inclui:

* API.
* Worker.
* Scheduler.
* Probe.
* Comandos.
* Configuração.
* Logger.
* Injeção de dependências.
* Lifecycle.

### Processos serão separados

Mesmo usando o mesmo framework, API, worker, scheduler e probe não deverão executar no mesmo processo.

### Módulos de rede serão isolados

Ping, SNMP, discovery e traceroute não deverão ficar dentro de controllers ou models.

### Probe remoto não acessará o banco central

A comunicação acontecerá por API autenticada ou conexão persistente.

### PostgreSQL será o banco principal

SQLite poderá ser utilizado somente no buffer local do probe.

### SSE será utilizado no frontend

WebSocket será reservado principalmente para comunicação persistente com probes, caso necessário.

## 23. Resumo final

```text
Frontend
Vue 3 + Vuetify + PWA

API
AdonisJS

Worker
AdonisJS Queue

Scheduler
Comando ou serviço AdonisJS

Probe
Comando AdonisJS de longa duração

Banco central
PostgreSQL

Buffer do probe
SQLite

Fila
Redis

Tempo real do frontend
SSE

Comunicação com probe
HTTPS ou WebSocket autenticado
```

A arquitetura utilizará AdonisJS como base comum para todos os processos, reduzindo duplicação de configuração, logs, lifecycle e injeção de dependências.

A separação ocorrerá por responsabilidade e por processo, não pela remoção do framework.

```text
Mesmo código-base
├── Processo API
├── Processo worker
├── Processo scheduler
└── Processo probe
```

Essa abordagem permite iniciar o projeto de forma simples e evoluir para múltiplas redes e probes remotos sem precisar reconstruir toda a arquitetura.
