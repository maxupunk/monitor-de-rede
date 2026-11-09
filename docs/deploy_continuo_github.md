# Deploy contínuo (GitHub → NetMonitor → servidor)

> A decisão de arquitetura está no [ADR 021](adr/021-deploy-continuo-pelo-agente.md).
> Para configurar na ordem, com cada passo conferido:
> [passo_a_passo_deploy_continuo.md](passo_a_passo_deploy_continuo.md).
> Este documento é o guia: como funciona, como configurar e o que cada tela faz.
> Ele substitui a especificação inicial: as decisões abaixo saíram da revisão
> com o time (aprovação em produção, `.env` na central, raiz única no agente,
> OIDC, rollback que avisa sobre o banco).

## 1. Em uma frase

O **GitHub** compila e publica as imagens no ghcr.io com a tag do commit; a
**central** decide quando e onde elas entram no ar (aprovação, janela de
manutenção, backup, fila, canário, rollback); o **agente** que já está no
servidor aplica — sem SSH, sem porta aberta, sem compilar nada no servidor.

```text
GitHub Actions ── build + push (ghcr.io, tag = SHA) ── POST /api/deploy-hooks/github
                                                         (token OIDC do próprio GitHub)
                                                                    │
Central ── aprovação? janela? fila? canário? ── backup do banco ── StackDeploy ──▶ agente
   ▲                                                                                │
   └──── SSE: etapa e console ao vivo ◀── git fetch (SHA) · .env 0600 · pull · up --wait
                                              └─ falhou? a central pede a versão anterior
```

## 2. Vocabulário

| Termo | O que é |
|---|---|
| **Aplicação** | Um repositório do GitHub (`dono/repo`): de onde vêm o compose e as imagens. |
| **Alvo** | Onde a aplicação roda: um agente, um projeto compose e a branch (`main`) ou padrão de tag (`v*`) que o alimenta, com a política. Uma aplicação pode ter vários (homologação, produção, cliente B). |
| **Deploy** | Cada pedido, do aviso do GitHub (ou do clique) até a conclusão, com o console inteiro. |
| **Versão do `.env`** | Cada `.env` salvo para um alvo. Fica cifrado; o deploy registra qual usou. |

## 3. Configurando (uma vez)

### 3.1 No servidor (agente)

1. Libere o deploy na política local: acrescente `deploy` ao `AGENT_ALLOW`
   (no agente local da central, `LOCAL_AGENT_ALLOW` — já vem ligado).
2. A raiz das stacks precisa estar montada **no mesmo caminho** do host:
   `- /srv/netmonitor/stacks:/srv/netmonitor/stacks`. As instalações novas já
   montam; em agentes antigos, recrie com o compose atualizado.
3. Agente instalado por systemd precisa do `git` no host.

A tela **Deploys → Automatizar deploy** checa tudo isso antes de salvar e diz o
que falta em cada item (agente, permissão, diretório, git, compose, disco,
healthchecks, projeto em outro diretório).

### 3.2 Na central

**Deploys → Conexões → Conectar GitHub** (recomendado): a central cria um
**GitHub App** privado na sua conta ou organização. Você confirma no GitHub,
escolhe os repositórios e volta — sem copiar token. Marque "deixar o NetMonitor
abrir o PR do workflow" para a central colocar o workflow no repositório por
você (por PR, nunca direto na branch padrão). A chave do app fica cifrada na
central; o servidor recebe, a cada deploy, um token de **uma hora**, só de
leitura e só daquele repositório.

**Imagens privadas no ghcr.io** ainda pedem um token **clássico** com
`read:packages` (Conexões → Novo token; o link já abre o GitHub com o escopo
marcado): o ghcr.io não aceita token fine-grained nem o do GitHub App. Imagens
públicas dispensam. Sem conexão, o mesmo token com `repo` também lê repositório
privado.

**Automatizar deploy** (ou o botão do mesmo nome em cada projeto da aba Docker →
Compose), em três passos:

1. **De onde vem** — com o GitHub conectado, escolha o repositório da lista: a
   central lê o compose e **preenche sozinha** os arquivos, as imagens que o
   GitHub compila e a variável da tag, e avisa o que falta no compose (serviço
   com `build:` sem `image:` publicada, imagem fora do ghcr.io). Sem conexão,
   digite `dono/repo` e use o botão de varinha para ler o compose de um
   repositório público.
2. **Onde roda** — servidor, projeto compose, branch/tag (escolhida da lista do
   repositório; o padrão dele já vem marcado), aprovação, janela de manutenção,
   backup do banco antes, rollback automático, e a checagem de prontidão.
3. **Variáveis** — o `.env` de produção (opcional agora).

No fim, a tela abre **Ligar o GitHub**.

### 3.3 Ligar o GitHub

A tela confere o repositório e mostra, em uma lista: se o workflow está lá (e
igual ao gerado), se há PR do NetMonitor esperando merge e como foi o último
build no Actions.

1. **Endereço público da central**: vem preenchido com o endereço da aba; troque
   se você acessa por IP interno (a tela avisa: o GitHub não alcança rede
   local). Ele vai escrito no workflow — criar a variável `NETMONITOR_URL` no
   repositório passou a ser opcional (ela tem precedência, para trocar sem
   editar o arquivo).
2. **Abrir o PR com o workflow** (com a conexão que pode escrever) — ou, sem
   ela, "Copiar o YAML" / "Criar no GitHub". O workflow:
   - roda nas branches/tags dos alvos;
   - compila cada imagem com cache do GitHub e publica com a tag `${{ github.sha }}`;
   - pede o token OIDC (`id-token: write`, `audience` desta central) e avisa a
     central. Nada do evento é interpolado dentro de `run:` — entra por `env:` e
     o JSON sai do `jq`.
