# ADR 025 — Vários processos da central: líder por trava consultiva e eventos por `LISTEN`/`NOTIFY`

- **Status:** aceito e implementado — 2026-11-25.
- **Emenda:** [ADR 007](007-scheduler-processo-unico.md) (o ciclo em processo
  longevo continua; deixa de ser "um processo só por construção").

## Contexto

O [ADR 007](007-scheduler-processo-unico.md) pôs o ciclo do scheduler num
processo longevo e o relay de eventos no servidor. Duas suposições ficaram
implícitas, e as duas quebram quando há mais de um processo sobre o mesmo
banco (o `scheduler_loop` dedicado, ou duas instâncias da API):

1. **O relay por cursor perdia evento no PostgreSQL.** Ele lia
   `event_outbox.id > último_id` a cada 5 s. No PostgreSQL o `id` é reservado
   no `INSERT` e fica visível no `COMMIT`: uma transação que reservou o 41 e
   confirmou depois que outra confirmou o 42 era pulada para sempre.
2. **O snapshot não atravessava processos.** `publish_snapshot` (o estado
   atual de VPN, Wi-Fi, agentes, deploys) guarda o evento num cache por
   `(tipo, chave)` que a aba recebe logo depois de `stream:connected`. Esse
   cache é do processo: a aba que conectava noutro processo ficava sem o
   estado até a próxima mudança.
3. **As rotinas de fundo assumiam processo único.** Backups, tarefas
   agendadas, despachante de deploys, IA proativa e as recuperações de boot
   ("o que estava em `running` morreu comigo") rodavam em todo processo que
   subisse: dois processos executariam o mesmo backup, e o segundo a subir
   marcaria como interrompido o que o primeiro estava rodando.

A instalação de produção e a de desenvolvimento são PostgreSQL
([ADR 020](020-postgresql-padrao-do-container.md)); a suíte de testes continua
em SQLite, num processo só.

## Decisão

### A escolha do dialeto é feita uma vez

`netmonitor_core::cluster::Coordination::for_database` decide no
`after_context` (todo processo, inclusive `task`):

| | SQLite (`Standalone`) | PostgreSQL (`Postgres`) |
| :--- | :--- | :--- |
| Saída de eventos (`EventFanout`) | `Standalone`: grava a linha do outbox, ninguém a avisar | `PgFanout`: linha + `pg_notify` no mesmo commit |
| Escuta | nenhuma | `PgListener` no canal `netmonitor_events` |
| Liderança | sempre líder | `pg_try_advisory_lock` numa sessão dedicada |

Nenhum outro ponto do código pergunta pelo dialeto para isso.

### Eventos por `LISTEN`/`NOTIFY`

- **O aviso** (`events::notice::Notice`, JSON) é um de: `event` (com o `id`
  da linha quando é evento de domínio), `snapshot` (tipo, **chave** e evento),
  `forget` (o emissor esqueceu os snapshots de uma chave) e `resync`. Todos
  levam `origin` — o processo emissor ignora o próprio aviso.
- **Evento de domínio** (`EventBus::publish`): `INSERT` no outbox e
  `SELECT pg_notify(...)` na mesma transação. O PostgreSQL só entrega o aviso
  no commit: ninguém recebe `id` que ainda não pode ler, e nenhuma linha
  confirmada fica sem aviso. Não há cursor.
- **Snapshot, esquecimento e efêmero** (código síncrono) vão por uma fila
  limitada a uma tarefa de envio por processo, que preserva a ordem. O
  snapshot chega ao outro processo **como snapshot**: entra no cache dele, e
  a aba que conectar lá depois recebe o estado atual.
- **Limite de 8000 bytes do `NOTIFY`.** O que cabe (até 7900 bytes) vai
  inteiro e quem recebe não consulta o banco. O que não cabe ganha linha no
  outbox (com `snapshot_key`, se for snapshot) e o aviso leva só o `id`.
- **Esquecer só o que veio do emissor.** O `forget` remoto apaga apenas os
  snapshots daquela chave que vieram do processo que avisou — o agente que
  caiu num processo e reconectou noutro não perde o snapshot novo por um
  aviso atrasado.

**Reconexão da escuta.** O `NOTIFY` não guarda nada para quem não escuta.
Quando a conexão cai, o `PgListener` reabre o `LISTEN` antes de devolver o
controle; só **então** a recuperação roda, então o que chega dali em diante já
está garantido:

1. relê do outbox as linhas de outros processos desde a última vez em que a
   escuta estava comprovadamente de pé (menos 30 s de folga, no máximo 1000
   linhas), sem repetir os `id` já entregues (memória dos últimos 4096);
2. manda `resync`: cada processo reanuncia os snapshots que **ele** produziu.

