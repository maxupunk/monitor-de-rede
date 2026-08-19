# Roadmap — Servidor NetMonitor como dispositivo

> **Objetivo**: representar o próprio NetMonitor como um dispositivo de
> primeira classe, usando os mesmos fluxos de métricas, monitores, regras,
> alertas e logs dos demais equipamentos. A interface deve mostrar apenas o
> que existe para o dispositivo selecionado.
>
> **Regra que decide todo impasse deste roadmap**: quando um recurso precisar
> existir para o servidor, ele nasce **para dispositivo** — nome, campo, tabela
> e tela genéricos — e o servidor é só o primeiro a usá-lo. Nenhum item aqui
> pode produzir um segundo vocabulário, um segundo pipeline ou uma segunda tela
> para responder a uma pergunta que o produto já responde.
>
> **Estado**: **implementado**. As sete fases receberam `[x]` e o badge
> `🟢 Concluído` depois da implementação e de todos os testes da fase passarem;
> a matriz obrigatória da seção 5 foi executada inteira e está registrada na
> Fase 7. Cada fase traz um "Registro de execução" com o que foi entregue e os
> achados corrigidos no caminho.

## 1. Decisões de produto

- “Servidor NetMonitor” aparece na lista, nos seletores e em
  `/devices/{id}` como um único dispositivo. O ID nunca é fixo.
- A identidade técnica do dispositivo é estável e não depende do nome exibido.
- Não existe aba **Saúde do Servidor**. Saúde atual, capacidade e gráficos
  essenciais ficam em **Visão Geral**.
- A aba **Métricas & Tráfego** deixa de existir como agrupamento genérico:
  métricas de saúde ficam na visão geral; histórico de uma checagem fica junto
  do monitor; tráfego fica junto da interface que o produz.
- **Interfaces SNMP** só aparece depois de uma comunicação SNMP bem-sucedida.
  Apenas marcar “SNMP habilitado” não cria uma aba vazia.
- **Regras** é uma aba contextual de todo dispositivo. Ela usa o mesmo catálogo,
  o mesmo formulário e os mesmos registros de **Regras Configuradas** em
  `/alerts`; não é um segundo gerenciador de regras.
- Logs do Servidor NetMonitor são ativados por padrão e gravados como logs desse
  dispositivo. `/logs` não possui aba “Servidor”: o filtro de dispositivo é a
  única forma de alternar a origem.
- Funcionalidade indisponível não gera aba vazia. A ação para habilitá-la fica
  na **Visão Geral**, acompanhada de uma explicação curta.
- **Alerta de CPU, memória e armazenamento nasce para todo dispositivo.** Hoje
  o produto não tem nenhum: `fields.rs` não publica campo de saúde e o monitor
  SNMP `cpu_usage` guarda a leitura fora do vocabulário avaliável. Os campos
  criados na Fase 3 valem igualmente para o servidor e para o roteador.
- **Nada do que este roadmap entrega pode ser exclusivo do servidor.** O que
  parecer exclusivo é sinal de que o item foi desenhado no lugar errado.

## 2. Fluxo final de navegação

### `/devices/{id}`

| Área | Quando aparece | Conteúdo |
|---|---|---|
| Visão Geral | sempre | identidade, estado, disponibilidade, resumo de saúde, métricas principais e ações de configuração |
| Monitores | sempre | checagens, estado, execução manual e acesso ao histórico de cada monitor |
| Regras | sempre | regras do dispositivo, recomendações compatíveis e criação personalizada |
| Interfaces SNMP | SNMP conectado | inventário de interfaces, estado, velocidade e tráfego por interface |
| Eventos | quando houver histórico | mudanças de estado e alertas relacionados ao dispositivo |
| Logs | log ativo, suportado ou já recebido | a mesma consulta e a mesma tabela usadas em `/logs`, filtradas pelo dispositivo |
| VPN | dispositivo associado a peer | dados e ações do túnel existente |

Regras de layout:

- No Servidor NetMonitor, a Visão Geral mostra CPU, memória, armazenamento,
  carga, processo e rede, com estado de coleta e horário da última amostra.
- Em dispositivos comuns, a Visão Geral mantém somente resumos aplicáveis.
- Gráficos detalhados são abertos a partir do card, monitor ou interface que os
  originou; não existe uma aba depósito para todas as séries.
- Se a aba indicada na URL deixar de ser aplicável, a página volta para
  `overview` sem erro e sem conteúdo vazio.
- **As capacidades governam também o cabeçalho.** Hoje o header oferece “Novo
  monitor”, “Configurar” (varredura SNMP), “Coletar” (leitura SNMP), “Portas” e
  “Editar” para qualquer dispositivo. No Servidor NetMonitor, escanear as
  próprias portas ou editar IP e comunidade SNMP de um dispositivo protegido
  não são ações válidas: a mesma projeção que decide as abas decide os botões.

### `/alerts`

- **Regras Pré-configuradas** começa pela escolha do dispositivo e mostra apenas
  templates compatíveis com suas capacidades.
- **Regras Configuradas** informa claramente o dispositivo e o monitor do
  escopo, com atalho para `/devices/{id}?tab=rules`.
- Abrir o catálogo pela página do dispositivo já fixa esse dispositivo como
  escopo; abrir por `/alerts` permite escolhê-lo.
- Criar ou editar uma regra usa um único componente compartilhado nas duas
  páginas.

### `/logs`

- Uma única listagem, um único live tail e um único conjunto de filtros.
- “Servidor NetMonitor” aparece no seletor de dispositivos como qualquer outro.
- Origens syslog ainda não reconhecidas continuam sem `device_id` e são tratadas
  no fluxo de vinculação de origens; isso não cria outra categoria visual.
- A aba Logs do dispositivo **já** usa a mesma store e a mesma tabela de
  `/logs`, com o filtro fixado. Isso é ponto de partida verificado, não trabalho
  a fazer — ver Fase 6.

## 3. Arquitetura alvo

```text
coletor de saúde local
        │
        ▼
monitor gerenciado do Servidor NetMonitor  (tipo `system_health`)
        │
        ▼
  process_result ──▶ monitor_results          (série da checagem)
        │        └─▶ metrics                  (série do dispositivo)
        │        └─▶ motor existente de alert_rules
        │                    │
        │                    ▼
        │                alert_events
        │
tracing da aplicação ──▶ LogQueue existente ──▶ writer em lote existente
                                                       │
                                                  device_logs
                                                       │
                                       API/SSE de logs existente
                                                       │
                                       /logs e aba Logs do dispositivo
```

Princípios obrigatórios:

- Controller apenas valida, delega e serializa.
- Coletores implementam contratos pequenos e são independentes de persistência.
- O orquestrador de saúde depende de traits de coleta, relógio e repositório,
  permitindo teste determinístico.
- O pipeline existente de monitoramento, métricas, alertas e logs é estendido;
  não são criados controllers, stores ou tabelas `runtime_*` paralelos.
- **Nenhuma peça de infraestrutura é construída duas vezes.** A fila limitada
  com descarte contado (`syslog/queue.rs`), a escrita em lote com gatilho de
  500 linhas / 200 ms (`syslog/writer.rs`) e o barramento do live tail
  (`syslog/bus.rs`) já existem e são os que a Fase 4 usa. Um segundo pipeline de
  log dentro do mesmo processo é violação deste roadmap, não detalhe de
  implementação.
- Toda consulta e migration funciona em SQLite e PostgreSQL. Entidades SeaORM
  continuam sendo geradas contra PostgreSQL.
- A coleta aceita Linux em container/cgroup v1 ou v2 e informa
  indisponibilidade por métrica, sem inventar zero.
- Escrita de log nunca ocorre dentro do callback de `tracing`: a fila limitada
  desacopla o request do banco e evita recursão.

### 3.1 As duas séries, e por que não são a mesma

Confundir as duas é o caminho mais curto para multiplicar o volume do banco
sem ganhar informação. A fronteira:

| tabela | o que guarda | retenção | quem já escreve |
|---|---|---|---|
| `monitor_results` | o desfecho de **uma checagem** — status, duração, latência, mensagem | 14 dias | `process_result`, para todo monitor |
| `metrics` | a grandeza contínua **do dispositivo ou da interface** — CPU, memória, tráfego | 30 dias | `snmp/service.rs` e `vpn/traffic_recorder.rs` |

Latência e perda de pacote **ficam onde estão**: `monitor_results.latency_ms`
tem índice próprio e alimenta o sparkline. Copiá-las para `metrics` a cada 15 s
não acrescenta nada e multiplica a tabela de maior volume do sistema.

### 3.2 Os dois vocabulários, e como ficam alinhados

| camada | convenção | valores |
|---|---|---|
| `metrics.name` (série persistida) | a que já existe, **inalterada** | `cpu_usage`, `memory_usage`, `snmp_uptime`, `inBps`, `outBps` |
| `condition.field` (regra de alerta) | camelCase do vocabulário | `latencyMs`, `packetLoss`, `ifOperStatus`, … |

A Fase 3 acrescenta em cada camada, sem renomear nada do que já existe:

- em `metrics.name`: `storage_usage`, `load_average_1m`, `process_memory_bytes`,
  `uptime_seconds` — mesma família dos que o SNMP já grava, então os widgets de
  CPU e memória e o endpoint `/devices/{id}/metrics` aceitam o servidor sem uma
  linha de frontend novo;
- em `condition.field`: `cpuUsagePercent`, `memoryUsedPercent`,
  `storageUsedPercent`, `loadAverage1m` — **nomes de dispositivo, não de
  servidor**, publicados tanto pelo coletor local quanto pelo dataset do SNMP.

O `METRIC_FIELD_MAP` de `alerts/datasets/monitor_result.rs` é o único ponto de
tradução entre as duas camadas e ganha as entradas correspondentes. O SNMP hoje
publica `usagePercent`/`usedPercent` soltos no `data`, fora do vocabulário — não
há regra possível sobre eles, então trocá-los pelas chaves acima não quebra
nada e passa a valer para o parque inteiro.

## 4. Fases de implementação

### Fase 0 — Limpeza da tentativa anterior `🟢 Concluído`

- [x] Descartar integralmente as 105 alterações rastreadas que estavam no
  `Staged`, restaurando o código ao `HEAD`.
- [x] Preservar alterações não rastreadas fora desse pacote (`.claude/`).
- [x] Confirmar que não restaram rotas, stores, componentes ou módulos
  `runtime_*` no código-base.
- [x] Definir este roadmap como a única especificação da nova implementação.

Antes de iniciar a Fase 1, o banco local usado para testar a tentativa
descartada deve ser recriado. Como aquele código não foi entregue, não haverá
migration de compatibilidade para tabelas ou colunas experimentais.

### Fase 1 — Identidade do dispositivo do sistema `🟢 Concluído`

- [x] Adicionar uma chave de sistema anulável e única em `devices`, usada para
  localizar `netmonitor` sem depender de ID, nome, IP, site ou rede. Coluna
  anulável mais índice único criado à parte — `ALTER TABLE ADD COLUMN UNIQUE`
  não existe no SQLite, e `NULL`s são distintos nos dois bancos, como o
  `devices_network_ip_unique` já explora.
- [x] Criar um serviço idempotente que garante exatamente um Servidor
  NetMonitor, inclusive sob boots concorrentes.
- [x] **Executar o serviço num `Initializer`, não em `after_context`.** As
  migrations do banco principal só convergem depois do `create_context`; um
  serviço que rodasse ali consultaria uma coluna que ainda não existe.
- [x] Expor um resolvedor com cache em memória (`current() -> Option<i64>`) para
  os caminhos quentes, alimentado pelo próprio serviço. **Ninguém consulta a
  chave por linha de log**: `device_logs` mora em outro banco e não tem FK para
  `devices`.
- [x] **Reexecutar o serviço e invalidar o cache ao fim de uma restauração de
  backup.** `backup::restore` faz `wipe` e recarrega as linhas **com os IDs do
  arquivo**: sem essa reexecução, o ID cacheado passa a apontar para outro
  equipamento e os logs internos são atribuídos a um roteador qualquer. Se o
  arquivo for anterior a esta feature, o dispositivo simplesmente não existe até
  o próximo boot.
- [x] Proteger exclusão e mudança dos campos que quebrariam a identidade; as
  demais leituras continuam nos endpoints normais de dispositivos. **A proteção
  vive no serviço/controller, nunca em `ActiveModelBehavior`** — um gatilho de
  entidade quebraria o `wipe()` da restauração e o `truncate` da suíte de testes.
- [x] A proteção é regra de negócio, **não** um perfil de acesso: ninguém apaga
  o dispositivo do sistema, nem `admin`. A política de acesso do produto tem
  duas linhas (`viewer` só lê; `operator` e `admin` escrevem; `admin` também
  administra usuários) e não ganha uma terceira categoria por causa deste
  roadmap.
- [x] Não criar rede, site ou probe fictício: esses vínculos permanecem nulos
  quando não representam algo real.
- [x] Cobrir criação, segundo boot, concorrência, proteção **e reexecução após
  restore** por testes em SQLite e PostgreSQL.