3. No `docker-compose.yml`, cada serviço compilado aponta para a imagem
   publicada: `image: ghcr.io/<dono>/<nome>:${IMAGE_TAG:-latest}`. O `build:`
   pode ficar para o desenvolvimento local; no servidor o compose roda com
   `--no-build`.

Nenhum segredo vai para o GitHub.

## 4. O que acontece num deploy

| Etapa | Onde | O quê |
|---|---|---|
| Aprovação | central | Alvo com aprovação espera um administrador da área Servidores. A tela mostra os commits entre a versão no ar e a nova (API do GitHub) e oferece **Publicar agora** ou **Publicar na janela**. A notificação (Web Push) tem o botão "Revisar e publicar", que abre essa tela. |
| Janela | central | Fora da janela, o deploy aprovado fica `agendado` e entra na fila quando ela abre. "Publicar agora" ignora a janela. |
| Fila | central | Um deploy por alvo. Na estratégia "um de cada vez", o alvo seguinte só roda se o anterior subiu saudável. Agente desconectado: espera 10 min. |
| Backup | central (ADR 013) | Se o alvo tem conexão de banco, o dump roda antes e o deploy só segue se ele terminou bem. |
| Repositório | agente | `git fetch --depth 1` do SHA exato, checkout; o token só no ambiente do processo. |
| Variáveis | agente | `.env` da versão escolhida, gravado de forma atômica, `0600`. |
| Imagens | agente | `docker compose pull` com login temporário no registry. |
| Subir | agente | `up --detach --no-build --remove-orphans --wait`: o próprio compose espera os healthchecks. Falhou? As últimas linhas dos serviços que caíram vão ao console. |
| Limpeza | agente | Só depois do sucesso: remove tags antigas das imagens da stack, guardando as últimas N para o rollback. |
| Rollback | central → agente | Com rollback automático, a central pede ao agente a versão anterior — commit, tag **e** `.env`. O deploy termina `revertido`, com o aviso de que o banco pode estar à frente e, havendo backup, o botão para restaurar a cópia feita antes. |

Um push novo **substitui** os deploys ainda pendentes do mesmo alvo. Reexecutar
o workflow de uma versão que já está no ar (ou na fila) não cria outro deploy.

## 5. As telas

- **Deploys** (menu Servidores): números do dia, aviso de aprovações
  pendentes, um cartão por aplicação com uma linha por alvo (servidor, branch,
  aprovação, janela, versão no ar) e os deploys recentes.
- **Deploy (console)**: etapas, console ao vivo, commits (na aprovação),
  aprovar/recusar/cancelar, "Restaurar esta versão" e a restauração do banco.
- **Variáveis**: modo lista (busca, olho para segredos) ou texto bruto;
  **Revisar mudanças** mostra só os nomes que entram, mudam e saem; **Salvar
  para o próximo deploy** ou **Salvar e aplicar agora**; histórico de versões.
- **Publicar**: republicar a versão no ar (depois de mudar variáveis) ou
  escolher outra; agora ou na janela.
- **Histórico** do alvo.

## 6. Quem pode o quê

| Ação | Papel |
|---|---|
| Ver deploys, console e histórico | qualquer papel com acesso à área Servidores |
| Cadastrar, aprovar, publicar, restaurar, abrir/salvar `.env` | administrador da área Servidores (ADR 019) |
| Conectar o GitHub, listar repositórios, ler o compose, abrir o PR | administrador da área Servidores |
| A IA (`get_deploys`, `get_deploy_log`) | quem tem a área; `deploy_action` só administrador, **sempre** com confirmação |

Toda mudança vai à auditoria (recurso "Deploy contínuo"), inclusive a abertura
do `.env`.

## 7. Limites conhecidos

- Serviço sem healthcheck só é conferido como "de pé".
- `container_name` fixo implica alguns segundos fora do ar a cada troca.
- O rollback não desfaz migrations: por isso o backup antes e o aviso.
- Aplicações com URL embutida no build do frontend precisam de uma imagem por
  cliente; a saída é ler a configuração em tempo de execução (é o que o SGCv4
  passa a fazer).

## 8. Referência técnica

| Peça | Onde |
|---|---|
| Contrato do fio (`StackDeployRequest`, `StackChunk`, validação) | `backend/crates/agent-protocol/src/docker/stacks.rs` |
| Execução no agente (git, `.env`, registry, compose, limpeza, prontidão) | `backend/crates/servers/src/docker/stacks/` |
| Domínio na central (fila, aprovação, OIDC, rollback, SSE, IA) | `backend/crates/servers/src/deploys/` |
| GitHub (app pelo manifest, descoberta, compose, PR) | `backend/crates/servers/src/deploys/github/`, `deploys/access.rs`, `deploys/connections.rs` |
| Rotas | `backend/crates/servers/src/controllers/deploys.rs` (`/api/deploys`, `/api/deploy-hooks/github`) e `deploy_github.rs` (`/api/deploys/github/…`) |
| Tabelas | `backend/crates/servers/src/migrations/m20261108_000001_deploys.rs` |
| Tela | `frontend/src/pages/deploys/`, `frontend/src/components/deploys/`, `frontend/src/stores/deploys.ts` |

Variáveis de ambiente: `AGENT_STACKS_DIR` (agente, padrão
`/srv/netmonitor/stacks`) e `DEPLOY_OIDC_AUDIENCE` (central, padrão
`netmonitor-deploy`).