"Comprovadamente de pé" é a última notificação recebida ou o último
`SELECT 1` pela própria conexão de escuta, que roda após 30 s de silêncio — é
ele que descobre a conexão meio-aberta. O mesmo `resync` sai quando o
processo começa a escutar: é assim que um processo recém-subido recebe o
estado atual produzido no líder. Efêmero perdido na queda fica perdido, como
numa aba que reconecta.

A escuta roda em **todo** processo PostgreSQL, inclusive `task`: é ela que
responde ao `resync` com os snapshots daquele processo.

### Líder por trava consultiva

- Duas funções (`cluster::leadership::LeaderRole`), duas travas:
  **`Scheduler`** (o ciclo: monitores, rotinas e retenção das áreas, despacho
  de notificações e Web Push, purga, rollup) e **`Background`** (backups do
  NetMonitor e dos bancos, despachante de deploys, tarefas agendadas,
  atualização do registro de fabricantes, IA proativa, limpeza dos testes de
  enlace). São duas para o `SCHEDULER_ENABLED=false` continuar funcionando: o
  servidor que abre mão do ciclo nem disputa a função `Scheduler`, e não tira
  a vez do `task scheduler_loop`.
- **A trava é de sessão**, então mora numa conexão **dedicada**, tirada do
  pool (`detach`) e nunca devolvida. A sessão é um *lease*:
  `idle_session_timeout = 30s` (o líder que para de conferir perde a sessão e
  a trava) e `tcp_keepalives_*` curtos (a máquina que some é notada em
  segundos).
- Um `Elector` por função e processo: seguidor tenta a cada 5 s; líder
  confere a cada 5 s, pela própria sessão, que a trava continua dele
  (`pg_locks`). Perdeu (erro ou trava ausente): avisa pelo `watch` **antes**
  de soltar a sessão, e `while_leader` descarta o futuro da rotina (a
  transação aberta nele é desfeita). Voltou a ser líder: a rotina recomeça do
  início — inclusive a recuperação de boot, que agora é "o que o líder
  anterior deixou em `running`".
- **Deveres de cada processo** continuam em todo processo, porque servem às
  conexões dele: SSE e snapshots, canais dos agentes conectados ali (modo ao
  vivo, `last_seen_at`, entrega de tarefas), o mapa Wi-Fi ao vivo das abas
  daquele processo (o interesse `wifi` é da conexão SSE), a porta de syslog e
  os cadastros idempotentes de boot.
- `task scheduler_loop` disputa a função `Scheduler` como o servidor.
  `task scheduler_run` pega a função para a única passada e **recusa** com
  outro líder vivo — dois ciclos executariam os mesmos monitores.
- No SQLite o processo é sempre líder; nada muda nos testes.

### O rollup sem janela residual

O rollup agregava fora da transação e purgava até o maior `id` visível; no
PostgreSQL um `id` menor confirmado depois entrava na purga sem ter entrado na
conta. Agora agrega e purga dentro de `begin_snapshot_write`
(`REPEATABLE READ` no PostgreSQL, `BEGIN IMMEDIATE` no SQLite): a linha que
confirma depois do começo da transação é invisível para os dois passos, e o
próximo ciclo a agrega pelo recálculo das últimas horas.

## Consequências

- Some o laço de 5 s do relay, o `RELAY_BATCH_SIZE` e o cursor
  (`seed_from_outbox`). O outbox continua: é o registro dos eventos de
  domínio, a carga dos avisos grandes e o que a escuta relê depois de uma
  queda. A retenção dele é a mesma da purga.
- Um `pg_notify` por snapshot e efêmero, mesmo com um processo só. É uma
  instrução barata, e o ganho é o mesmo código servir a uma e a várias
  instâncias.
- Cada processo PostgreSQL usa até três conexões fora do pool: a escuta e uma
  por função disputada.
- **Limites conhecidos.** (a) O despachante de deploys fala com os agentes
  pelo canal do processo líder: com várias instâncias da API, os agentes
  precisam chegar ao líder. (b) A recuperação de boot de quem assume marca
  como interrompida uma execução **manual** (iniciada pela tela num processo
  que não é o líder) que ainda esteja rodando naquele instante — só acontece
  na troca de líder; registrar a instância dona por execução fica para quando
  houver várias instâncias de fato. (c) Snapshots de um processo que morreu
  ficam no cache dos outros até o recurso publicar de novo (o líder novo
  republica os dele no ciclo seguinte; o agente, ao reconectar).
- Testes: o roteamento dos avisos e a máquina de estados da liderança são
  unitários (`events::notice`, `events::fanout::receive`,
  `cluster::leadership`); a entrega real por `NOTIFY`, o snapshot entre dois
  barramentos, o aviso grande pela linha e a troca de líder rodam contra um
  PostgreSQL de verdade em `tests/requests/cluster_postgres.rs`
  (`NM_TEST_POSTGRES_URL`, `--ignored`).