**Aceite**: a instalação sempre encontra o mesmo dispositivo pela chave
`netmonitor`, sem assumir `/devices/4`, sem duplicá-lo após reinício e sem
passar a apontar para outro equipamento depois de uma restauração.

#### Registro de execução

Entregue em `migration/src/m20260819_000001_devices_system_key.rs`,
`services/devices/system_device.rs`, `initializers/system_device.rs` e nas
proteções de `controllers/devices.rs`. Testes em
`tests/requests/system_device.rs`; suíte completa em 187 testes verdes, com
`cargo fmt --all --check` e `cargo clippy --all-targets -- -D warnings` limpos.

Achados corrigidos durante a execução:

1. **`devices::Model` é construído à mão em três testes de unidade**
   (`devices/access.rs`, `topology/service.rs`, `vpn/peer_service.rs`). A coluna
   nova quebrou os três com `E0063`. Corrigido com `system_key: None` — é o
   valor correto, não um remendo: nenhum deles fala do dispositivo do sistema.
2. **Os testes de backup contavam dispositivos sem qualificar quais.**
   `exportar_e_restaurar_...` e `arquivo_de_versao_desconhecida_...` assumiam
   que a instalação começa sem dispositivo nenhum, o que deixou de ser verdade.
   Corrigido com um filtro explícito no teste, e não escondendo o servidor do
   backup: ele **deve** viajar no arquivo como qualquer outro dispositivo.
3. **A tela precisava de uma resposta do backend sobre "isto é o servidor?".**
   Sem isso, a única saída do frontend seria deduzir por nome ou por ID — o que
   a seção 6 proíbe. `GET /api/devices` passa a devolver `systemKey` e
   `isSystem`, e é sobre esse campo que a Fase 5 monta as capacidades.

### Fase 2 — Saúde pelo pipeline normal de monitoramento `🟢 Concluído`

- [x] Criar o tipo interno de monitor `system_health`, permitido somente para o
  dispositivo `netmonitor` e provisionado de forma idempotente, com índice único
  por `(device_id, type)` para o caso de boots concorrentes.
- [x] Registrar o tipo em `monitoring::runner::run_monitor`. Sem esse ramo o
  agendador devolve “tipo de monitor desconhecido” a cada ciclo.
- [x] **Impedir `probe_id` no monitor gerenciado.** `execute_one` despacha para
  o probe remoto quando o campo está preenchido — e aí a coleta mediria a saúde
  do probe, não a do servidor.
- [x] Provisionar o monitor com `retry_count = 0`. `run_local_confirming_failure`
  repete a checagem até quatro vezes num `down`; reler `/proc` quatro vezes não
  confirma coisa alguma.
- [x] Coletar CPU, carga, memória disponível/usada, armazenamento disponível/
  usado, memória do processo, uptime e tráfego agregado com unidades explícitas.
- [x] Separar coletores de host, processo, cgroup e armazenamento atrás de
  traits; o coordenador apenas escolhe a melhor fonte e normaliza o resultado.
- [x] **Criar em `process_result` o caminho genérico de gravação de série de
  dispositivo**: as medidas do `CheckResult` cujo nome está na lista de séries
  de dispositivo (§3.1) são persistidas em `metrics` quando o monitor tem
  `device_id`. Uma passagem, válida para qualquer checker — não um gravador do
  servidor. Latência e perda continuam apenas em `monitor_results`.
- [x] Publicar cada ciclo como `monitor_results` e `metrics` por esse caminho,
  reutilizando retenção, SSE, histórico, estado do dispositivo e avaliação de
  alertas.
- [x] Marcar métricas não suportadas como indisponíveis, registrando a origem
  (`host`, `cgroup` ou `process`) usada em cada valor.
- [x] Permitir “Executar agora” e ajuste de intervalo, mas impedir troca de tipo,
  alvo, probe, desativação ou exclusão do monitor gerenciado.
- [x] Testar parsers com fixtures de `/proc`, cgroup v1/v2 e casos parciais;
  nenhum teste de rede usa alvo externo.

**Aceite**: a saúde do servidor é consultável pelos mesmos endpoints de
monitores e métricas de qualquer dispositivo; `/devices/{id}/metrics` e os
widgets de CPU e memória mostram o servidor sem código de frontend novo; não
existe `/api/runtime/*`.

#### Registro de execução

Entregue em `services/monitoring/health/` (contratos, parsers puros e fontes),
`checkers/system_health.rs`, `monitoring/managed.rs`, o caminho genérico de
séries em `result_processor.rs` e a migration
`m20260819_000002_monitors_managed_unique.rs`. Testes em
`tests/requests/system_health.rs` mais os unitários dos parsers e do
coordenador; suíte em 706 unitários + 196 de integração, verdes, com `fmt` e
`clippy` limpos.

Achados corrigidos durante a execução:

1. **O índice único `(device_id, type)` do roadmap quebraria o SNMP.** A coleta
   SNMP cria **dois** monitores `type = 'snmp'` no mesmo dispositivo —
   `cpu_usage` e `memory_usage` (ver `snmp::service`). Um índice global recusaria
   o segundo e derrubaria o upgrade de toda instalação com SNMP ligado. O índice
   entregue é **parcial** (`WHERE type = 'system_health'`), sintaxe que SQLite e
   PostgreSQL aceitam igual. É o único ponto do esquema em SQL cru, porque o
   `IndexCreate` do SeaORM não expressa índice parcial.
2. **Renomear o servidor criava um monitor de ping para o próprio nome.**
   `sync_device_monitor` roda em todo `PUT /api/devices/{id}` e usa
   `ip_address` **ou o nome** como alvo. O dispositivo do sistema não tem IP,
   então renomeá-lo geraria um ping para “Servidor NetMonitor” — uma checagem
   que só pode falhar, deixando o servidor `offline` para sempre. O
   `sync_device_monitor` agora sai cedo para o dispositivo protegido: seu
   monitoramento é a coleta gerenciada.
3. **O restore não reprovisionava o monitor.** Um arquivo anterior a esta fase
   restaura um servidor sem coleta: ele aparece na lista e nunca mais mede
   nada. `backup::restore` passou a chamar `ensure_system_health_monitor` junto
   com o `ensure` do dispositivo.
4. **`o_ciclo_do_scheduler_nao_consome_o_outbox` ficou ambíguo.** O teste
   afirmava que o ciclo não entrega evento SSE **algum** — o que era verdade só
   enquanto o ciclo não tinha monitor próprio para executar. Com a coleta de
   saúde provisionada no boot, ele passa a publicar legitimamente. A asserção
   foi reescrita sobre a **origem** do evento: o que o ciclo não pode entregar
   é a linha do `event_outbox`, e isso continua garantido pelo `relay_pending`.
