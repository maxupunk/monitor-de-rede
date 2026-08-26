# ADR 007 — Scheduler: laço em processo único, no lugar do scheduler nativo do Loco

- **Status:** aceito — 2026-08-12.
- **Data:** 2026-08-12

## Contexto

O desenho inicial executava cada ciclo de monitores em uma task isolada,
disparada a cada 5 s pelo scheduler nativo do Loco. Cada job iniciava um
processo novo e descartava as dependências mantidas em memória ao terminar.

O desenho rodou por meses sem que ninguém notasse um problema, porque a stack
não subia inteira: o serviço `migration` falhava e derrubava o `up`. Quando ela
subiu, um monitor de ping numa instalação limpa gravou:

```
status  = unknown
message = "A checagem não pôde ser executada localmente: Cliente ICMP não inicializado"
```

E o processo `scheduler` passou a registrar, a cada 5 segundos:

```
WARN falha ao retransmitir eventos  error="Barramento de eventos não inicializado"
```

## A premissa que estava errada

A ADR 005 se apoia, na seção *"Por que isso não conflita com o `surge_ping::Client`
compartilhado"*, nesta frase:

> O `Initializer` `ping_client` (§9.5) roda no boot da task igual roda no boot do
> servidor.

**Não roda.** No `loco-rs` 1.0.1, quem executa os initializers é o `run_app`:

```rust
// loco-rs-1.0.1/src/boot.rs:464
pub async fn run_app<H: Hooks>(mode: &StartMode, app_context: AppContext) -> Result<BootResult> {
    H::before_run(&app_context).await?;
    let initializers = H::initializers(&app_context).await?;
    for initializer in &initializers {
        initializer.before_run(&app_context).await?;
    }
    ...
```

E o `run_task` não passa por ali:

```rust
// loco-rs-1.0.1/src/boot.rs:204
pub async fn run_task<H: Hooks>(
    app_context: &AppContext,
    task: Option<&String>,
    vars: &task::Vars,
) -> Result<()> {
    let mut tasks = Tasks::default();
    H::register_tasks(&mut tasks);
    // ...só registra e executa. Nem `H::before_run`, nem initializers.
```

Consequência: os processos `scheduler` e `probe` subiam com o `shared_store`
vazio. Como `run_monitor` resolve o checker de ping por
`PingChecker::from_context(ctx)`, **nenhum monitor de ping funcionava** — nem
pelo scheduler nem pelo probe, que usa o mesmo caminho. O fallback local, que o
`AGENTS.md` marca como "NÃO remover", estava morto desde sempre.

Nenhum teste cobria isso: a suíte roda via `request_with_config`, que boota pelo
caminho do **servidor** — justamente o único onde os initializers rodam.

## Decisão

**O ciclo passa a rodar em laço, dentro de um processo de longa duração**
(`task scheduler_loop`), e as dependências de processo passam a ser instaladas
em `Hooks::after_context`.

O `after_context` é o único gancho do Loco chamado em **todos** os modos —
`create_context` o invoca antes de qualquer coisa
(`loco-rs-1.0.1/src/boot.rs:421`), e todo comando cria um contexto.

`task scheduler_run` continua existindo e executando um ciclo só: é o comando
manual de depuração.

## Por que abandonar o scheduler nativo

Corrigir o `shared_store` sozinho já destravaria o ping. O que decidiu contra
manter o subprocesso por tique foi o que a correção **custaria** naquele
desenho, e o que ele nunca entregou:

| | Subprocesso por tique | Laço em processo único |
| :--- | :--- | :--- |
| Socket ICMP | Um novo a cada 5 s | Um por processo |
| Pool de conexões | Reconecta a cada 5 s | Um por processo |
| Cadências internas (`is_due`) | **Não funcionam** — a memória morre com o processo, então VPN, tráfego e purga rodavam a cada tique em vez de 10 s / 30 s / 1 h | Funcionam como escritas |
| Ciclos concorrentes | Possíveis: o scheduler dispara por cima de um ciclo lento | Impossíveis: o laço é sequencial |

A terceira linha é a mais séria e passou despercebida na ADR 005: o
`run_data_pruner_if_due` foi escrito para rodar de hora em hora e, em produção,
tentava rodar a cada 5 segundos.

A quarta linha resolve, por construção, a consequência que a própria ADR 005
listou como aceita ("Ciclos concorrentes são possíveis").

## O que se perde, e por que é aceitável

A ADR 005 defendeu o processo por tique com dois argumentos. Eles continuam
válidos — e continuam atendidos:

- *"Um `panic` no laço derruba a API junto"* — continua verdade, e é por isso
  que o scheduler segue num **container separado** do `server`. O que muda é o
  que acontece dentro do container, não a topologia. `restart: unless-stopped`
  cobre o processo inteiro.
- *"Falha em um bloco não interrompe os outros"* — inalterado: cada bloco do
  ciclo mantém seu `if let Err(...)`, e agora o laço também captura erro do
  ciclo inteiro e continua.

O que de fato se perde é o isolamento **por tique**: um vazamento de memória
lento no ciclo, que o processo descartável mascarava, agora se acumula. É um
risco aceito conscientemente — e é o preço de as cadências internas passarem a
valer.

Também se perde o `config/scheduler.yaml`, removido. O intervalo agora é
`SCHEDULER_INTERVAL_SECONDS` (padrão 5 s).

## O relay de eventos volta para onde a ADR 005 já mandava

A ADR 005 termina dizendo:

> O `EventBus` em memória não atravessa o processo `scheduler`. Por isso os
> eventos gerados no ciclo vão para `event_outbox` e o `server` os retransmite
> no SSE (§11).

A implementação não seguiu isso: `relay_pending` era chamado de dentro do
`run_cycle`, ou seja, **no scheduler**. Lá ele nunca entregava nada — a primeira
linha da função é `if !bus.has_subscribers() { return Ok(0) }`, e um processo que
não atende HTTP nunca tem assinante SSE. Todo evento gerado pelo ciclo — mudança
de estado de dispositivo, alerta aberto — ficava parado no `event_outbox`.

O relay agora é um laço no **servidor**, subido pelo `MonitoringInitializer` —
que, sendo um `Initializer`, roda só no `start`. Aqui a característica que
causou o bug original é exatamente a desejada.

## Consequências

- Uma imagem, quatro papéis, e o `shared_store` populado em todos eles.
- `is_due` passa a valer: purga de hora em hora, telemetria VPN a cada 10 s.
- Dois testes de regressão cobrem os defeitos: um garante que o contexto criado
  fora do caminho do servidor tem cliente ICMP e barramento; outro garante que o
  ciclo não depende de assinante SSE para completar.
