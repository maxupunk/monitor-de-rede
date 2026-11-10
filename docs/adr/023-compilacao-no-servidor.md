# ADR 023 — Compilação no servidor: o push chega, o agente compila, o deploy sobe

## Status

**Aceito** (2026-11-09) e implementado. Muda o "GitHub compila" do ADR 021,
que passa a ser um modo opcional por aplicação ("Quem compila?"), sobre o
registry próprio (ADR 022). Requisitos e experiência de uso em
[compilacao_no_servidor.md](../compilacao_no_servidor.md); fases em
[registry_proprio.md §9](../registry_proprio.md#9-fases).

**Pendente de medição em campo:** os spikes BLD-02 (BuildKit rootless), BLD-03
(impacto da compilação no jitter dos monitores) e BLD-04 (tempos frio/quente)
dependem de um host Linux real; o builder dedicado já sai com prioridade baixa
(`cpu-shares=256`) e um núcleo livre por padrão (`AGENT_BUILD_CPUS`). O
BLD-01 foi resolvido por desenho: a saída OCI vai a um arquivo no agente, que
a central busca pela ponte — sem depender de vários exportadores no buildx.

## Contexto

No ADR 021, o GitHub Actions compila, publica no ghcr.io e avisa a central.
Isso funciona, mas cobra caro em uso e em dependência:

- um workflow em YAML e um PR em cada repositório;
- duas telas para acompanhar (Actions e NetMonitor);
- minutos do Actions gastos;
- imagem que sai do GitHub, vai ao ghcr.io e volta ao servidor;
- token clássico `read:packages` para repositório privado.

Ferramentas como Dokploy e Coolify mostram o modelo que a equipe quer: **o
servidor recebe o push, compila e sobe**, sem arquivo nenhum no repositório.

A central já tem quase tudo para isso:

- o GitHub App, que dá token de uma hora, só leitura e só do repositório;
- o agente, que já faz `git` e `docker compose` e só disca para fora;
- a fila de deploy com aprovação, janela e rollback;
- o console ao vivo pelo SSE e o registry próprio (ADR 022).

## Decisão

1. **Quem compila é um agente, nunca o processo da API** (ADR 014: a API não
   roda `docker`).
   - O agente precisa da permissão nova `build`, fora do padrão, em `AGENT_ALLOW`.
   - **Padrão: o agente do servidor de destino**, então a imagem nasce onde vai rodar.
   - Opções por aplicação: o agente local da central ou outro agente com
     `build`, uma "máquina de build".
2. **Gatilho: o evento `push` do GitHub App.**
   - O manifest passa a criar o app com webhook ativo e o evento `push`.
   - O GitHub gera o segredo do webhook e o devolve na conversão do manifest.
     A central o guarda cifrado e confere `X-Hub-Signature-256` em cada aviso.
   - `X-GitHub-Delivery` torna o aviso idempotente.
   - Só compila push em branch ou tag que algum alvo da aplicação acompanha.
     PR, e PR de fork em especial, nunca.
   - Há também o botão "Compilar agora".
   - Central sem endereço público: só o botão. Não há consulta periódica ao
     GitHub, porque o GitHub oferece aviso (AGENTS §9).
3. **Operação fechada no protocolo:** `DockerCall::ImageBuild`.
   - Leva repositório, commit, token de clone de uma hora, imagens (Dockerfile,
     contexto, alvo), argumentos, segredos de build e plataforma.
   - Sobe `PROTOCOL_VERSION`.
   - O agente nunca recebe um comando de shell nem um caminho vindo da central.
4. **Builder dedicado e isolado.**
   - O agente mantém um container BuildKit próprio, criado e atualizado por ele,
     com a label `com.netmonitor.protected=true`.
   - O container tem limites de CPU, memória e tempo, cache persistente com
     teto em bytes e nenhum acesso ao `docker.sock`, ao token do agente ou ao
     disco do host.
   - O código é clonado numa área de trabalho descartável.
   - Segredos de build (token do npm, por exemplo) vêm do cofre da central e
     entram só como `--secret` do BuildKit: nunca como argumento, no log ou
     numa camada.
5. **Saída:** a imagem vai para o registry da central pelo canal do agente
   (quadros binários, sentido agente → central, a mesma peça da entrega do
   ADR 022).
   - Quando o builder é o próprio destino, a imagem também fica no Docker
     local, e o deploy não transfere nada.
   - O BuildKit anexa o SBOM e a proveniência (`--attest`), então a varredura
     do ADR 022 não muda.
6. **A proveniência é da central.** Foi ela que pediu a compilação, então ela
   registra o commit buscado com o token do app, o agente, os Dockerfiles, os
   argumentos (sem os segredos), a duração e o log. Isso não depende do plano
   do GitHub.
7. **Testes são opcionais e ficam no Dockerfile.** A aplicação pode indicar um
   estágio de teste (`--target test`), que roda antes das imagens e, se falhar,
   interrompe a compilação.
8. **Depois de compilar, segue o fluxo do ADR 021:** varredura, aprovação,
   janela, backup, deploy e rollback, na mesma fila e no mesmo console.
9. **O GitHub Actions continua como modo da aplicação** ("Quem compila?
   NetMonitor | GitHub Actions"). Nesse modo valem o workflow gerado do
   ADR 021 e o envio em partes por OIDC do ADR 022.

## Alternativas consideradas

| Alternativa | Por que não |
| :--- | :--- |
| Runner do GitHub (`actions/runner`) gerenciado pelo NetMonitor | Mantém o YAML, o PR e a tela dividida. Exige a permissão de administração do repositório para registrar o runner, e o GitHub desaconselha runner próprio em repositório público. Pode voltar como modo extra se alguém precisar do CI do Actions na própria máquina. |
| Compilar no processo da API | Proibido: a API não roda `docker` (ADR 014), e compilar executa código do repositório. |
| `docker build` no daemon do host, sem builder dedicado | O build divide o daemon e o cache com os containers em produção e não tem limite de recurso. O builder dedicado isola os dois e dá para podar. |
| Compilar sem Dockerfile (Nixpacks/Railpack, buildpacks) | Fora da primeira versão. A aplicação já declara `build:` no compose; detectar a linguagem pode vir depois, como gerador de Dockerfile. |
| Consulta periódica ao GitHub em vez de webhook | O GitHub oferece aviso, e a regra do projeto só admite coletor no backend quando a fonte não oferece. |

## Consequências

- O manifest do GitHub App muda: webhook ativo, evento `push` e segredo
  guardado na conexão. Apps criados antes precisam ser recriados (sem produção,
  sem caminho de migração).
- Rota nova do webhook do app, separada da do OIDC do Actions, com a
  assinatura conferida antes de qualquer leitura do corpo.
- Tabelas novas: compilações (fila, estado, commit, agente, duração, imagens
  produzidas) e segredos de build cifrados. O log segue o mesmo caminho do log
  de deploy.
- O assistente ganha "Quem compila?" e "Onde compilar?". A checagem de
  prontidão confere a permissão `build`, o builder, a CPU e o disco livre.
- Risco assumido: o servidor executa o código do repositório. As mitigações
  são o item 4 e a regra "só branches cadastradas, nunca PR".
- Compilar na central pode atrapalhar o monitoramento (CPU e jitter de ping);
  por isso o padrão é o servidor de destino. A medição fica no SPIKE-BLD-03.
- `AGENTS.md` §7 ganha a regra quando for implementado: compilar só pela
  operação fechada, só no builder dedicado e só com a permissão `build`.