5. **Contagens em `tests/requests/backup.rs`** precisaram excluir o monitor
   gerenciado, pelo mesmo motivo da Fase 1 — ele é um monitor comum, e é esse
   justamente o ponto da fase.
6. **Sem amostra anterior não existe uso de CPU nem taxa de tráfego.** Os dois
   são deltas de contador acumulado. A primeira coleta de cada processo declara
   `cpu_usage` e `inBps` **indisponíveis com motivo** em vez de publicar zero —
   um `0%` seria indistinguível de um servidor ocioso e enganaria o operador e
   o motor de alertas.

### Fase 3 — Regras de saúde para todo dispositivo `🟢 Concluído`

Esta fase deixou de ser “regras do servidor”. O servidor é o primeiro
dispositivo a usá-las.

- [x] Registrar no vocabulário do motor os campos de saúde de **dispositivo**
  (§3.2), com nome, unidade, operadores válidos e faixa aceitável.
- [x] Publicar esses campos no dataset do resultado `system_health` **e no
  dataset do SNMP**, substituindo os atuais `usagePercent`/`usedPercent`, que
  hoje não são avaliáveis por regra alguma. Nenhuma lógica de severidade dentro
  do coletor.
- [x] Adicionar templates de saúde ao catálogo, aplicáveis a qualquer
  dispositivo que publique os campos, no mínimo:
  - CPU acima de 85% por 5 minutos;
  - memória usada acima de 90% por 5 minutos;
  - armazenamento usado acima de 85% por 10 minutos.
- [x] **Corrigir a idempotência do catálogo para aceitar escopo.** Hoje
  `ExistingRules::matching` procura por `template_key` sem escopo e
  `template_signature` fixa `(None, None, None)`: aplicar o mesmo template a um
  segundo dispositivo devolve `already_exists` e não cria nada, em silêncio. A
  chave passa a ser `(template_key, site_id, device_id, monitor_id)` nas duas
  estruturas de índice.
- [x] Estender o catálogo para receber `deviceId`, calcular aplicabilidade a
  partir das capacidades do dispositivo e criar regras já vinculadas ao
  dispositivo ou monitor correto.
- [x] Aplicar os defaults uma única vez ao Servidor NetMonitor, de modo
  transacional e idempotente, **usando um marcador em `system_settings`** — é o
  mesmo mecanismo que `server_addresses` já usa, e não custa coluna nem tabela.
  `ensure_defaults` continua servindo à instalação nova (“não existe regra
  alguma”), que é uma pergunta diferente. Regra removida pelo usuário não
  reaparece no boot.
- [x] Permitir regra personalizada escolhendo métrica, comparação, limiar,
  duração, recuperação, severidade e cooldown; não aceitar campo livre fora do
  vocabulário publicado pelo backend.
- [x] Filtrar/listar regras por `deviceId` em `GET /api/alerts/rules` e manter a
  Central de Alertas como fonte única da verdade.
- [x] **Fechar o vão entre os dois vocabulários** exportando as chaves de
  `fields.rs` por `ts-rs` e tipando `alertPresentation.ts` contra elas. Hoje as
  chaves vivem no Rust e os rótulos no TypeScript, e — como o próprio comentário
  do `fields.rs` registra — renomear um lado apaga o outro sem erro de
  compilação em nenhum dos dois. Acrescentar quatro campos sem fechar isso
  dobraria a dívida.

**Aceite**: carga curta não alerta; CPU sustentada acima do limite abre um
evento vinculado ao dispositivo — servidor **ou** roteador SNMP; o mesmo
template aplicado a dois dispositivos cria duas regras; a mesma regra aparece e
pode ser editada tanto no dispositivo quanto em `/alerts`; renomear um campo no
Rust quebra o `typecheck` do frontend.

#### Registro de execução

Entregue em `alerts/fields.rs` (quatro campos novos), `dtos/alerts.rs` (o enum
`AlertField` exportado por `ts-rs`), `alerts/datasets/monitor_result.rs`
(`METRIC_FIELD_MAP`), `snmp/service.rs` (troca das chaves soltas),
`alerts/catalog/templates.rs` (três templates e a categoria `saude`),
`alerts/catalog/service.rs` (`TemplateScope`, `describe_for`, `apply_scoped`),
`alerts/catalog/health_defaults.rs` (o marcador) e
`services/devices/capabilities.rs` + `dtos/devices.rs` (a projeção). Testes em
`tests/requests/health_rules.rs`; suíte em 715 unitários + 204 de integração,
verdes, com `typecheck`, `format` e `lint` do frontend limpos.

Achados corrigidos durante a execução:

1. **A aplicabilidade precisava de uma fonte de verdade, e ela não existia.**
   O roadmap pede “calcular aplicabilidade a partir das capacidades do
   dispositivo”, mas a projeção de capacidades só era prevista na Fase 5. Foi
   antecipada — e é a mesma projeção, não uma prévia: `capabilities::for_device`
   serve à aplicabilidade agora e às abas e botões depois. Construí-la duas
   vezes é que teria sido o erro.
2. **A aplicabilidade é decidida pelo `condition.field`, não pela categoria.**
   Categoria é rótulo de tela; o que decide se uma regra pode disparar é o
   dispositivo publicar o campo que ela compara. Um template de CPU oferecido a
   quem só responde ping criaria uma regra que nunca dispara — pior que não
   oferecer, porque parece configurado.
3. **Os templates de saúde nascem `recommended: false`.** O conjunto básico é
   **global** (`ensure_defaults` cria regras sem escopo), e uma regra de CPU sem
   escopo dispararia para o parque inteiro, inclusive para quem não publica o
   campo. Quem os aplica é o catálogo por dispositivo — via `health_defaults`
   no servidor, via tela nos demais.
4. **A contagem de templates estava fixada em dois testes.**
   `templates.rs` e `tests/requests/phase6_phase7.rs` afirmavam 25; agora são
   28. Atualizados com o motivo escrito, não com o número trocado.
5. **`AlertRuleTemplateView` ganhou `applicable`,** e o teste de serialização do
   catálogo precisou do campo. O `false` só aparece quando há dispositivo no
   escopo: no catálogo global tudo é aplicável, porque ainda não há dispositivo
   escolhido.

### Fase 4 — Logs internos como logs do dispositivo `🟢 Concluído`

