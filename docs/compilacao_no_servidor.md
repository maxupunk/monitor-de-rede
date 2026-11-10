# Compilação no servidor — requisitos e experiência de uso

> **Status:** implementado (2026-11-09). Decisão em
> [ADR 023](adr/023-compilacao-no-servidor.md). Depende do registry próprio
> ([ADR 022](adr/022-registry-proprio.md), [requisitos](registry_proprio.md)).
> As fases dos dois estão juntas em [registry_proprio.md §9](registry_proprio.md#9-fases).
> Área: **Servidores**.

## 1. Em uma frase

Como no Dokploy e no Coolify: **você dá push, o servidor compila e sobe.**
Sem workflow, sem PR, sem ghcr.io e sem minutos do Actions. O GitHub só avisa
que houve push.

```
git push ─► GitHub App (evento push, assinado)
              │
              ▼
Central: confere a assinatura → acha a aplicação e a branch → fila de compilação
              │  DockerCall::ImageBuild (canal do agente)
              ▼
Agente do servidor de destino
  clona o commit (token de 1 h, só leitura) → builder BuildKit dedicado
  → estágio de teste (opcional) → imagens + SBOM + proveniência
              │  quadros binários (agente → central)
              ▼
Registry da central (ADR 022): guarda, analisa CVEs, registra a origem
              │
              ▼
Fila de deploy (ADR 021): aprovação → janela → backup → up → rollback se cair
```

## 2. Experiência de uso

### 2.1 Configurar uma aplicação

O assistente "Automatizar deploy" continua com três passos. Muda o que cada um
pede:

1. **Repositório.** Como hoje: escolhe da lista do GitHub App, e o compose
   preenche imagens e Dockerfiles. Entram dois campos novos:
   - **Quem compila?** **NetMonitor** (padrão) ou **GitHub Actions**, o modo do ADR 021.
   - **Estágio de teste** (opcional): o nome do `target` do Dockerfile que
     roda os testes antes de gerar as imagens.
2. **Onde roda.** Como hoje: o agente, o projeto compose e a branch. Entra:
   - **Onde compilar?** **Neste servidor** (padrão: o servidor de destino),
     **Na central** ou **Outro servidor**, que lista os agentes com a
     permissão `build`.
   - A checagem de prontidão mostra se o agente tem a permissão `build`, se o
     builder está pronto, a CPU, a memória e o disco livre, e o tamanho do
     cache.
3. **Concluir.** No modo NetMonitor não há "Ligar o GitHub": nada a copiar,
   nada a commitar. O cartão mostra "Pronto: o próximo push na branch `main`
   compila e publica".

### 2.2 No dia a dia

- **Push na branch:** a compilação aparece na tela Deploys e no console ao
  vivo, com as etapas:
  1. Clonar
  2. Testar
  3. Compilar `<imagem>` (com o percentual de cache aproveitado)
  4. Analisar CVEs
  5. Guardar
  - Depois disso, segue a política do alvo (aprovação, janela), igual a hoje.
- **Botões:** **Compilar agora** (escolhe branch ou commit) e **Recompilar
  sem cache**. Há ainda **Cancelar**, enquanto a compilação está na fila ou
  rodando.
- **Pushes seguidos** na mesma branch: a compilação ainda na fila é
  substituída pela mais nova; a que já está rodando termina. Mesma regra de
  substituição do deploy.
- **Falha:** o console mostra as últimas linhas do passo que falhou e a
  notificação leva até elas. Nada sobe.
- **Histórico** por aplicação: commit, quem deu push, duração, cache, tamanho
  das imagens, resultado e link para o log.

### 2.3 Segredos de build

- Ficam numa aba da aplicação ("Segredos de build"), no formato nome e valor,
  cifrados na central.
- O Dockerfile os usa com `RUN --mount=type=secret,id=NOME`.
- A tela explica isso e avisa quando um `ARG` do Dockerfile parece segredo,
  por exemplo `*_TOKEN`, `*_KEY` ou `*PASSWORD*`.

## 3. Requisitos funcionais

- **RC-01** Webhook do GitHub App:
  - rota própria;
  - assinatura `X-Hub-Signature-256` conferida com o segredo da conexão
    **antes** de interpretar o corpo;
  - idempotente por `X-GitHub-Delivery`;
  - responde em menos de 1 s: só enfileira.
- **RC-02** Filtro: compila só `push` em branch ou tag que algum alvo da
  aplicação acompanha, com os mesmos padrões de ref do ADR 021. Ignora push
  que apaga branch e qualquer evento de PR.
- **RC-03** Fila de compilação por aplicação e ref:
  - substitui a que ainda espera;
  - no máximo uma compilação por builder ao mesmo tempo (configurável);
  - tempo limite por compilação (padrão 30 min).
- **RC-04** Operação `DockerCall::ImageBuild`, sob a permissão `build`, com
  repositório, commit, token de clone, imagens (Dockerfile, contexto,
  `target`), argumentos, referências de segredos, plataforma e estágio de
  teste. Os dois lados validam o contrato.
- **RC-05** O agente:
  - faz fetch raso do commit numa área de trabalho descartável, com o token só
    no ambiente do `git` (como no ADR 021), e apaga a área ao terminar;
  - roda o estágio de teste, se houver, e depois cada imagem no builder dedicado;
  - devolve o andamento por etapas, que vira log e SSE.
- **RC-06** A saída vai ao registry da central em quadros binários com janela
  de crédito, no sentido agente → central. É a mesma peça da entrega do
  ADR 022: `HEAD` antes de cada blob, e o que a central já tem não trafega.
  Quando o builder é o destino, a imagem também é carregada no Docker local.
- **RC-07** `--attest type=sbom` e `--attest type=provenance,mode=max` em toda
  compilação. A central registra a proveniência dela: commit, agente,
  Dockerfiles, argumentos sem segredos, duração e quem disparou (push ou
  usuário).
- **RC-08** Tags: `<sha do commit>` imutável (ADR 022 RF-04), mais a branch
  como tag móvel.
- **RC-09** Ao terminar com sucesso, a central abre o deploy de cada alvo
  daquela ref, com a política do alvo. O bloqueio por CVE vale antes da
  aprovação.
- **RC-10** "Compilar agora", "Recompilar sem cache" (`--no-cache`) e cancelar
  ficam com o administrador de Servidores — a mesma regra de disparar um
  deploy, já que a compilação termina em deploy nos alvos. O operador
  acompanha o console. Todas são auditadas.
- **RC-11** IA: `get_builds` e `get_build_log` só para leitura; disparar
  compilação é `deploy_action` com `action: "build"` — só administrador,
  sempre confirmada, pelo mesmo caminho do "Compilar agora".

## 4. Requisitos de segurança

- **RSC-01** O builder BuildKit dedicado roda num container criado pelo agente.
  - Tem rede própria: não entra nas redes dos stacks nem na da central.
  - Tem limites de CPU, memória e PIDs, mais o tempo limite da compilação.
  - Não monta o `docker.sock`, o estado do agente nem o disco do host.
  - Tem a label `com.netmonitor.protected=true`, para não ser parado pela tela.
  - Execução rootless conforme o SPIKE-BLD-02.
- **RSC-02** O código executado (`RUN`) só enxerga a área de trabalho daquela
  compilação e os segredos montados naquele passo.
- **RSC-03** Os segredos de build:
  - ficam cifrados no cofre da central e viajam no comando só para o agente
    que vai compilar;
  - entram só como `--secret`;
  - nunca vão para argumento, log, SSE, IA, auditoria (registra só o nome) ou
    camada da imagem.
- **RSC-04** Só compila repositório ligado a uma aplicação, em ref acompanhada
  por um alvo. PR, e PR de fork em especial, nunca. O aviso sem assinatura
  válida é descartado sem efeito e registrado.
- **RSC-05** O token de clone é o do GitHub App: uma hora, só leitura, só
  daquele repositório (`deploys::access`). Nunca é gravado.
- **RSC-06** O cache do builder tem teto em bytes, com coleta própria do
  BuildKit. "Recompilar sem cache" não apaga o cache dos outros.
- **RSC-07** A política local do agente é soberana: sem `build` em
  `AGENT_ALLOW`, o agente recusa, e a tela mostra como liberar.

## 5. Requisitos não funcionais

- **RNC-01** O log e o andamento vão ao vivo pelo SSE global, sem polling.
  Snapshot do estado da fila ao conectar.
- **RNC-02** Compilação incremental: o cache persistente por builder deve
  deixar uma compilação sem mudança de dependências em segundos, não minutos.
  A meta de referência é o SGC (a medir).
- **RNC-03** Compilar não atrapalha a medição: no host da central, o builder
  roda com prioridade baixa (CPU shares) e o padrão é compilar no destino
  (SPIKE-BLD-03).
- **RNC-04** Testes:
  - contrato de `ImageBuild` validado dos dois lados;
  - assinatura do webhook (válida, inválida, ausente, repetida);
  - filtro de refs;
  - substituição na fila;
  - segredos fora do `Debug`, dos argumentos e do log;
  - snapshot do comando `buildx` montado;
  - SSE da tela sem chamar a API de novo.

## 6. Spikes

| Spike | Pergunta | Plano B |
| :--- | :--- | :--- |
| **SPIKE-BLD-01** | O `buildx` consegue, numa compilação só, mandar a saída OCI em fluxo para o agente e também carregar no Docker local (vários exporters, BuildKit ≥ 0.13)? | Duas exportações: OCI para a central e `docker load` a partir do mesmo arquivo. |
| **SPIKE-BLD-02** | BuildKit rootless num container no Docker Engine (Linux) e no Docker Desktop: que opções de segurança exige (seccomp, apparmor, `/dev/fuse`)? | BuildKit com root dentro do container dedicado, mantendo rede própria, limites e nenhuma montagem do host. |
| **SPIKE-BLD-03** | Quanto uma compilação do SGC no host da central mexe na latência e no jitter dos monitores de ping? | Proibir "na central" quando o host tiver poucos núcleos; recomendar outro servidor. |
| **SPIKE-BLD-04** | Tempo de uma compilação fria e quente do SGC e do NumBlaster no builder dedicado, comparado ao GitHub Actions. | — (serve de referência para o RNC-02) |
