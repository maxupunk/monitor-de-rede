# Análise Profunda — Alertas Inteligentes: Arquitetura (SOLID) e UX

> Documento irmão de [roadmap_monitoramento_inteligente.md](roadmap_monitoramento_inteligente.md).
> O roadmap diz **o quê** e **em que ordem**; esta análise diz **como** —
> o design arquitetural e de experiência para que a implementação saia
> certa na primeira vez.
>
> **Estado (2026-08-15): as Fases 1 a 5 do roadmap estão entregues.** O texto
> de análise abaixo é preservado como **registro do desenho** — é ele que
> explica *por quê* o código ficou como ficou. O que mudou é que cada item
> agora carrega o desfecho, e os desfechos incluem os desvios: onde a
> implementação divergiu do desenho, o motivo está anotado no lugar.
>
> Legenda: `✅ feito` · `🟡 parcial` · `🔴 em aberto` · `⚪ não revisado`.
>
> **O que sobrou** — três itens desta análise **não** foram entregues e não
> estão em fase nenhuma do roadmap: **F4** (dedup por construção), **F9**
> (query duplicada do device) e a maior parte de **§3.6** (filtros do
> histórico, `silencedUntil` na tela). Ver §5.

## 1. Diagnóstico SOLID do motor atual

O motor de alertas (`backend/src/services/alerts/`) tem uma espinha dorsal
excelente e um centro frágil. Vale dizer os dois com precisão.

### 1.1 O que já está certo (e não se deve tocar) `✅ preservado`

> **As cinco fases passaram sem quebrar nenhum dos quatro.** O ponto que mais
> se pagou foi o Dataset/Facts: a Fase 2 acrescentou a classificação de
> problema lendo os datasets existentes, sem que produtor nenhum soubesse
> disso. E a disciplina "acessório nunca derruba essencial" ficou mais
> explícita na Fase 4 — enfileirar notificação também falha com `warn!`, nunca
> desfazendo o alerta já gravado.

- **Dataset/Facts pattern**: produtores traduzem observação → fatos
  (`datasets/monitor_result.rs`, `interface_state.rs`, `vpn_peer.rs`) sem
  conhecer `alert_events`, notificação ou severidade
  (`contracts.rs:1-6`). Adicionar fonte nova de fatos é genuinamente OCP —
  arquivo novo em `datasets/`, zero edição no motor. **É o padrão a
  preservar acima de todos.**
- **Núcleo puro e testado**: `evaluator.rs`, `silence.rs`, catálogo e
  datasets são funções puras com testes inline exaustivos.
- **Acessório nunca derruba essencial**: falha de notificação/SSE é `warn!`
  e segue (manager.rs:240) — disciplina correta.
- **Outbox SSE funcional** (`events/bus.rs:81-91` + relay multi-processo):
  infraestrutura pronta para estender a notificações.

### 1.2 Fragilidades que este roadmap vai esbarrar

As linhas de origem citadas são as de **agosto/2026, antes da Fase 1** — o
código andou muito desde então e elas servem só para localizar o defeito no
histórico.

