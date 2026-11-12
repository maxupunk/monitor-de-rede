# ADR 021 — Deploy contínuo pelo agente: GitHub compila, central decide, agente aplica

## Status

**Aceito** (2026-11-08) e implementado. Estende o ADR 011 (agente remoto) e o
ADR 014 (Docker de todo host pelo agente). Especificação de uso em
[deploy_continuo_github.md](../deploy_continuo_github.md).

**A rever (proposto em 2026-11-09):** o [ADR 023](023-compilacao-no-servidor.md)
põe o servidor para compilar, como no Dokploy e no Coolify: o push chega pelo
GitHub App, o agente compila e o deploy segue esta mesma fila. Se ele for
aceito, "GitHub compila" passa a ser um modo opcional da aplicação, e o resto
deste ADR (aprovação, janela, backup, `.env`, rollback) continua valendo.

## Contexto

Projetos em Docker Compose (o SGCv4 é o primeiro) são atualizados à mão: SSH no
servidor, `git pull`, editar o `.env`, `docker compose pull && up -d`. Com
vários clientes, isso vira rotina arriscada — sem aprovação, sem backup antes da
migration, sem volta rápida quando a versão nova não sobe.

O NetMonitor já tem, em cada servidor, um agente que só disca para fora (ADR
011), executa `docker compose` (permissão `compose`) e enxerga os projetos pelas
labels. Faltavam três coisas que o agente **não podia** fazer até aqui:

1. escrever no disco do host (o `.env`, os arquivos do repositório);
2. falar `git`;
3. autenticar no registry sem deixar a senha em `/root/.docker/config.json`.

E faltava a central saber **quando** um deploy deve acontecer: o GitHub precisa
avisá-la, de um jeito que não exija guardar segredo no GitHub nem expor um token
numa URL.

## Decisão

### 1. Quem faz o quê

| Peça | Faz | Nunca faz |
|---|---|---|
| GitHub Actions | compila e publica as imagens no ghcr.io com a tag do commit; avisa a central | acessar o servidor |
| Central | prova que o aviso veio do repositório, decide (aprovação, janela, fila, canário), guarda o `.env` e as credenciais cifrados, faz o backup do banco, pede o rollback, registra tudo | executar `git` ou `docker` (ADR 010/014) |
| Agente | sincroniza o repositório no commit, grava o `.env`, baixa e sobe a stack esperando os healthchecks, limpa imagens antigas | decidir quando, guardar segredo, compilar |

### 2. O agente ganha uma operação fechada, não um shell

`DockerCall::StackDeploy { StackDeployRequest }` e `DockerCall::StackReadiness`,
sob a permissão nova **`deploy`** — fora do padrão (`AGENT_ALLOW`), como
`database` e `volume_export`. `PROTOCOL_VERSION` sobe para **6**. O pedido é
validado **dos dois lados** (`StackDeployRequest::validate`): nome de projeto
sem `/` nem `.`, repositório só `https://` e sem credencial na URL, commit
hexadecimal, arquivos compose relativos e sem `..`, tag e variável em formato
fechado. Argumentos vão um a um ao processo, nunca por shell.

O `ComposeRequest` que já existia continua para as ações manuais da aba Compose;
o deploy reaproveita o mesmo executor de CLI (`docker/cli.rs`) e a mesma guarda
de projeto protegido (o da própria instalação é recusado).

### 3. Uma raiz só, vista pelo caminho do host

O agente escreve **só** dentro de `AGENT_STACKS_DIR`, uma pasta por projeto. O
CLI compose roda no agente, mas quem resolve os bind mounts relativos da stack
(`./docker/nginx`) é a Engine do host: o caminho que o compose manda precisa
existir lá.

No container a raiz é `/stacks`, com um **volume nomeado** montado — o Docker o
cria, e o host não precisa de pasta nenhuma. O agente lê na Engine a origem do
que está montado ali (`Mounts[].Source` do próprio container, achado pelo
`mountinfo`) e cria, dentro do container, um link com esse caminho do host
apontando para a raiz (`docker/stacks/host_path.rs`); o compose trabalha por
ele. Uma pasta do host montada em `/stacks` funciona do mesmo jeito. Nativo
(systemd), a raiz é `/srv/netmonitor/stacks` e o caminho já é o do host.

> Até 2026-11 a raiz era uma pasta do host montada **no mesmo caminho** dos
> dois lados. Funcionava, mas a pasta precisava existir antes do
> `docker compose up` — em hosts que não a criam sozinhos, o agente nem subia.

Um projeto que hoje roda de outro lugar (`/opt/sgc`) é **adotado**: o primeiro
deploy o sobe a partir da raiz com o mesmo nome de projeto compose, então os
containers e os volumes nomeados continuam os mesmos. Pastas montadas que não
estão no repositório precisam ser copiadas antes — a checagem de prontidão avisa.

### 4. Segredos de passagem

- **Git:** o token vai como cabeçalho HTTP em `GIT_CONFIG_*`, variáveis do
  processo filho. Nunca na linha de comando (que o `ps` mostra) nem no
  `.git/config`. O fetch é raso e por SHA, direto pela URL (sem remote gravado).
- **Registry:** um `DOCKER_CONFIG` temporário `0700`, com o `config.json`
  mínimo, apagado ao fim (`Drop`). Nada de `docker login`.
- **`.env`:** guardado cifrado (XChaCha20) na central, em **versões**; gravado no
  host de forma atômica e `0600`. Cada deploy registra a versão que usou.
- No fio, todo segredo é `Secret`, cujo `Debug` não mostra o valor.

