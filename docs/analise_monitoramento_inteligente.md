# Análise Profunda — Alertas Inteligentes: Arquitetura (SOLID) e UX

> Documento irmão de [roadmap_monitoramento_inteligente.md](roadmap_monitoramento_inteligente.md).
> O roadmap diz **o quê** e **em que ordem**; esta análise diz **como** —
> o design arquitetural e de experiência para que a implementação saia
> certa na primeira vez. Nada aqui foi implementado.

## 1. Diagnóstico SOLID do motor atual

O motor de alertas (`backend/src/services/alerts/`) tem uma espinha dorsal
excelente e um centro frágil. Vale dizer os dois com precisão.

### 1.1 O que já está certo (e não se deve tocar)

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

| # | Fragilidade | Onde | Por que importa agora |
|---|---|---|---|
| F1 | **God function procedural**: `manager.rs` acumula seleção de regras, avaliação, histerese, dedup, persistência, notificação, SSE e recuperação | `manager.rs:54-245` | Cada fase do roadmap adiciona mais uma responsabilidade nessa função. Sem extrair, a Fase 3 torna o arquivo imantenível. |
| F2 | **Status stringly-typed**: status são consts `&str` | `contracts.rs:53-58` | Adicionar `recovering`/`flapping` é cirurgia manual em N pontos sem auxílio do compilador — nenhum `match` exaustivo aponta o que faltou. |
| F3 | **Relógio real não injetável** + histerese em `static` | `manager.rs:44-47,150` | O caso "disparou após a tolerância" é **intestável** hoje; a janela de recuperação (Fase 1) nasceria com o mesmo defeito. |
| F4 | **Dedup read-then-insert sem índice único nem transação** | `manager.rs:167-201`; migration `m20260810_000016:60-67` | Correto hoje por circunstância (scheduler único + guard por monitor), não por construção. Escopos de interface/VPN não têm guard algum. |
| F5 | **Notificação fora do outbox**: crash entre INSERT e `notify` = notificação perdida sem rastro | `manager.rs:200-242` | Com cooldown e digest (Fase 4), a entrega vira assíncrona de qualquer forma — o outbox é o mecanismo natural. |
| F6 | **`NotificationService` construído no ponto de uso** (env relido a cada alerta) | `manager.rs:204`, `recovery.rs:49` | Sem injeção, um `NotificationPolicy` com cooldown não tem onde se pendurar sem editar o miolo. |
| F7 | **`pending_since` não é limpo após disparo** — leak lento por (regra × alvo) | `manager.rs:137-156` | Um sweep barato resolve; agrava com mais estados temporais. |
| F8 | **`is_silenced` não é chamado por produção** — silêncio não suprime nem a notificação de resolução | `silence.rs:26` (só testes); `recovery.rs:67-81` | Um alerta silenciado que resolve **notifica** ✅. Com recaídas frequentes, isso vira ruído novo. |
| F9 | **Query duplicada**: `evaluate_monitor_result` re-busca o `device` que `result_processor` já carregou | `manager.rs:95-102` vs `result_processor.rs:75` | Menor, mas trivial de corrigir passando o device no contexto. |

### 1.3 Princípio orientador

As correções F1–F6 não são "refactor por estética": cada uma é **o ponto de
extensão que uma fase do roadmap precisa**. A ordem econômica é pagar a
dívida na hora em que ela bloqueia a feature — não antes, não depois.

## 2. Arquitetura-alvo

### 2.1 Extrair a máquina de estados como domínio puro

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

### 2.2 Status como enum, fronteira como string

Trocar as consts de `contracts.rs:53-58` por `enum AlertStatus` com
`serde`/`sea-orm` mapeando para a coluna string existente (migration não
muda). O compilador passa a **obrigar** o tratamento de `Recovering` e
`Flapping` em cada `match` — elimina a classe de bug "esqueceu um ponto em
silêncio" (F2) e espelha o que o frontend precisa fazer em
`statusLabel`/`STATUS_TONES` (§3.2).

### 2.3 Política de notificação como porta (DIP) + outbox

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

### 2.4 Dedup por construção, não por circunstância

- Índice **único parcial** `(alert_rule_id, scope_key) WHERE status IN
  (abertos)` — ou, se o SQLite complicar o parcial, coluna `open_key`
  preenchida só enquanto aberto, com índice único sobre ela.
- INSERT com tratamento de conflito vira a dedup; o read-then-insert some.
  Interface/VPN deixam de depender de boa vontade de scheduling (F4).

### 2.5 Persistência da histerese