| # | Fragilidade | Onde (original) | Por que importava | Situação |
|---|---|---|---|---|
| F1 | **God function procedural**: `manager.rs` acumula seleção de regras, avaliação, histerese, dedup, persistência, notificação, SSE e recuperação | `manager.rs:54-245` | Cada fase do roadmap adiciona mais uma responsabilidade nessa função. Sem extrair, a Fase 3 torna o arquivo imantenível. | ✅ **Fase 1**. `manager.rs` virou orquestrador; a decisão saiu para `state_machine.rs`, a escrita do episódio para `episode.rs`, os payloads SSE para `feed.rs`. |
| F2 | **Status stringly-typed**: status são consts `&str` | `contracts.rs:53-58` | Adicionar `recovering`/`flapping` é cirurgia manual em N pontos sem auxílio do compilador — nenhum `match` exaustivo aponta o que faltou. | ✅ **Fase 1**. `enum AlertStatus`; a Fase 3 acrescentou `Flapping` e o compilador apontou cada ponto. |
| F3 | **Relógio real não injetável** + histerese em `static` | `manager.rs:44-47,150` | O caso "disparou após a tolerância" é **intestável** hoje; a janela de recuperação (Fase 1) nasceria com o mesmo defeito. | ✅ **Fases 1 e 5**. `state_machine::decide` recebe `now`; a Fase 5 fez o mesmo com a histerese de disparo (`hysteresis.rs`), que era o pedaço que faltava. |
| F4 | **Dedup read-then-insert sem índice único nem transação** | `manager.rs:167-201`; migration `m20260810_000016:60-67` | Correto hoje por circunstância (scheduler único + guard por monitor), não por construção. Escopos de interface/VPN não têm guard algum. | 🔴 **Em aberto**. Continua read-then-insert; nenhuma fase o cobriu. Ver §5. |
| F5 | **Notificação fora do outbox**: crash entre INSERT e `notify` = notificação perdida sem rastro | `manager.rs:200-242` | Com cooldown e digest (Fase 4), a entrega vira assíncrona de qualquer forma — o outbox é o mecanismo natural. | ✅ **Fase 4**. `notification_outbox` + despacho no ciclo do scheduler: entrega ao menos uma vez. |
| F6 | **`NotificationService` construído no ponto de uso** (env relido a cada alerta) | `manager.rs:204`, `recovery.rs:49` | Sem injeção, um `NotificationPolicy` com cooldown não tem onde se pendurar sem editar o miolo. | ✅ **Fase 4**. Construído uma vez por passagem do despachante; o miolo só enfileira. |
| F7 | **`pending_since` não é limpo após disparo** — leak lento por (regra × alvo) | `manager.rs:137-156` | Um sweep barato resolve; agrava com mais estados temporais. | ✅ **Fase 5**. `hysteresis::sweep` roda junto da purga de dados e descarta o que ninguém mais alimenta. |
| F8 | **`is_silenced` não é chamado por produção** — silêncio não suprime nem a notificação de resolução | `silence.rs:26` (só testes); `recovery.rs:67-81` | Um alerta silenciado que resolve **notifica** ✅. Com recaídas frequentes, isso vira ruído novo. | ✅ **Fase 4**. O silêncio é lido pelo **prazo**, não pelo status (um alerta silenciado que entra em `recovering` perdia o rótulo mas não o pedido do operador), e a política o transforma em `Suppress(Silenced)`. |
| F9 | **Query duplicada**: `evaluate_monitor_result` re-busca o `device` que `result_processor` já carregou | `manager.rs:95-102` vs `result_processor.rs:75` | Menor, mas trivial de corrigir passando o device no contexto. | 🔴 **Em aberto**. Ver §5. |

### 1.3 Princípio orientador

As correções F1–F6 não são "refactor por estética": cada uma é **o ponto de
extensão que uma fase do roadmap precisa**. A ordem econômica é pagar a
dívida na hora em que ela bloqueia a feature — não antes, não depois.

> **O princípio se confirmou, e explica as duas sobras.** F1, F2, F3, F5, F6 e
> F8 foram pagas exatamente quando bloquearam uma fase, e cada uma saiu junto
> da feature que dependia dela. F4 e F9 sobraram justamente porque **nunca
> bloquearam nada**: a dedup segue correta por circunstância e a query extra
> custa um `SELECT` por resultado de monitor. É a consequência esperada da
> regra — o que ela não previu foi ninguém registrar a sobra.

## 2. Arquitetura-alvo

### 2.1 Extrair a máquina de estados como domínio puro `✅ feito (Fase 1)`

> **Entregue em `services/alerts/state_machine.rs`, com dois desvios.**
>
> - **O Clock não virou trait.** `decide` recebe `now: DateTime<Utc>` dentro do
>   `EpisodeInput`. Uma trait `Clock` só se pagaria se houvesse mais de uma
>   implementação de produção; um parâmetro entrega a mesma testabilidade sem
>   o indireto.
> - **`EnterFlapping`/`ExitFlapping` viraram `StartFlapping` + saída pelo
>   `Resolve`.** A Fase 3 descobriu que não existe "sair do flapping para
>   continuar aberto": ou o alvo estabiliza (e aí a saída é a resolução) ou
>   segue oscilando. Uma transição a menos, sem perder caso nenhum.
> - **A máquina recebe `EpisodePolicy`, não a regra inteira.** O efeito da
>   §3.4 do roadmap é o mesmo — todo parâmetro vem da regra —, mas o domínio
>   puro não fica acoplado ao modelo do `sea-orm`.

A decisão central — *"dado o evento aberto, o resultado novo e o relógio,
qual a transição?"* — deve sair de `manager.rs` para um módulo puro:

```
alert_events + novo fato + Clock ──▶ AlertStateMachine (puro, sem I/O)
                                          │
                                          ▼
                              Transition::{ None
                                          | EnterRecovering{..}
                                          | Relapse{recurrence}
                                          | Resolve{episode_summary}
                                          | EnterFlapping / ExitFlapping }
```

- **Puro = testável**: todos os cenários do roadmap (recaída reinicia
  janela, janela vencida resolve, flap suprime) viram testes unitários de
  tabela, sem banco e sem `sleep` — o que hoje é impossível (F3).
- **Clock injetável** (`&dyn Fn() -> DateTime<Utc>` ou trait `Clock`):
  produção usa `Utc::now`, teste usa relógio fixo. Custo: uma linha por
  chamador.
- `manager.rs` vira orquestrador fino: carrega regras/evento aberto, chama
  a máquina, persiste a transição, delega efeitos. SRP restaurado sem
  reescrever nada que funciona.
- A máquina recebe a **regra inteira** (que carrega `duration_seconds`,
  `recovery_window_seconds`, limiares de flap) — honrando a decisão
  §3.4 do roadmap: toda tolerância é parâmetro de regra.

### 2.2 Status como enum, fronteira como string `✅ feito (Fase 1)`

> Entregue como desenhado. A prova veio na Fase 3: acrescentar `Flapping` ao
> enum quebrou a compilação em cada ponto de decisão que faltava tratar — que
> era exatamente o efeito pretendido.

Trocar as consts de `contracts.rs:53-58` por `enum AlertStatus` com
`serde`/`sea-orm` mapeando para a coluna string existente (migration não
muda). O compilador passa a **obrigar** o tratamento de `Recovering` e
`Flapping` em cada `match` — elimina a classe de bug "esqueceu um ponto em
silêncio" (F2) e espelha o que o frontend precisa fazer em
`statusLabel`/`STATUS_TONES` (§3.2).

### 2.3 Política de notificação como porta (DIP) + outbox `✅ feito (Fase 4)`

> **Entregue em `notifications/policy.rs` (decisão pura) + `notifications/
> outbox.rs` (diário e despacho), com um desvio de forma.**
>
> **Não virou um port injetado via `shared_store`.** A inversão de dependência
> existia para um problema que sumiu: a política acabou sendo uma **função
> pura** e o outbox uma **tabela**. Não há colaborador a substituir em teste —
> `policy::decide` é exercitada em tabela e o outbox, contra o banco de teste.
> Injetar aqui seria indireto sem ganho. O objetivo do DIP — "o miolo não
> constrói nem conhece o canal" — foi atingido por outro caminho: `manager` e
> `recovery` só chamam `outbox::enqueue`.
>
> **Um desdobramento que a análise não previu**: a linha suprimida guarda o
> **motivo** (`cooldown`/`inhibited`/`unannounced`/`silenced`), o que fez o
> diário responder "por que não fui avisado?" — pergunta que não tinha resposta
> em lugar nenhum do sistema.
>
> **Onde a inibição entrou**: a análise a listava só como item de roadmap.
> Na prática ela **não** cabe na política pura, porque depende de o pai já ter
> sido detectado — e o filho quase sempre é visto antes. Virou um julgamento na
> hora da entrega (`alerts/inhibition.rs`), com carência de 120 s na fila.

Hoje: `trigger_alert` → `NotificationService::with_default_channels()` →
envio imediato. Alvo:

- `manager`/`recovery` dependem de um port `NotificationPolicy` injetado
  (via `shared_store`, padrão já usado em `process_deps.rs:28-42`), não
  constroem nada.
- A política concreta decide: **enviar agora, suprimir (cooldown/flapping/
  silenciado), ou enfileirar no digest**. Regras de anti-fadiga viram dado
  da política, não `if` espalhado.
- A decisão "enviar" grava na tabela-outbox (estender `event_outbox` ou
  `notification_outbox`) e um relay entrega — **pelo menos uma vez**,
  sobrevivendo a crash (F5) e habilitando o digest da Fase 4 de graça.
- Efeito colateral que resolve F8: a política consulta `is_silenced` de
  verdade, e o silenciado para de receber até o ✅ da resolução.

### 2.4 Dedup por construção, não por circunstância `🔴 em aberto`