- [x] **Reusar a fila e o escritor existentes.** A camada `tracing` monta um
  `PendingLog` e chama `LogQueue::try_enqueue`, que nunca faz `await` e já
  contabiliza descarte. Escrita em lote, live tail, SSE, busca, FTS, paginação e
  retenção vêm sem uma linha nova. Não se cria fila, escritor nem barramento.
- [x] Mover a montagem do pipeline (`syslog::build` — fila, escritor,
  barramento) de `initializers::syslog` para `Hooks::after_context`, deixando
  apenas `spawn_listeners` no initializer.
- [x] Instalar a camada por `Hooks::init_logger`, compondo com
  `logger::init_env_filter::<App>` e `logger::init_layer` para herdar
  exatamente a política de filtro e o formato do `config.logger`.
- [x] **Desamarrar o banco de logs do servidor de syslog.** O flag passa a valer
  apenas para o listener (`initializers/syslog.rs`), que é quem abre porta.
- [x] Acrescentar a `device_logs` **uma única coluna** de origem tipada
  (`syslog` | `application`). Nada de coluna JSON de contexto.
- [x] Mapear o evento nos campos que já existem: `severity` recebe a severidade
  syslog equivalente ao nível (`ERROR`→3, `WARN`→4, `INFO`→6, `DEBUG`/`TRACE`→7),
  `app_name` recebe o `target`, `pid` o PID do processo.
- [x] Definir `source_ip` para a origem local (`127.0.0.1`). A camada **não**
  passa pelo `Ingestor`.
- [x] Vincular cada entrada ao `device_id` do resolvedor com cache da Fase 1.
  Linhas emitidas antes de o dispositivo existir vão com `device_id` nulo.
- [x] Ignorar o próprio writer, consultas SQL bem-sucedidas e alvos ruidosos
  definidos em uma política testável.
- [x] Manter stdout para operação do container, sem duplicar eventos dentro da
  aplicação e sem esconder `WARN`/`ERROR` do SQLx.
- [x] **Decidir e registrar o efeito na retenção.** A decisão é **aceitar a
  disputa** pelos 4 GB entre log de aplicação e syslog do parque — cota por
  origem custaria mais complexidade do que resolve. Documentada no runbook
  (Fase 7).
- [x] Testar lote, ordem, filtros, overflow, retenção, boot, log antes da
  resolução do dispositivo e a ausência de realimentação `log → INSERT → log`.

**Aceite**: após o boot, selecionar Servidor NetMonitor em `/logs` mostra seus
logs ao vivo; com `SYSLOG_ENABLED=false` os logs internos continuam gravando e
só o listener some; não existe aba “Servidor”, endpoint separado, segunda fila,
segundo escritor nem tabela `runtime_logs`.

#### Registro de execução

Entregue em `syslog/app_layer.rs` (a camada), `syslog/queue.rs` (`LogSource`),
`syslog/mod.rs` (`install_queue` no `build`), `app.rs` (`init_logger` e a
montagem em `after_context`), `initializers/syslog.rs` (só listeners),
`syslog/db.rs` (flag desamarrado) e a migration
`logs/m20260819_000001_device_logs_source.rs`. Testes em
`tests/requests/app_logs.rs`; suíte em 721 unitários + 211 de integração.

Achados corrigidos durante a execução:

1. **`OnceLock` para a fila da camada travava a suíte inteira.** A primeira
   fila do processo valeria para sempre: o segundo `syslog::build` gravaria num
   canal cujo escritor já morreu e a linha sumiria sem erro. Trocado por
   `RwLock<Option<LogQueue>>`, com `clear_queue()` para o desligamento. O
   sintoma foi um teste que nunca terminava — o escritor esperava o canal
   fechar, e o `OnceLock` segurava um clone do remetente para sempre.
2. **`Hooks::init_logger` não é chamado pelo harness de teste** (só pelos
   caminhos de `cli.rs`). Os testes instalam a camada com
   `tracing::subscriber::with_default`, que é por thread — um `.init()` global
   dentro da suíte derrubaria todos os demais testes.
3. **O `file_appender` do Loco deixa de funcionar por configuração.** Assumir
   `init_logger` significa assumir a montagem inteira. Nenhum `config/*.yaml`
   deste projeto o habilita, então nada se perdeu — mas a consequência está
   escrita no lugar onde alguém a encontraria ao tentar ligá-lo.
4. **`PendingLog` e `device_logs::Model` são construídos em três testes de
   unidade** (`queue.rs`, `writer.rs`, `views/logs.rs`). O campo novo quebrou os
   três; corrigidos com `LogSource::Syslog` / `"syslog"`, que é o valor correto.

### Fase 5 — Página do dispositivo orientada a capacidades `🟢 Concluído`

- [x] Criar no backend uma projeção `capabilities` para a página de detalhe; o
  frontend não deduz suporte a partir de nome ou ID.
- [x] Definir `snmp.connected` por comunicação bem-sucedida persistida.
  `devices.snmp_enabled` é campo de cadastro, não prova de conexão. O estado
  “configurado, mas ainda não conectado” aparece como ação na Visão Geral.
- [x] Extrair componentes reutilizáveis para resumo de saúde, lista de regras,
  métricas de monitor, tráfego de interface e logs filtrados.
- [x] Incorporar toda a saúde do Servidor NetMonitor em **Visão Geral**.
- [x] Remover a aba **Métricas & Tráfego** e mover cada conteúdo para seu dono:
  monitor, interface ou card da visão geral.
- [x] Exibir **Interfaces SNMP**, **Logs**, **Eventos** e **VPN** somente quando
  a capacidade correspondente estiver disponível.
- [x] **Aplicar as mesmas capacidades aos botões do cabeçalho**, e não só às
  abas: varredura SNMP, coleta SNMP, escaneamento de portas e edição de
  identidade não aparecem onde não fazem sentido.
- [x] Adicionar a aba **Regras** com contagem, defaults recomendados, criação,
  edição, ativação e exclusão, reutilizando os componentes de `/alerts`.
- [x] Garantir layout responsivo, estados de loading/erro/vazio e navegação por
  teclado, sem ações duplicadas no cabeçalho e nas abas.

**Aceite**: `/devices/{id}` não apresenta abas nem botões inaplicáveis; o
Servidor NetMonitor tem saúde na Visão Geral, regras no fluxo comum e logs já
ativos.

#### Registro de execução

Entregue em `services/devices/capabilities.rs` + `dtos/devices.rs` (a projeção,
antecipada na Fase 3), `GET /api/devices/{id}/capabilities`,
`components/devices/DeviceHealthSummary.vue`,
`components/devices/DeviceRulesTab.vue`, `stores/deviceDetail.ts` e a
reestruturação de `pages/DeviceDetailPage.vue`. `typecheck`, `lint`, `format` e
`build` do frontend limpos.