A Fase 1 já manda persistir o estado de recuperação. Aproveitar o mesmo
movimento para o disparo: `pending_since` pode ser reconstruído no boot a
partir de `monitor_results` (o fato bruto está no banco) ou migrado para
coluna. Qualquer das duas mata F7 e a fragilidade de testes com estado
global (que hoje dependem de ids altos + `forget_pending` manual).

## 3. UX profunda

A pergunta de design não é "onde mostrar o novo chip" — é **como o usuário
responde a três perguntas em 5 segundos**: *está quebrado agora? está
melhorando de verdade? preciso agir?*

### 3.1 Modelo mental: do evento ao episódio

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

### 3.2 Linguagem visual dos novos estados

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

### 3.3 Tempo honesto

- As telas de alerta usam só data absoluta, mas **`formatRelativeTime`
  existe** (`formatters.ts:128-137`) e ninguém usa aqui. "Último problema
  há 4 min" comunica mais que "15/08/2026 14:27:33". Adotar relativo +
  tooltip com absoluto.
- Falta um helper de duração decorrida ("estável há 5 min") — derivação
  trivial do relativo sobre `last_problem_at`.
- **Eliminar dados sintéticos**: `BinaryStatusWidget.vue:180-188` gera 25
  amostras falsas quando não há resultados — no widget que fala de
  flapping, o lugar onde confiança mais importa. Empty state honesto.

### 3.4 Formulário de regras e preview em linguagem natural

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

### 3.5 Pipeline SSE e notificações PWA

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

### 3.6 Lacunas adjacentes que valem entrar na mesma etapa de UI

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

### 3.7 Acessibilidade e convenções

- Chips de estado com cor + ícone + texto (nunca só cor) — daltonismo.
- Botões-ícone mantêm `v-tooltip`; adicionar `aria-label` nos novos.
- Mobile vem de graça com `ResponsiveDataTable`, mas a expansão de
  episódio precisa funcionar no slot `#mobile-item` também.

## 4. Riscos e decisões em aberto

1. **Máquina de 3 estados vs `keep_firing_for`** (Prometheus): a máquina é
   mais expressiva (estado visível "estabilizando" é feature de UX, não só
   interna); o `keep_firing_for` é mais simples mas não dá ao usuário a
   noção de progresso. Recomendação: máquina de estados — o valor de UX da
   barra de estabilização justifica o estado a mais.
2. **Janela vs contagem de checagens**: raciocinar só em tempo pode vencer
   entre duas checagens se `interval_seconds` for grande. Regra prática a
   validar na implementação: exigir **também** ≥1 checagem ok dentro da
   janela, ou janela mínima = 2× intervalo (catalog defaults já podem
   refletir isso).
3. **Estado `flapping` persistido vs derivado**: derivar de
   `monitor_results` evita estado novo, mas consulta a cada avaliação;
   persistir um score com decaimento é O(1) e testável. Decidir na Fase 3.
4. **Outbox de notificações muda a semântica de entrega** (imediata →
   quase-imediata). Aceitável: segundos de atraso compram "nunca perde".
5. **Enum de status toca serialização**: frontend union type
   (`alerts.ts:64`) precisa estender junto — contrato único, dois lados,
   mesma entrega.

## 5. Impacto no roadmap (ajustes, sem mudar as fases)

- **Fase 1** ganha três pré-requisitos arquiteturais pagos no boleto da
  própria fase: máquina de estados pura + Clock (§2.1), enum de status
  (§2.2), tipar `data` no frontend (§3.1). Sem eles, a fase nasce com os
  defeitos F2/F3.
- **Fase 4** passa a incluir o port `NotificationPolicy` + outbox de
  notificações (§2.3) **antes** do cooldown/digest — é a fundação de ambos,
  e resolve F5/F6/F8 na mesma tacada.
- **Fase 5** absorve F4 (dedup por construção), F7 (limpeza do
  `pending_since`) e F9 (query duplicada) como itens explícitos.
- **Nova nota de UX** transversal: toda fase que toca a Central entrega
  também o registro no `STATUS_TONES`/apresentação central — nunca um
  componente local com cor hardcoded.
- **Princípio de execução (decisão registrada)**: implementação **sem
  legado** — pré-requisitos arquiteturais (§2.1, §2.2, tipagem de `data`)
  são entregues junto com a Fase 1, código morto é removido na mesma
  entrega, e validação completa é critério de aceite, mesmo que leve mais
  tempo. Ver nota equivalente no cabeçalho do roadmap.