> **Não implementado.** `manager::trigger_alert` ainda procura o evento aberto
> antes de inserir. Nada quebrou porque as circunstâncias que o tornam correto
> continuam de pé — o scheduler é um processo só (ADR 007) e há guard por
> monitor —, mas a fragilidade original permanece: os escopos de interface e
> de túnel não têm guard algum, e uma instalação que separe o `scheduler_loop`
> em outro processo passa a depender de sorte.
>
> **O que a Fase 4 mudou no cálculo**: com `notification_outbox` no caminho, um
> evento duplicado passaria a gerar **duas** notificações em vez de uma linha
> repetida na Central. O custo do defeito subiu; a probabilidade, não.

- Índice **único parcial** `(alert_rule_id, scope_key) WHERE status IN
  (abertos)` — ou, se o SQLite complicar o parcial, coluna `open_key`
  preenchida só enquanto aberto, com índice único sobre ela.
- INSERT com tratamento de conflito vira a dedup; o read-then-insert some.
  Interface/VPN deixam de depender de boa vontade de scheduling (F4).

### 2.5 Persistência da histerese `✅ feito (Fase 5), pelo caminho oposto`

> **Entregue em `services/alerts/hysteresis.rs` — mas a opção escolhida foi a
> reconstrução, e a persistência foi recusada com motivo.**
>
> O comentário que segurava `pending_since` em memória estava certo:
> `duration_seconds` mede *continuidade observada*, e gravar o carimbo faria um
> restart herdar uma tolerância que ninguém acompanhou. Persistir mataria F7 e
> criaria um defeito pior — silencioso, e exatamente na direção que um sistema
> de monitoramento não pode errar.
>
> A saída foi **ler `monitor_results`**, que é onde a observação de fato está.
> Com uma fronteira explícita: a reconstrução só afirma o que a linha gravada
> prova (status, duração, latência e o `data` do checker — não as métricas
> soltas). Regra de `packetLoss` não acha o campo, a avaliação da linha mais
> recente já dá `false`, e a contagem começa agora, como antes. A caminhada
> também rompe quando duas observações estão a mais de 3× `interval_seconds`
> uma da outra.
>
> A fragilidade de teste que a análise citou também caiu: os testes já não
> dependem de "ids altos + `forget_pending` manual" — `hysteresis` expõe
> `forget` como operação de domínio e os cenários de contagem viraram testes de
> tabela sobre o relógio injetado.

A Fase 1 já manda persistir o estado de recuperação. Aproveitar o mesmo
movimento para o disparo: `pending_since` pode ser reconstruído no boot a
partir de `monitor_results` (o fato bruto está no banco) ou migrado para
coluna. Qualquer das duas mata F7 e a fragilidade de testes com estado
global (que hoje dependem de ids altos + `forget_pending` manual).

## 3. UX profunda

A pergunta de design não é "onde mostrar o novo chip" — é **como o usuário
responde a três perguntas em 5 segundos**: *está quebrado agora? está
melhorando de verdade? preciso agir?*

### 3.1 Modelo mental: do evento ao episódio `🟡 parcial`

> - ✅ **`data` tipado no frontend** (`AlertEventData` em `stores/alerts.ts`),
>   que era o "pré-requisito nº 1": os metadados chegavam e eram descartados.
> - ✅ **Linha do episódio na Central**: `episodeInfo()` na `AlertsPage.vue`
>   monta "oscilando desde X · último problema há Y · N recaídas" para
>   `recovering` e `flapping`.
> - 🔴 **Expansão da linha com a timeline** e 🔴 **barra de progresso da
>   estabilização** ("estável há 3min12s de 5min necessários") **não foram
>   feitas**. O dado existe (`lastProblemAt` + `recoveryWindowSeconds` da
>   regra) e era a peça que respondia "está melhorando de verdade?" sem
>   interpretação — continua sendo a lacuna de UX mais cara deste conjunto.
> - 🔴 **Link alerta → regra** na linha expandida: não implementado.

Hoje cada disparo é um evento isolado; 20 oscilações = 20 linhas
indiferenciadas. O novo modelo mental é o **episódio**: um alerta com
linha do tempo própria (caiu → voltou → caiu → … → estável → resolvido).

- **Central de Alertas**: linha do alerta ganha expansão (padrão já usado
  em `MonitorDetailPage.vue:942-969` com `v-expand-transition`) revelando
  o episódio: tipo de problema, timeline de transições, contagem de
  recaídas, último problema, e **progresso da estabilização** — uma barra
  "estável há 3min12s de 5min necessários" responde "está melhorando de
  verdade?" sem interpretação.