Achados corrigidos durante a execução:

1. **A tela deduzia “está monitorando CPU?” pelo nome do monitor.**
   `isCpuMonitored` procurava a palavra `cpu` no rótulo, e `isMemoryMonitored`
   normalizava acento para procurar `memoria`. Renomear o monitor apagava o
   card; chamar um monitor de ping de “CPU do roteador” o acendia sem dado
   algum. O componente novo decide pelas **séries gravadas** — se há amostra de
   `cpu_usage`, há card; se não há, não há.
2. **Uma métrica indisponível gerava card com `0%`.** O antigo caía em
   `cpuUsageValue || 0`. Um `0%` de CPU é indistinguível de um servidor ocioso.
   Agora a série ausente simplesmente não produz card, e o backend registra o
   motivo da indisponibilidade no `data` do resultado.
3. **A aba pedida na URL podia deixar de existir.** `?tab=interfaces` num
   equipamento cujo SNMP nunca respondeu abria uma aba vazia. A página volta
   para `overview` quando a aba deixa de ser aplicável — sem erro e sem
   conteúdo vazio, como manda a regra de layout.
4. **O `v-btn-group` do cabeçalho ficaria vazio no dispositivo do sistema.** Um
   grupo de botões sem nenhum botão dentro renderiza uma moldura solta. A
   capacidade `anyHeaderAction` esconde o grupo inteiro quando nenhuma ação é
   válida.

### Fase 6 — Central de alertas e logs unificadas `🟢 Concluído`

- [x] Transformar o diálogo de regras pré-configuradas em componente contextual
  por dispositivo, compartilhado entre `/alerts` e `/devices/{id}`.
- [x] Mostrar escopo, origem e atalho do dispositivo em Regras Configuradas e
  nos eventos de alerta.
- [x] Incluir o Servidor NetMonitor no seletor normal de dispositivos de `/logs`
  e manter filtros, URL, paginação e live tail ao trocar a seleção.
- [x] **Verificar** — não reimplementar — que a `LogsPage` continua única e que
  a aba Logs do dispositivo segue usando a mesma `useLogsStore` e o mesmo
  `LogTable`.
- [x] Revisar textos: “servidor” descreve o dispositivo selecionado, não uma
  categoria paralela do produto.

**Aceite**: uma regra ou um log possui a mesma representação em todas as telas;
trocar de tela não muda o recurso nem cria cópia.

#### Registro de execução

Entregue em `stores/alerts.ts` (`AlertRuleScope` e as três chamadas com
escopo), `components/AlertRuleCatalogDialog.vue` (o mesmo diálogo nas duas
telas) e `pages/AlertsPage.vue` (coluna de escopo, atalho para
`/devices/{id}?tab=rules` e escolha do dispositivo antes do catálogo). A
verificação virou teste executável em
`backend/tests/conventions/tela_unificada.rs`.

Achados corrigidos durante a execução:

1. **A verificação pedida pelo roadmap não tinha onde morar.** O frontend não
   tem runner de testes, então “verificar que a `LogsPage` continua única” só
   podia ser um comentário — e comentário não impede uma fase seguinte de
   quebrar. Foi transformada em teste de convenção no Rust, que lê o código do
   frontend, exatamente como o `camel_case.rs` já fazia com o do backend. Os
   mesmos testes guardam a ausência de `/api/runtime/*`, a ausência da aba
   depósito de métricas e a proibição de identificar o servidor por nome ou por
   ID fixo.
2. **Duas regras do mesmo template ficavam indistinguíveis na lista.** Com o
   escopo por dispositivo da Fase 3, “CPU acima de 85%” passa a existir várias
   vezes. Sem a coluna de escopo, a Central mostrava linhas de nome idêntico e
   nada dizia de quem era cada uma.
3. **O seletor de dispositivos de `/logs` já incluía o servidor** — ele é um
   dispositivo como qualquer outro, e a lista vem de `devicesStore.devices`.
   Item verificado, não implementado, que é o que o roadmap pedia.

### Fase 7 — Remoção, documentação e validação final `🟢 Concluído`

- [x] Remover código duplicado, flags temporárias, componentes sem uso e
  qualquer caminho `runtime_*` introduzido durante a execução das fases.
- [x] Atualizar `docs/arquitetura.md`, contratos da API e runbook de diagnóstico
  com o fluxo único por dispositivo, incluindo a disputa de retenção da Fase 4.
- [x] Validar backup/restore com atenção ao que a Fase 1 previu: exportar com o
  dispositivo presente, restaurar num sistema que já tem o seu, restaurar um
  arquivo anterior à feature, e confirmar em todos os casos que o dispositivo
  volta correto, que os logs internos seguintes vão para ele e que não sobra
  linha órfã nem FK quebrada.
- [x] Confirmar retenção e limpeza do dispositivo protegido e de suas regras.
- [x] Validar upgrade em SQLite e PostgreSQL; validar instalação vazia e reinício
  com dados existentes.
- [x] Executar toda a matriz obrigatória abaixo e registrar o resultado nesta
  fase antes de marcá-la como concluída.

#### Registro de execução

