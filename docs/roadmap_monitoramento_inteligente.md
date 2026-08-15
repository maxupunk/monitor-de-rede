# Roadmap — Alertas Inteligentes e Detecção de Instabilidade

> **Escopo**: este roadmap cobre a evolução do motor de alertas para tratar
> **oscilação (flapping)**, **instabilidade intermitente** (perda de pacotes,
> DNS falhando de vez em quando, interface Ethernet caindo e voltando) e a
> **fadiga de notificações** que esses cenários geram. Não é uma lista de
> features soltas: é a construção de um conceito que hoje não existe no
> sistema — o de **estabilidade como condição de resolução**.
>
> Estado atual mapeado em `backend/src/services/alerts/`, `monitoring/` e
> `tasks/scheduler_run.rs`. Ler junto com [arquitetura.md](arquitetura.md) §5
> e com a análise de design detalhada em
> [analise_monitoramento_inteligente.md](analise_monitoramento_inteligente.md)
> (arquitetura SOLID do motor e UX dos novos estados).
>
> **Princípio de execução (registrado na decisão de implementar a Fase 1)**:
> cada fase é implementada **sem deixar legado** — os pré-requisitos
> arquiteturais são pagos junto com a feature (máquina de estados pura, enum
> de status, contratos tipados), código morto ou duplicado é removido na
> mesma entrega, e a validação completa (backend e frontend) é critério de
> aceite. Rapidez nunca justifica implementação dupla, compatibilidade
> provisória ou gambiara "temporária" — mesmo que fazer certo leve mais
> tempo.

## 1. O problema

Hoje o ciclo de vida de um alerta é binário e míope:

- **Dispara** quando a condição da regra se sustenta por `duration_seconds`
  (histerese temporal em memória, `manager.rs:137-156`) — a única proteção
  existente.
- **Resolve na primeira checagem ok** (`manager.rs:78-80` →
  `recovery::resolve_scope`). Não há exigência de N sucessos consecutivos nem
  de tempo mínimo de estabilidade.
- **Cada par dispara+resolve gera duas notificações** (🚨 + ✅). Um link que
  cai e volta 20 vezes por hora gera 40 mensagens — e o usuário aprende a
  ignorar o canal, que é o pior desfecho possível para um sistema de alertas.
- **Não há memória de instabilidade**: quando o alerta resolve, o sistema
  "esquece" que aquele alvo oscilou. O usuário não fica sabendo que o link
  está degradado e provavelmente vai cair de novo.

O comportamento desejado (conforme levantado na discussão de origem deste
roadmap):

1. Alvo que **cai e volta dentro de uma janela curta** não resolve o alerta —
  ele entra num estado intermediário de **estabilização/aviso**.
2. O alerta só fica **resolvido de verdade** depois que o alvo se mantém
  estável por um período configurável, **contado a partir do último
  down/problema** (cada recaída reinicia a contagem).
3. Durante a estabilização, o usuário vê o alerta como **aviso** ("voltou, mas
  esteve oscilando — fique de olho"), com detalhes: tipo de problema (down,
  perda de pacotes, DNS, interface), número de recaídas, horário do último
  evento.
4. Notificações deixam de ser 1:1 com transições e passam a respeitar
  **cooldown e agrupamento**.

## 2. O que o mercado já resolveu (referências)

Vale copiar o que funciona em vez de inventar:

- **Nagios — flap detection**: mede o percentual de mudança de estado num
  histórico deslizante (últimas N checagens). Acima de um limiar, declara o
  alvo "flapping", **suprime notificações** e emite um único evento
  `FLAPPING STARTED/STOPPED`. Simples e comprovadamente eficaz.
- **Zabbix — histerese de recuperação**: a expressão de *problema* e a de
  *resolução* são independentes (`PROBLEM: perda > 10%`, `RECOVERY: perda < 2%
  por 10 min`). A faixa morta entre os dois limiares evita oscilação de
  estado. Conceito diretamente aplicável às nossas `alert_rules.condition`.
- **Prometheus — `for:` e `keep_firing_for`**: o alerta só dispara sustentado
  (já temos, via `duration_seconds`) e pode **continuar firing por um período
  mesmo após a condição sumir** (`keep_firing_for`) — é exatamente o "alerta
  permanece até superar o tempo do último down".