- **Pré-requisito nº 1, custo quase zero**: o backend **já serializa**
  `data` (JSON) e `alert_rule` em `views/alerts.rs:101,109`, mas a
  interface `AlertEvent` do frontend (`stores/alerts.ts:58-74`) não declara
  `data` — os metadados chegam e são **descartados silenciosamente**.
  Tipar o campo habilita metade da UX deste roadmap.
- **Rastreabilidade alerta → regra**: a linha expandida linka para a regra
  que gerou o alerta (o nome já trafega!). É a materialização em UI da
  decisão §3.4 do roadmap: o usuário vê a situação e, no mesmo gesto,
  ajusta a tolerância.

### 3.2 Linguagem visual dos novos estados `🟡 parcial`

> - ✅ **`recovering` e `flapping` registrados no StatusTone central**
>   (`monitorPresentation.ts`) — o ponto único, como a análise pedia; a
>   `AlertsPage` consome via `statusColor`, e `statusLabel` devolve
>   "Estabilizando"/"Oscilando" em vez do valor cru.
> - 🟡 **A hierarquia de três cores não saiu**: os dois estados ficaram no
>   mesmo tom `warning`, então "estabilizando" e "cronicamente instável" têm a
>   mesma cor e só o texto os separa. O roxo sugerido para `flapping` continua
>   valendo — é o estado que pede investigar a causa, não o sintoma.
> - 🟡 **Dashboard**: ganhou o widget "Alvos Instáveis" (Fase 3), mas o
>   contador "Alertas Ativos / Requerem atenção" **continua somando tudo que
>   não é `resolved`**. O risco que a análise previu se materializou: agora que
>   o sistema retém alertas por mais tempo em `recovering`, esse número infla
>   sem que nada tenha piorado.

Ponto fraco atual: **todo status é um chip outlined sem cor própria**
(`AlertsPage.vue:99-103`) — só o texto diferencia. E o fallback de
`statusLabel` (`alertPresentation.ts:352-353`) exibiria "RECOVERING" cru.

- Registrar `recovering` e `flapping` no **StatusTone central**
  (`monitorPresentation.ts:174-248`) — o ponto único e correto; propaga
  para Central, detalhe do monitor, dashboard e devices de uma vez.
  Sugestão: `recovering` = tom `warning` com ícone de "curva se
  estabilizando"; `flapping` = tom `warning` mais forte/roxo com ícone de
  oscilação; `active` mantém severidade como cor dominante.
- Hierarquia de atenção explícita: **vermelho = agindo agora, âmbar =
  estava quebrado e está estabilizando, roxo = cronicamente instável
  (investigar a causa, não o sintoma)**.
- Dashboard: o contador "Alertas Ativos / Requerem atenção"
  (`DashboardPage.vue:248-251`) hoje mistura tudo que não é `resolved`
  (`stores/alerts.ts:88`). Separar visualmente **"em falha"** de
  **"estabilizando"** — senão o número infla e perde significado justo
  quando o sistema começa a reter alertas por mais tempo.

### 3.3 Tempo honesto `🟡 parcial`

> - ✅ **`formatRelativeTime` adotado** na Central, via `episodeInfo()`:
>   "último problema há 4 min" em vez do carimbo absoluto.
> - ✅ **Duração decorrida** existe do lado do backend, no resumo da
>   notificação de resolução ("estável há 5 min", `formatter.rs`).
> - 🔴 **Tooltip com o absoluto** ao lado do relativo: não feito.
> - 🔴 **Dados sintéticos no `BinaryStatusWidget`**: não verificado nesta
>   revisão e não endereçado por nenhuma fase.

- As telas de alerta usam só data absoluta, mas **`formatRelativeTime`
  existe** (`formatters.ts:128-137`) e ninguém usa aqui. "Último problema
  há 4 min" comunica mais que "15/08/2026 14:27:33". Adotar relativo +
  tooltip com absoluto.
- Falta um helper de duração decorrida ("estável há 5 min") — derivação
  trivial do relativo sobre `last_problem_at`.
- **Eliminar dados sintéticos**: `BinaryStatusWidget.vue:180-188` gera 25
  amostras falsas quando não há resultados — no widget que fala de
  flapping, o lugar onde confiança mais importa. Empty state honesto.

### 3.4 Formulário de regras e preview em linguagem natural `✅ feito`