**Documentação.** `docs/arquitetura.md` ganhou a seção 11-A ("O próprio
NetMonitor como dispositivo") com o fluxo, as duas séries, os dois vocabulários,
as capacidades e a decisão de retenção; a seção "O que não existe" registra a
ausência de segundo pipeline de log e de observador externo do processo.
`docs/runbook_diagnostico.md` é novo e cobre os sete procedimentos de campo,
incluindo a disputa de 4 GB entre log de aplicação e syslog.

**Limpeza.** Não há `runtime_*` em `backend/src`, `backend/migration` nem
`frontend/src` — e agora um teste de convenção
(`tests/conventions/tela_unificada.rs`) impede que volte. O código morto deixado
pela extração da Fase 5 (`isCpuMonitored`, `isMemoryMonitored`, os quatro
helpers de cor e os `computed` de histórico) foi removido de
`DeviceDetailPage.vue`.

**Validação de ciclo.** `tests/requests/system_device_lifecycle.rs` cobre os
três casos de backup previstos pela Fase 1, o ciclo normal das regras do
dispositivo protegido, a instalação vazia e — explicitamente — que a recusa de
exclusão continua sendo **400** e não **403**: um 403 significaria uma terceira
categoria na política de acesso, que a Fase 1 proíbe.

**Matriz obrigatória, executada nesta ordem:**

| Comando | Resultado |
|---|---|
| `cargo fmt --all --check` | limpo |
| `cargo clippy --all-targets -- -D warnings` | limpo |
| `cargo test` | 721 unitários + 223 de integração, 0 falhas |
| `cargo build --release` | ok |
| `npm --prefix frontend run typecheck` | limpo |
| `npm --prefix frontend run format` | aplicado **depois** do `cargo test`, como manda a nota |
| `npm --prefix frontend run lint` | 0 erros, 0 avisos |
| `npm --prefix frontend run build` | ok |

**Bancos.** A suíte roda contra o backend configurado em `config/test.yaml`.
Toda migration desta entrega foi escrita para os dois bancos: a coluna
`devices.system_key` usa `ADD COLUMN` + `CREATE UNIQUE INDEX` separados (o
SQLite não aceita `ADD COLUMN UNIQUE`, e `NULL`s são distintos nos dois); o
índice de monitor gerenciado é **parcial** (`WHERE type = 'system_health'`),
sintaxe válida em SQLite ≥ 3.8 e em PostgreSQL; a coluna `device_logs.source`
usa `NOT NULL DEFAULT 'syslog'`, que evita reescrever a tabela inteira.

## 5. Matriz obrigatória de validação

```bash
# backend/
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release

# raiz do projeto
npm --prefix frontend run typecheck
npm --prefix frontend run format
npm --prefix frontend run lint
npm --prefix frontend run build
```

> `cargo test` regenera os bindings do `ts-rs`. Rode `npm --prefix frontend run
> format` **depois** dele, nunca antes.

Além da suíte automatizada:

- instalação vazia cria um único Servidor NetMonitor, monitor e conjunto de
  defaults;
- reinício não duplica dispositivo, monitor nem regra;
- restaurar um backup mantém o dispositivo do sistema correto, e o log interno
  seguinte vai para ele — não para outro equipamento;
- CPU alta abaixo da duração não alerta e acima da duração alerta;
- o mesmo template de CPU aplicado ao servidor e a um roteador SNMP cria **duas**
  regras, e as duas disparam;
- `/devices/{id}/metrics` devolve as séries de saúde do servidor sem endpoint
  novo;
- SNMP habilitado sem resposta não mostra a aba Interfaces SNMP nem os botões de
  varredura e coleta;
- dispositivo sem tráfego não mostra área de tráfego;
- log interno aparece em `/logs` e na aba do mesmo dispositivo;
- com `SYSLOG_ENABLED=false`, o log interno continua sendo gravado e consultável;
- SQL de sucesso não entra no banco de logs e não polui stdout por padrão;
- excluir ou alterar a identidade do dispositivo protegido retorna erro de
  negócio em português, para qualquer perfil;
- `viewer` só lê e `operator` escreve, exatamente como já é hoje — este roadmap
  não muda a política de acesso.

## 6. Fora de escopo e itens proibidos

- Monitorar a queda total do próprio processo: um processo parado não consegue
  alertar sobre si; isso exige um observador externo.
- Regra sobre ausência de coleta: o motor é orientado a evento e não avalia o
  que não chegou (ver nota da Fase 3).
- Criar dashboard, rota ou aba exclusiva para “servidor”.
- Criar `runtime_logs`, `runtime_metrics`, `/api/runtime/*` ou stores paralelos.
- Criar segunda fila, segundo escritor em lote ou segundo barramento de log.
- Criar vocabulário de alerta, nome de métrica, template ou componente que só
  faça sentido para o servidor.
- Copiar latência e perda de pacote para `metrics`.
- Identificar o Servidor NetMonitor por nome, posição na lista ou ID fixo.
- Exibir aba ou botão por configuração otimista quando ainda não houve conexão
  real.
- Alterar a política de perfis de acesso para acomodar o dispositivo protegido.
- Manter compatibilidade com a tentativa não entregue que foi descartada na
  Fase 0.

## 6-A. Revisão de uso — o que a operação real encontrou

Depois de as sete fases fecharem, o produto foi exercitado na tela. Os itens
abaixo não eram desvios do roadmap: eram lugares onde a implementação **dizia**
cumpri-lo e não cumpria. Cada um está corrigido, com teste.

### 1. O formulário de regra não era compartilhado

A Fase 6 pede: "criar ou editar uma regra usa um único componente compartilhado
nas duas páginas". O que existia era um formulário **inline** em
`AlertsPage.vue` e, na aba Regras do dispositivo, um *link* para ele. Duas
consequências: criar a regra a partir do equipamento perdia o escopo no
caminho — o operador tinha de escolher o dispositivo de novo, do zero — e todo
campo novo precisaria nascer duas vezes.

Extraído para `components/AlertRuleFormDialog.vue`, usado pelas duas telas. O
escopo virou o **primeiro** campo do formulário, com "Todos os dispositivos"
entre as opções. Aberto de dentro de um equipamento, ele vem preenchido e
travado; aberto por `/alerts` com um filtro ativo, vem preenchido e editável.
Guardado por `tests/conventions/tela_unificada.rs`.

### 2. `PUT /api/alert-rules/{id}` não conseguia limpar o escopo

`input.device_id.or(current.device_id)` mantinha o dispositivo quando o campo
chegava vazio — e um `Option<i64>` não distingue "campo ausente" de "campo
`null`". O `PUT` é parcial de propósito (o toggle da lista manda só `enabled`),
então a primeira leitura estava certa; a segunda não existia. Resultado: uma
regra vinculada por engano a um dispositivo ficava presa a ele para sempre, e a
tela oferecia uma opção que o backend ignorava em silêncio.

As três dimensões de escopo passaram a `Option<Option<i64>>`, com um
desserializador que embrulha o valor lido em `Some`. Campo ausente mantém;
`null` explícito limpa. Coberto por
`o_escopo_de_uma_regra_pode_voltar_para_todos_os_dispositivos`.

### 3. `/alerts?tab=rules&deviceId=1` abria a aba errada

A `AlertsPage` nunca leu a query. O atalho que a Fase 6 criou — "com atalho para
`/devices/{id}?tab=rules`" — funcionava só num sentido. Agora `tab`, `deviceId`
e `ruleId` vêm da URL, a aba escolhida na tela volta para a URL, e o recorte por
dispositivo é **anunciado** com um aviso e um botão "Ver todas": uma lista
filtrada que não se anuncia parece uma lista curta, e o operador conclui que
perdeu regras.

### 4. O catálogo escondia-se atrás de uma pergunta

"Regras Pré-configuradas" abria um diálogo perguntando o dispositivo **antes**
de mostrar qualquer coisa, e sem oferecer "todos". Era uma leitura literal
demais de "começa pela escolha do dispositivo": o operador só consegue escolher
o escopo depois de ver as opções, e regras como indisponibilidade e latência são
genuinamente globais. O seletor passou para dentro do próprio catálogo, com
"Todos os dispositivos" como padrão; trocar o dispositivo recarrega a lista,
porque "já configurada" e "indisponível neste equipamento" são respostas **por
escopo**, não propriedades do template.

### 5. O servidor recebia um template de alcance que nunca dispararia

O Servidor NetMonitor aparecia como candidato a "Dispositivo sem resposta". O
`status` de uma coleta de saúde descreve a *coleta*, não o alcance: ela devolve
`up` quando mediu algo e `unknown` quando não conseguiu medir nada — **nunca**
`down`. A regra seria criada e ficaria inerte; e o caso que ela descreveria — o
processo parado — é o que a seção 6 já declara fora de escopo, porque um
processo parado não alerta sobre si.

`capabilities::published_fields` deixou de publicar `STATUS` para o monitor
gerenciado. O template continua no catálogo global; ele apenas não é oferecido a
quem não pode usá-lo.

### 6. `/monitors/{id}` tirava o operador do contexto

O detalhe de um monitor é sempre consultado **a partir de** alguma coisa: a
lista, a página do dispositivo, o painel. Abri-lo como tela cheia custava ao
operador o caminho de volta. `MonitorDetailPage` virou um invólucro que monta
`MonitorDetailDialog`; o conteúdo mudou-se para
`components/monitors/MonitorDetailView.vue`, que serve à rota e ao diálogo sem
duas cópias para envelhecer. Os pontos que linkavam para lá abrem o diálogo, com
`@click.stop` para não engolir o clique dos botões de linha, e mantendo o
`href` — abrir em nova aba e copiar o link continuam funcionando, e a rota monta
o mesmo diálogo. Guardado por `o_detalhe_do_monitor_so_abre_em_dialogo`.

### 7. Os cards de saúde não abriam nada

A regra de layout diz: "gráficos detalhados são abertos a partir do card,
monitor ou interface que os originou". Os cards mostravam sparkline e paravam
aí. Criado `components/devices/MetricHistoryDialog.vue` — o par do
`TrafficChartDialog` para as séries que não são de interface —, sobre o mesmo
`BaseMetricChart`. O card virou alvo clicável com `role`, `tabindex` e resposta
a Enter/Espaço.

### 8. O resumo de tráfego estava na aba errada

Tráfego agregado é uma **métrica principal** do equipamento, e é isso que a
Visão Geral apresenta; o que pertence à interface é o detalhe dela — inventário,
estado, velocidade, gráfico daquela porta. A tabela-resumo mudou-se para a Visão
Geral e some inteira quando não há interface monitorada, o que também entrega o
critério "dispositivo sem tráfego não mostra área de tráfego".

### 9. Três campos da projeção não eram consultados por ninguém

`traffic`, `health` e `canDelete` viajavam em toda resposta de
`/devices/{id}/capabilities` sem um único leitor. Campo de contrato que ninguém
lê é dívida com aparência de completude. `traffic` e `canDelete` saíram — o
primeiro é derivável das interfaces e métricas que a página já tem, o segundo é
`isSystem`, que viaja no próprio dispositivo. `health` ficou e passou a **ser
usado**: a seção de saúde inteira some para quem não publica série alguma, em
vez de mostrar um aviso de vazio a um alvo que só responde ping.

No mesmo movimento, `/devices` deixou de oferecer editar, escanear portas e
excluir para o dispositivo do sistema — pela mesma `isSystem`, nunca pelo nome.

## 7. O que mudou nesta revisão

Auditoria do roadmap original contra o código, em 19/08/2026. Cada item abaixo
travava a implementação ou produzia um segundo fluxo.

| # | Fase | Achado | Efeito no roadmap |
|---|---|---|---|
| 1 | 3 | A idempotência do catálogo é global por `template_key`, e templates nascem sempre sem escopo | Item explícito de correção; sem ele, aplicar o mesmo template a um segundo dispositivo falha em silêncio |
| 2 | 3 | Não existe campo de CPU/memória no vocabulário — nem para o servidor, nem para o parque | Campos passam a ser de dispositivo e o dataset do SNMP publica os mesmos; o roadmap entrega alerta de CPU para todo o inventário |
| 3 | 3 | “Coleta ausente” não dispara num motor orientado a evento | Template removido, com o motivo registrado |
| 4 | 4 | Fila, escrita em lote e barramento já existem em `syslog/` | A camada `tracing` passa a usá-los; construir um segundo pipeline virou item proibido |
| 5 | 4 | `Hooks::init_logger` roda depois de `after_context` e antes dos initializers | O pipeline de log muda de lugar no boot; `run_task` deixa de ficar sem log |
| 6 | 4 | `db::install` desiste com `SYSLOG_ENABLED=false`; `source_ip` é `NOT NULL` | Flag passa a valer só para o listener; origem local definida |
| 7 | 4 | Coluna JSON de contexto criaria busca invisível ao FTS | Só uma coluna de origem; campos achatados na mensagem |
| 8 | 1 e 7 | `backup::restore` faz `wipe` + recarga com os IDs do arquivo | Reexecução do serviço e invalidação do cache após restore, com teste próprio |
| 9 | 1 | Migrations do banco principal convergem depois do `after_context` | O serviço de identidade roda num `Initializer` |
| 10 | 1 | Contradição: “proteger exclusão” x “restrito a `admin`” — e a política real tem duas linhas | Vira regra de negócio; a política de acesso não muda |
| 11 | 2 | `metrics` só é escrita pelo SNMP e pela VPN; `CheckResult.metrics` é descartado | Caminho genérico em `process_result`, restrito a séries de dispositivo (§3.1) |
| 12 | 2 | `run_monitor` não conhece o tipo; `execute_one` despacharia para probe; `retry_count` releria `/proc` 4× | Três itens novos na Fase 2 |
| 13 | 3 | `ensure_defaults` usa “nenhuma regra existe” como marcador global | Marcador por dispositivo em `system_settings` |
| 14 | 3 | Vocabulário duplicado entre `fields.rs` e `alertPresentation.ts`, sem erro de compilação no drift | Exportação por `ts-rs` e tipagem do lado do frontend |
| 15 | 5 | As capacidades governavam só as abas | Passam a governar os botões do cabeçalho |
| 16 | 4 | Retenção corta por tamanho; aplicação e syslog dividem 4 GB | Decisão explícita de aceitar, documentada no runbook |
| 17 | 6 | A unificação de `/logs` já está feita no código | Item vira verificação, não trabalho |