- **BGP route flap damping**: cada flap acumula uma *penalidade* que decai
  exponencialmente; acima de `suppress-threshold` o alvo é suprimido, abaixo
  de `reuse-threshold` volta. Modelo mais sofisticado que o do Nagios — útil
  se quisermos um "score de instabilidade" contínuo em vez de estado binário.
- **Alertmanager — grouping/inhibition/cooldown**: notificações agrupadas por
  janela (digest), silenciamento de alertas-filho quando o pai cai
  (inibição), e intervalo mínimo entre repetições.

## 3. Modelo conceitual proposto

### 3.1 Máquina de estados do alerta

```
            condição bateu (sustentada)
                 │
                 ▼
        ┌─────────────────┐   condição sumiu
        │     active      │ ──────────────────────▶ ┌──────────────────┐
        │ (notifica, dedup)│                        │  recovering ⚠️    │
        └─────────────────┘                         │ ("voltou, mas      │
                 ▲                                  │  esteve instável") │
                 │ recaída dentro da janela         └──────────────────┘
                 └──────────────────────────────────────│          │
                                          estável por `recovery_    │
                                          window_seconds` desde o   │
                                          último problema           ▼
                                                        ┌──────────────────┐
                                                        │    resolved      │
                                                        │ (notifica 1x)    │
                                                        └──────────────────┘
```

Regras-chave:

- `recovering` é um estado **aberto** para dedup (recaída não cria evento
  novo nem notifica de novo — apenas atualiza `data.recurrence_count` e
  `data.last_problem_at`).
- A resolução exige `now - last_problem_at >= recovery_window_seconds`
  **e** o status atual ok. Cada recaída reinicia o relógio.
- Opcional e posterior: um terceiro estado `flapping` (via percentual de
  transições, estilo Nagios, ou score com decaimento, estilo BGP) que
  **suprime notificações** e sinaliza o alvo como cronicamente instável.

### 3.2 Dados que o alerta precisa carregar

`alert_events.data` já é JSON — nada de migração de schema para começar:

- `problem_kind`: `down | packet_loss | latency | dns_failure | interface_flap | vpn_instability`
- `recurrence_count`: quantas recaídas desde a abertura
- `last_problem_at`: timestamp do último problema (reinicia a janela)
- `first_seen_at` / timeline resumida das transições
- Métricas do episódio: perda média de pacotes, latência p95, taxa de falha DNS

### 3.3 Detalhamento por tipo de problema

O usuário pediu explicitamente que o alerta diga **o que** está acontecendo,
não só "caiu". Isso já é quase gratuito: os checkers já produzem os campos
(`packet_loss` no ping, `dns_success_rate` no DNS, `if_oper_status` no SNMP).
O trabalho é de **classificação e apresentação**, não de coleta:

- Interface Ethernet desconectando → `if_oper_status` alternando
  up/down + contagem de transições.
- Perda de pacotes intermitente → status `warning` do ping hoje não gera
  memória; deve contar como "problema" para a janela de recuperação.
- DNS instável → `dns_success_rate < 100%` parcial, hoje diluído.

### 3.4 Obs.: tudo passa pelo sistema de regras existente

> **Observação de escopo (decisão de design)**: o sistema **já tem um motor de
> regras** completo — `alert_rules` com escopo (global/site/device/monitor),
> `condition` JSON, severidade, `duration_seconds`, catálogo de templates e
> CRUD na Central de Alertas. A implementação deste roadmap **não deve criar
> um mecanismo paralelo de configuração**: cada comportamento novo deve ser
> expresso como **parâmetro de regra**, para que o usuário verifique e ajuste
> a situação pela tela de regras que já conhece.
>
> Na prática:
>
> - `recovery_window_seconds` é **campo de `alert_rules`**, editável no
>   formulário de regra — não constante global nem variável de ambiente.
> - Os limiares de flapping da Fase 3 (transições por janela) idem: campo da
>   regra, com default no catálogo de templates.
> - O estado `recovering`/`flapping` de um alerta deve ser **rastreável até a
>   regra que o gerou** (já é, via `alert_rule_id`), e a tela do alerta deve
>   linkar para a regra correspondente — o usuário vê a situação e, no mesmo
>   caminho, ajusta a tolerância.
> - O catálogo (`alerts/catalog/templates.rs`) é o veículo dos defaults
>   sensatos: quem não quer configurar nada aplica o catálogo e ganha a
>   histerese de resolução pronta.
> - Escopo anulável já existente (global/site/device/monitor) permite ao
>   usuário ter, por exemplo, janela de 10 min para links críticos e 2 min
>   para o resto — sem código novo, só composição de regras.

