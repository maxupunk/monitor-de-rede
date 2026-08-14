# ADR 005 — Scheduler: task de um ciclo, disparada pelo scheduler nativo do Loco

- **Spike:** SPIKE-05 (§3.4 do `roadmap_backend_rust.md`)
- **Status:** ⚠️ **SUPERSEDIDA** pela [ADR 007](007-scheduler-processo-unico.md)
  em 2026-08-12. Aceita na Fase 0; confirmava a decisão pré-registrada na §9.1.
- **Data:** 2026-08-10

> **Leia a [ADR 007](007-scheduler-processo-unico.md) antes de usar este
> documento.** A frase *"O `Initializer` `ping_client` roda no boot da task
> igual roda no boot do servidor"*, na seção sobre o `surge_ping::Client`, é
> **falsa**: o `run_task` do Loco não executa initializers. Foi essa suposição
> que deixou todo monitor de ping gravando `unknown` em produção.
>
> O restante do documento — inclusive a medição de custo de boot — continua
> correto e é registro de por que a decisão original fez sentido na época.

## Contexto

O AdonisJS roda um processo `scheduler:run` separado, com um laço que a cada
ciclo busca monitores vencidos (`next_run_at <= now`), executa e grava o
resultado. O intervalo mínimo de um monitor é 15 segundos (§6, tabela 10) e a
§9.1 prevê um tique de 5 s.

Pergunta do spike: `cargo loco task` num laço infinito é o padrão certo, ou
`Initializer` + `tokio::spawn`?

## Decisão

**Nenhum dos dois na forma proposta pela pergunta.** Vale o desenho da §9.1:

- `src/tasks/scheduler_run.rs` executa **um ciclo** e termina;
- o **scheduler nativo do Loco** (`backend-cli scheduler`) o dispara a cada
  5 s, num **processo `scheduler` separado** do `server` (topologia da §9.1).

## Evidência

### Como o scheduler do Loco executa um job

```rust
// loco-rs-1.0.1/src/scheduler.rs
pub fn run(&self) -> io::Result<std::process::Output> {
    let shell = if cfg!(windows) {
        duct::cmd!("cmd.exe", "/C", &self.command)
    } else {
        duct::cmd!("/bin/sh", "-c", &self.command)
    };
    ...
}
```

Cada disparo é um **processo novo**: `fork` + `exec` + boot da aplicação. A
objeção óbvia é o custo — trocar o `execFile('ping')` por um `fork` a cada
ciclo seria o mesmo problema com outro nome. Então foi medido.

### Custo real do boot de uma task

Binário release, `LOCO_ENV=development`, SQLite, 5 execuções de
`backend-cli task` (que faz boot completo do `AppContext`, incluindo o pool
de banco):

```
1490 ms   ← primeira execução (cache frio de página do SO)
  29 ms
  21 ms
  21 ms
  68 ms
```

**~25 ms de boot num tique de 5 000 ms: 0,5% de overhead.** O argumento de
custo não se sustenta. Com Postgres o handshake TCP soma algumas dezenas de ms
— ainda uma ordem de grandeza abaixo do tique.

### Por que isso não conflita com o `surge_ping::Client` compartilhado

A §3.2.1 exige **um `Client` por processo**, não um cliente global entre
processos. O defeito do backend atual é abrir um socket **por checagem**; um
socket por ciclo de 5 s, reaproveitado por todos os monitores daquele ciclo,
respeita a regra. O `Initializer` `ping_client` (§9.5) roda no boot da task
igual roda no boot do servidor.

### Por que não `Initializer` + `tokio::spawn` no processo HTTP

O `server` já é o processo que atende SSE e mantém a sessão de scan ao vivo.
Pendurar nele o laço de monitoramento faz uma varredura pesada disputar CPU com
requisição de usuário, e um `panic` no laço derruba a API junto. Processos
separados isolam a falha — que é o desenho do `docker-compose` atual e o motivo
de o `event_outbox` (§6 #20) existir.

### Por que não um laço infinito dentro de uma task

Um laço infinito registrado no scheduler nativo mistura os dois modelos: a task
nunca terminaria e o scheduler continuaria disparando novas a cada tique. Um
laço infinito dentro de uma task é um serviço disfarçado de tarefa.

## Consequências

- `scheduler_run` é idempotente e **termina**. Isso a torna testável sem
  servidor e invocável à mão (`backend-cli task scheduler_run`) para
  depuração — o mesmo código que roda em produção.
- Cada bloco do ciclo (§9.2) tem tratamento de erro próprio: falha em um não
  interrompe os outros, e o processo sai com status limpo para o scheduler não
  entender o tique como quebra.
- **Ciclos concorrentes são possíveis.** Se um ciclo passar de 5 s, o scheduler
  dispara o próximo por cima. Duas defesas, ambas já no roadmap: gravar
  `next_run_at = now + interval_seconds` **antes** de executar (§9.2) e o
  `probe_tasks.monitor_id UNIQUE` (§6 #21). Sem isso, um monitor lento seria
  checado em duplicata.
- O `EventBus` em memória não atravessa o processo `scheduler`. Por isso os
  eventos gerados no ciclo vão para `event_outbox` e o `server` os retransmite
  no SSE (§11) — não é redundância, é a consequência direta desta decisão.