### 5. O aviso do GitHub é provado por OIDC

`POST /api/deploy-hooks/github`, fora do guarda JWT. O workflow pede ao GitHub
um token OIDC (`permissions: id-token: write`) com o `audience` desta central
(`DEPLOY_OIDC_AUDIENCE`, padrão `netmonitor-deploy`) e o manda no
`Authorization`. A central confere assinatura (JWKS do GitHub, em cache),
emissor, `audience`, validade e reuso (`jti`). O **repositório do token** escolhe
as aplicações; o ref, os alvos; o commit vem do token, não do corpo.

Rejeitados: token na URL (vaza em log de proxy e do Cloudflare) e segredo HMAC
(mais um segredo para guardar e trocar nos dois lados).

### 6. Imagem por tag imutável; rollback pela central

A tag é o SHA do commit. O agente só sabe deployar **uma revisão**; o rollback
é a central pedindo outro deploy com o commit, a tag e a versão do `.env` do
último deploy que deu certo. As imagens antigas ficam em disco (a limpeza só
roda depois de um sucesso e guarda as últimas N), então a volta não depende do
registry.

O rollback **não desfaz migration**: o deploy termina `rolled_back` com o aviso
escrito, e a tela oferece a cópia do banco feita antes (quando o alvo tem backup
configurado) para restaurar — com confirmação, pelo fluxo de restauração que já
existe (ADR 013).

### 7. O acesso ao GitHub é um GitHub App da central

"Conectar GitHub" cria, pelo **manifest** do GitHub, um app **privado** na conta
ou organização de quem clicou — um por central, nunca um app compartilhado entre
instalações. O fluxo é todo pelo navegador (criar → instalar escolhendo os
repositórios → voltar a `/deploys/github`), então a central não precisa estar na
internet para isso; o `state` é de uso único e emitido por ela.

- A **chave privada** do app fica cifrada na central (`deploy_github_connections`)
  e nunca sai dela. O que sai são **tokens de instalação de uma hora**, pedidos na
  hora e com o mínimo: para o agente clonar, só `contents: read` daquele
  repositório; para o assistente ler, `contents` e `actions` de leitura.
- Permissões pedidas: `metadata`, `contents` e `actions` de leitura. A escrita
  (`contents`, `pull_requests`, `workflows`) é **opcional** e serve só para a
  central abrir o PR do workflow — numa branch dela (`netmonitor/deploy-workflow`),
  nunca direto na branch padrão; quem faz o merge é uma pessoa.
- O webhook do app fica **desligado**: o aviso de versão continua sendo o OIDC
  (§5). O app não amplia o que o aviso prova.
- Com o app, o assistente lista os repositórios, lê o compose (arquivos, imagens
  com `build:`, a variável da tag) e as branches — a pessoa escolhe em vez de
  digitar. É tudo HTTP na central; o processo da API continua sem `git`.
- **Imagem privada no ghcr.io continua pedindo token clássico** com
  `read:packages`: o ghcr.io não documenta o token de GitHub App como credencial
  de registry. A regra de qual credencial vai em cada uso mora num lugar só
  (`deploys::access`): repositório pelo app (ou pelo token, sem app); `pull` só
  pelo token.

O endereço da central vai **escrito no workflow** (não é segredo), com a
variável `NETMONITOR_URL` do repositório tendo precedência — criar a variável
deixa de ser um passo obrigatório.

Rejeitados: OAuth/device flow (o escopo `repo` dá escrita em **todos** os
repositórios da pessoa e fica preso a ela) e um app público do NetMonitor (a
chave dele teria de morar em toda central).

### 8. A decisão de quando

Estados: `awaiting_approval`, `scheduled`, `queued`, `running`, `succeeded`,
`failed`, `rolled_back`, `superseded`, `cancelled`, `rejected`.

- Alvo com aprovação (produção) espera um **administrador da área Servidores**
  (ADR 019): pela tela (com os commits entre a versão no ar e a nova), pelo
  botão da notificação (que só abre a tela) ou pela IA (`deploy_action`, sempre
  risco alto — confirmada a cada chamada).
- Janela de manutenção por alvo; "Publicar agora" a ignora.
- Um deploy novo **substitui** os pendentes do alvo.
- Um deploy por alvo de cada vez; na estratégia sequencial, o alvo seguinte só
  roda se o anterior subiu saudável (canário).
- Agente desconectado: o deploy espera 10 minutos na fila antes de falhar.

O despachante é um laço no processo da API (é ele que tem os canais dos
agentes). O andamento vai pelo SSE (`deploys:run` como snapshot, `deploys:log`,
`deploys:updated`), sem polling (AGENTS §9).

## Consequências

- A imagem do agente traz o `git`; o compose do agente e os comandos de
  instalação montam a raiz das stacks. A permissão `deploy` continua desligada
  nos agentes remotos até alguém a liberar no host; no agente local da central
  ela vem ligada, como `compose`.
- Um projeto adotado muda de diretório de trabalho uma vez.
- Serviço sem healthcheck só é conferido como "de pé" — a prontidão avisa.
- `container_name` fixo implica alguns segundos de indisponibilidade a cada
  troca; deploy sem parada (blue-green) fica fora do escopo.
- Seis tabelas novas na área Servidores (`deploy_credentials`,
  `deploy_github_connections`, `deploy_applications`, `deploy_targets`,
  `deploy_env_versions`, `deploy_runs`).
- Remover a conexão apaga a chave da central, mas o app continua no GitHub: a
  tela leva às configurações dele para apagá-lo de vez.