## 4. Fases

### Fase 1 — Histerese de resolução (núcleo do pedido) `🟢 Concluído`

A menor mudança que já elimina o par 🚨+✅ a cada oscilação.

> **Entrega (2026-08-15)**: migration
> `m20260815_000001_alert_rules_recovery_window`; máquina de estados pura em
> `services/alerts/state_machine.rs` (Clock injetável, 12 testes de tabela);
> `enum AlertStatus` tipado substituindo as consts string (sem legado);
> `manager.rs` virou orquestrador; `feed.rs` centraliza os payloads SSE
> (`alert:updated` novo); notificação de resolução com resumo do episódio;
> `close_scope` para fechamento administrativo (monitor desativado não fica
> preso em `recovering`); frontend com `recovering` tipado, `RECOVERY_WINDOWS`
> + terceira cláusula no `describeRule`, chip "Estabilizando" e linha
> "último problema há X · N recaídas". Validação completa verde: fmt, clippy
> `-D warnings`, 375+98 testes, build release; typecheck, format, lint e
> build do frontend.

- [x] Adicionar `recovery_window_seconds` a `alert_rules` (migration +
  entidade; default sensato, ex.: 300 s; 0 = comportamento atual) e expor o
  campo no formulário de regras da Central de Alertas — é parâmetro de regra,
  configurável pelo usuário por escopo (§3.4).
- [x] Em `manager.rs`, substituir a resolução imediata por transição para
  `recovering`: registrar `last_problem_at` em `data` e só fechar o evento
  quando a janela se esgotar sem recaída.
- [x] Recaída durante `recovering`: atualizar `recurrence_count` e
  `last_problem_at`, **sem** novo evento e **sem** notificação.
- [x] Persistir o estado de recuperação no banco (não em memória como
  `pending_since`) para sobreviver a restart do processo.
- [x] Endpoint/store/aba na Central de Alertas refletindo o novo estado
  (badge "estabilizando", contador de recaídas, último problema).
- [x] Notificação única de resolução ao fim da janela, com resumo do episódio
  ("oscilou 7 vezes em 42 min, estável há 5 min").
- [x] Testes: recaída reinicia janela; janela vencida resolve; restart do
  processo não perde o estado; `#[serial]` onde tocar estado global.

**Critério de aceite**: link caindo e voltando a cada 30 s por 10 min gera
**1 notificação de problema + 1 de resolução**, e o alerta fica visível como
"estabilizando" entre as duas.

### Fase 2 — Classificação do problema e aviso de instabilidade `🔴 Não iniciado`

- [ ] Preencher `data.problem_kind` a partir do dataset do monitor
  (`datasets/monitor_result.rs`) e dos fatos de interface/VPN.
- [ ] Tratar `warning` (perda parcial de pacotes, DNS parcial) como problema
  para a janela de recuperação — hoje só `down`/`recovered` movem o alerta.
- [ ] Templates do catálogo (`alerts/catalog/templates.rs`) revisados:
  perda de pacotes e latência ganham `recovery_window_seconds` coerente;
  transições de interface ganham regra própria de flap.
- [ ] Formatter de notificação com o tipo e os detalhes do episódio
  (perda %, latência, contagem de quedas).

### Fase 3 — Detecção de flapping (supressão inteligente) `🔴 Não iniciado`

Para alvos **cronicamente** instáveis, onde nem a Fase 1 basta.

- [ ] Contador deslizante de transições de estado por alvo (últimas N
  checagens — cabe em memória + persistência leve, ou derivado de
  `monitor_results`).
- [ ] Estado `flapping` no evento (ou score estilo BGP damping): acima do
  limiar, suprime notificações recorrentes e emite um único aviso "alvo
  oscilando"; abaixo do limiar de retorno, avisa que estabilizou.
- [ ] Limiares configuráveis por regra (ex.: >5 transições em 15 min).
- [ ] Widget/indicador de instabilidade no frontend (página do monitor e
  dashboard): "este link oscilou 12x nas últimas 24h".