> Entregue e **estendido além do desenho**. O `describeRule` acumulou cinco
> cláusulas ao longo das fases: tolerância (já existia), estabilização
> (`RECOVERY_WINDOWS`, Fase 1), oscilação (`FLAP_THRESHOLDS` + `FLAP_WINDOWS`,
> Fase 3) e, na Fase 4, intervalo entre notificações
> (`NOTIFICATION_COOLDOWNS`) e inibição por pai. O catálogo
> (`AlertRuleCatalogDialog.vue`) recebe todas.
>
> **A aposta se confirmou**: a frase-resumo é o que documenta a máquina de
> estados para quem não a conhece. Ela também virou o lugar onde os avisos de
> configuração incoerente aparecem — o formulário alerta quando o limiar de
> oscilação é ligado sem janela de estabilização, porque nesse arranjo a
> detecção nunca dispara.

O padrão já existe e é bom: `ALERT_DURATIONS` + `describeRule()` geram a
frase-resumo (`alertPresentation.ts:288-294,385-390`). Estender:

- `RECOVERY_WINDOWS` ("Resolver após 2/5/15/30 min estável") como
  `v-select` clonando o padrão de tolerância.
- `describeRule` ganha a terceira cláusula: *"…se persistir por 5 min, e
  resolve após 10 min sem recaída"*. **O usuário lê a frase e entende o
  comportamento da máquina de estados sem conhecer a máquina** — é a
  melhor documentação possível do recurso.
- Propagar ao catálogo (`AlertRuleCatalogDialog.vue:70` chama
  `describeRule` com 2 args hoje).

### 3.5 Pipeline SSE e notificações PWA `🟡 parcial`

> - ✅ **`alert:updated`** entregue na Fase 1 (`alerts/feed.rs` centraliza os
>   três payloads), e é ele que atualiza o contador de recaídas sem evento novo.
> - 🔴 **`lastRealtimeUpdateAt` continua órfão**: o flag é escrito pelo store
>   e nenhuma tela o consome como flash visual.
> - 🔴 **PWA**: `useNotifications.ts` ainda escuta **só `alert:triggered`**. A
>   recomendação da análise — notificar disparo **e resolução final**, nunca
>   recaída — está metade feita: as recaídas de fato não notificam (não geram
>   evento), mas "estabilizou de vez após 7 oscilações", que era a notificação
>   **valiosa**, não chega ao push.
>
> Vale registrar o que mudou de contexto: a Fase 4 pôs cooldown, agrupamento e
> inibição **nos canais externos** (Telegram/Discord/e-mail/webhook). O push
> PWA não passa por essa política — ele reage direto ao SSE. Ligar a resolução
> no push hoje entregaria um ✅ que os outros canais podem ter suprimido.

- Recaída não gera evento novo (decisão do roadmap) — mas a UI precisa
  atualizar o contador. Um SSE `alert:updated` (patch com `data` novo) ou
  estender o payload de um `alert:relapsed`; o `patchAlertEvent`
  (`alerts.ts:291-296`) já aceita `Partial`, falta só o tipo e o `case` no
  switch de `events.ts:140`. O flag `lastRealtimeUpdateAt`
  (`alerts.ts:85`) — hoje órfão — pode virar o flash visual "atualizou
  agora".
- PWA (`useNotifications.ts:90-119`): notificar **disparo** e
  **resolução final**; **não** notificar recaída nem entrada em
  recovering (push é o canal mais caro de atenção — a dedup do push deve
  ser ainda mais agressiva que a da tela). "Estabilizou de vez após 7
  oscilações" é uma notificação valiosa; 7 pushes de recaída são
  exatamente a fadiga que estamos eliminando.

### 3.6 Lacunas adjacentes que valem entrar na mesma etapa de UI `🔴 em aberto`

> **Nenhum dos quatro itens foi feito.** A aposta era "custo marginal baixo uma
> vez que se toca a Central" — e a Central foi tocada nas Fases 1, 3 e 4 sem
> que nenhum deles entrasse, porque cada fase entregou o que o roadmap pedia e
> parou aí.
>
> Dois ficaram **mais** caros de adiar do que eram:
>
> - **Filtros do histórico**: `controllers/alerts.rs::index` continua só
>   paginando. Com episódios agora vivendo muito mais tempo (janela de
>   estabilização) e o histórico guardado por 90 dias (purga da Fase 4), a
>   tabela cresceu em linhas *e* em duração de cada linha.
> - **`silencedUntil` nunca exibido**: o campo é serializado pelo backend e
>   ignorado pela tela. Depois da Fase 4 isso passou a esconder informação de
>   verdade — o silêncio agora suprime também a notificação de resolução, então
>   um alerta silenciado é um alerta do qual o operador **não vai ouvir falar**,
>   e a tela não diz isso em lugar nenhum.