### Fase 4 — Higiene de notificações `🔴 Não iniciado`

- [ ] Cooldown por (regra, scope_key): intervalo mínimo entre notificações
  mesmo quando o evento reabre.
- [ ] Agrupamento/digest: janela de N minutos consolidando alertas
  correlatos numa mensagem só ("8 alertas no site X").
- [ ] Inibição por dependência: dispositivo atrás de um link/roteador que
  caiu não gera enxurrada — o alerta do pai suprime os filhos (requer
  topologia; ver §5 sobre dependências).
- [ ] Purga de `alert_events` no `data_pruner` (hoje a tabela cresce sem
  teto — `scheduler_run.rs:292-307` cobre outras tabelas, não esta).

### Fase 5 — Robustez do motor (dívidas que atrapalham as fases acima) `🔴 Não iniciado`

- [ ] Histerese de disparo (`pending_since`, `manager.rs:44-47`) persistida
  ou reconstruída a partir de `monitor_results` no boot — hoje reiniciar o
  processo zera tolerâncias.
- [ ] Honrar `monitors.retry_count` na execução (gravado, nunca lido): N
  falhas consecutivas antes de declarar `down` já elimina metade dos
  falsos positivos que viram flap.
- [ ] Avaliar `keep_firing_for` estilo Prometheus como alternativa mais
  simples à máquina de 3 estados, **antes** de implementar a Fase 1, se a
  equipe preferir menos estados.

## 5. Outros problemas prováveis neste tipo de sistema (avaliação)

Levantamento de o que mais costuma morder em monitores de rede, para
priorização futura — não necessariamente neste roadmap:

1. **Tempestade de alertas em cascata** — um roteador central cai e 200
   dispositivos atrás dele alertam juntos. Solução: inibição por
   topologia/dependência (Fase 4) ou correlação temporal ("tudo caiu no
   mesmo segundo → provavelmente é o pai").
2. **Falso down por sobrecarga do próprio monitor** — rajada de monitores
   vencidos rodando inline (backpressure, já reconhecido em
   `arquitetura.md:448-449`) pode inflar latências e gerar downs espúrios,
   que por sua vez viram flapping. A fila de execução é pré-requisito de
   confiabilidade para tudo acima.
3. **Métricas degradadas sem down** — latência subindo devagar, perda de
   2-3% constante. Threshold fixo não pega; solução futura: baseline
   móvel (média/desvio das últimas 24h) e alerta por desvio, não por valor
   absoluto.
4. **Silêncio de manutenção** — hoje existe silence por alerta; falta
   **janela de manutenção por alvo/site** (não checar ou não notificar
   durante manutenção programada), que é onde nascem os flaps "falsos" de
   reboot agendado.
5. **Relógio da histerese vs. intervalo de checagem** — com
   `interval_seconds` grande e janela curta, a janela pode vencer entre duas
   checagens. A máquina de estados precisa raciocinar em **número de
   checagens consecutivas** além de tempo, ou garantir janela ≥ 2× intervalo.
6. **Alerta de probe offline mascarando down real** — o fallback local
   (diretriz §6 do AGENTS.md) evita falso `unknown`, mas um probe flapping
   (agente caindo e voltando) é ele mesmo um alvo de detecção de flap.
7. **Histórico sem agregação** — `monitor_results` é append-only e cresce;
   responder "este link é estável?" em 24h/7d exige rollups (média horária),
   que também alimentariam o score de instabilidade da Fase 3.

## 6. Fora de escopo (declarado)

- ML/detecção de anomalia estatística pesada — o baseline móvel do item 5.3
  cobre 80% do valor com 5% da complexidade.
- Incidentes como entidade separada de `alert_events` — a máquina de estados
  da Fase 1 já entrega o valor; separar entidade é refactor que pode vir
  depois se a linha do tempo de incidentes ficar rica.
- Escalonamento de severidade por tempo aberto (on-call) — outro roadmap.

---

> **Nota de contexto**: o `docs/roadmap.md` referenciado pelo `AGENTS.md` não
> existe nesta branch (perdido na limpeza pós-migração). Este documento é um
> roadmap **temático**; se o roadmap geral for recriado, os itens da Fase 1
> em diante devem ser linkados lá.