Custo marginal baixo uma vez que se toca a Central:

- **Histórico sem filtros** (`AlertsPage.vue:384-441`): exige filtro
  server-side no `index` (`controllers/alerts.rs:218-238` só pagina) —
  por regra, severidade e período. Sem isso, episódios longos são
  impesquisáveis.
- **`silencedUntil`/`acknowledgedBy` já estão no tipo** (`alerts.ts:67-68`)
  e nunca são exibidos — renderizar é quase grátis.
- **Aba Resolvidos** com rótulo "na sessão atual" enganoso
  (`AlertsPage.vue:204`) — é a janela do array, não a sessão.
- Tema dark obrigatório com classes de tema claro coexistindo
  (`bg-grey-lighten-4` etc.) — inconsistência prévia; os componentes
  novos devem nascer no padrão do tema dark.

### 3.7 Acessibilidade e convenções `🟡 parcial`

> - ✅ **"Nunca só cor"** está garantido: o chip de status sempre traz o texto
>   ("Estabilizando", "Oscilando"), na tabela e no slot mobile.
> - 🔴 **O ícone não entrou.** Isso importa mais do que parecia no desenho:
>   como `recovering` e `flapping` acabaram com o mesmo tom (§3.2), o ícone
>   seria hoje o **único** diferenciador além da palavra.
> - ⚪ `aria-label` nos botões-ícone novos e expansão de episódio no
>   `#mobile-item`: não revisados nesta passagem — a expansão sequer existe
>   (§3.1).

- Chips de estado com cor + ícone + texto (nunca só cor) — daltonismo.
- Botões-ícone mantêm `v-tooltip`; adicionar `aria-label` nos novos.
- Mobile vem de graça com `ResponsiveDataTable`, mas a expansão de
  episódio precisa funcionar no slot `#mobile-item` também.

## 4. Riscos e decisões — todas resolvidas

As cinco foram decididas nas fases correspondentes. Ficam registradas com o
desfecho porque é ele que explica o código.

1. **Máquina de 3 estados vs `keep_firing_for`** (Prometheus): a máquina é
   mais expressiva (estado visível "estabilizando" é feature de UX, não só
   interna); o `keep_firing_for` é mais simples mas não dá ao usuário a
   noção de progresso. Recomendação: máquina de estados — o valor de UX da
   barra de estabilização justifica o estado a mais.
   > **✅ Decidido: máquina de estados** (Fase 1; item fechado na Fase 5). A
   > Fase 3 confirmou por um motivo que a análise não tinha: `flapping` só
   > existe porque havia um estado onde pendurá-lo — com `keep_firing_for` não
   > haveria onde. **Ressalva honesta**: o argumento decisivo era a barra de
   > progresso da estabilização, e ela é justamente o que não foi construído
   > (§3.1). A escolha se sustenta pelo resto; a justificativa original, ainda
   > não inteira.
2. **Janela vs contagem de checagens**: raciocinar só em tempo pode vencer
   entre duas checagens se `interval_seconds` for grande. Regra prática a
   validar na implementação: exigir **também** ≥1 checagem ok dentro da
   janela, ou janela mínima = 2× intervalo (catalog defaults já podem
   refletir isso).
   > **✅ Decidido pela primeira opção** (Fase 1): a saída de `recovering`
   > exige uma **nova checagem ok depois** de a janela vencer — quem acabou de
   > entrar no estado segue sob observação mesmo com o último problema antigo.
   > A Fase 5 aplicou a mesma exigência à histerese de disparo: gap maior que
   > 3× o intervalo rompe a continuidade, no histórico e na memória.
3. **Estado `flapping` persistido vs derivado**: derivar de
   `monitor_results` evita estado novo, mas consulta a cada avaliação;
   persistir um score com decaimento é O(1) e testável. Decidir na Fase 3.
   > **✅ Decidido: persistido, mas sem score** (Fase 3). A contagem é uma
   > **lista deslizante de carimbos** em `data.problemTimeline`, medida sobre o
   > **episódio** e não sobre `monitor_results` — o episódio já atravessa a
   > oscilação desde a Fase 1 e já vale para monitor, interface e túnel, que é
   > mais do que `monitor_results` cobriria. O score com decaimento estilo BGP
   > foi recusado: os carimbos envelhecem sozinhos para fora da janela e
   > entregam a mesma histerese com um parâmetro a menos para o usuário
   > entender.
4. **Outbox de notificações muda a semântica de entrega** (imediata →
   quase-imediata). Aceitável: segundos de atraso compram "nunca perde".
   > **✅ Aceito e real** (Fase 4). O atraso ficou em até um ciclo do scheduler
   > (~5 s) para a entrega, mais o agrupamento quando ligado. A Fase 4 comprou
   > mais do que "nunca perde": a mesma tabela sustenta cooldown, digest e a
   > auditoria de supressão.
5. **Enum de status toca serialização**: frontend union type
   (`alerts.ts:64`) precisa estender junto — contrato único, dois lados,
   mesma entrega.
   > **✅ Respeitado** nas Fases 1 e 3: `recovering` e `flapping` entraram no
   > union type do `AlertEvent` na mesma entrega do enum do backend.

## 5. Impacto no roadmap — o que foi cumprido e o que sobrou

- **Fase 1** ganha três pré-requisitos arquiteturais pagos no boleto da
  própria fase: máquina de estados pura + Clock (§2.1), enum de status
  (§2.2), tipar `data` no frontend (§3.1). Sem eles, a fase nasce com os
  defeitos F2/F3.
  > **✅ Cumprido.** Os três saíram na Fase 1.
- **Fase 4** passa a incluir o port `NotificationPolicy` + outbox de
  notificações (§2.3) **antes** do cooldown/digest — é a fundação de ambos,
  e resolve F5/F6/F8 na mesma tacada.
  > **✅ Cumprido**, com o port virando função pura + tabela (ver §2.3). F5,
  > F6 e F8 caíram junto, como previsto.
- **Fase 5** absorve F4 (dedup por construção), F7 (limpeza do
  `pending_since`) e F9 (query duplicada) como itens explícitos.
  > **🟡 Cumprido pela metade — e esta é a divergência principal entre a
  > análise e o que foi feito.** A Fase 5 do roadmap nunca listou F4 e F9 na
  > sua checklist; ela nasceu com três itens próprios (histerese persistida,
  > `retry_count`, avaliar `keep_firing_for`) e foi fechada com eles. **Só F7
  > foi entregue.** F4 e F9 seguem em aberto e hoje não pertencem a fase
  > nenhuma.
- **Nova nota de UX** transversal: toda fase que toca a Central entrega
  também o registro no `STATUS_TONES`/apresentação central — nunca um
  componente local com cor hardcoded.
  > **✅ Respeitado** nas Fases 1, 3 e 4 — mas ver §3.2: o registro foi feito
  > no ponto certo e a *distinção visual* entre os dois estados não.
- **Princípio de execução (decisão registrada)**: implementação **sem
  legado** — pré-requisitos arquiteturais (§2.1, §2.2, tipagem de `data`)
  são entregues junto com a Fase 1, código morto é removido na mesma
  entrega, e validação completa é critério de aceite, mesmo que leve mais
  tempo. Ver nota equivalente no cabeçalho do roadmap.
  > **✅ Respeitado nas cinco fases.**

### 5.1 O que ficou sem dono

Nada disto está em fase alguma do roadmap. Em ordem de risco, não de custo:

| Item | Onde | Por que ainda importa |
|---|---|---|
| **F4** — dedup por construção (§2.4) | `manager::trigger_alert` | Com o outbox no caminho, um evento duplicado passou a custar **duas notificações**, não uma linha repetida. |
| **§3.6** — `silencedUntil` na tela | `AlertsPage.vue` | Depois da Fase 4 o silêncio suprime também o ✅. Um alerta silenciado é um alerta do qual não se ouvirá falar, e a tela não avisa. |
| **§3.1** — barra de progresso da estabilização | `AlertsPage.vue` | Era o argumento que decidiu a máquina de estados contra o `keep_firing_for` (§4.1). |
| **§3.2** — separar "em falha" de "estabilizando" no dashboard | `DashboardPage.vue` | O contador infla agora que os alertas ficam abertos por mais tempo. |
| **§3.6** — filtros do histórico | `controllers/alerts.rs::index` | Episódios mais longos + retenção de 90 dias = histórico maior em linhas e em duração. |
| **§3.5** — resolução final no push PWA | `useNotifications.ts` | Só ligar depois de decidir se o push respeita a política da Fase 4; hoje ele desviaria dela. |
| **F9** — query duplicada do device | `manager.rs` vs `result_processor.rs` | Um `SELECT` por resultado de monitor. Barato de corrigir, barato de conviver. |
