# Passo a passo — deploy contínuo no NetMonitor

> **Para quem:** o administrador que vai ligar o deploy automático de uma
> aplicação (o SGCv4 é o primeiro caso). **Resultado:** cada versão nova no
> GitHub chega ao servidor sozinha — com a sua aprovação, na janela que você
> escolher, com backup do banco antes e volta automática se algo der errado.
>
> A referência técnica está em [deploy_continuo_github.md](deploy_continuo_github.md)
> e a decisão de arquitetura no [ADR 021](adr/021-deploy-continuo-pelo-agente.md).
> O lado do SGC tem o próprio guia: `docs/PASSO_A_PASSO_DEPLOY.md` no repositório dele.

---

## 1. Como funciona, em 1 minuto

Três peças, cada uma com um papel só:

| Peça | O que faz | O que **não** faz |
|---|---|---|
| **GitHub Actions** | Roda os testes, compila as imagens e publica no ghcr.io com a tag do commit. No fim, avisa o NetMonitor. | Não acessa o servidor. |
| **NetMonitor (central)** | Confere que o aviso veio mesmo do seu repositório, decide **quando** (aprovação, janela, fila) e **onde** (quais servidores), faz o backup do banco, mostra tudo ao vivo e pede a volta se der errado. | Não executa `git` nem `docker`. |
| **Agente no servidor** | Baixa o compose do commit, grava o `.env`, baixa as imagens prontas e sobe os containers esperando os healthchecks. | Não compila nada, não decide nada, não guarda senha. |

```text
 você faz merge na branch
          │
          ▼
 GitHub Actions ── testes ── build das imagens ── publica no ghcr.io (tag = commit)
          │
          └─ "versão nova!" ─▶ NetMonitor ── precisa de aprovação? ── espera você
                                   │           fora da janela?     ── espera a janela
                                   │
                                   ├─ backup do banco
                                   └─ pede ao agente ─▶ servidor: git · .env · pull · up --wait
                                                              │
                                        ficou saudável? ── sim ─▶ ✔ no ar
                                                        └─ não ─▶ volta a versão anterior
```

Nenhuma porta é aberta no servidor (o agente só disca para fora) e nenhum
segredo vai para o GitHub.

### O caminho de um deploy

| Estado na tela | Quer dizer |
|---|---|
| **Aguarda aprovação** | Versão nova pronta; um administrador precisa revisar e publicar. |
| **Agendado** | Aprovado para a janela de manutenção; entra na fila quando ela abrir. |
| **Na fila** | Esperando o agente conectar, o alvo ficar livre ou o alvo anterior terminar. |
| **Publicando** | O agente está aplicando. O console mostra cada linha. |
| **No ar** | Subiu e todos os healthchecks passaram. |
| **Revertido** | A versão nova não ficou saudável e a anterior voltou ao ar. |
| **Falhou** | Não subiu e não havia (ou não deu para voltar) a versão anterior. |
| **Substituído** | Chegou uma versão mais nova antes desta rodar. |
| **Cancelado / Recusado** | Alguém desistiu antes de rodar. |

---

## 2. Antes de começar (checklist)

- [ ] A central do NetMonitor e **todos** os agentes estão na mesma versão (a
      com deploy contínuo). O protocolo subiu para a versão 6 e o aperto de mão
      é estrito: agente antigo não conecta numa central nova.
- [ ] Você é **administrador** da área Servidores.
- [ ] Você é dono da conta (ou organização) do GitHub onde está o repositório —
      é quem cria e instala o GitHub App.
- [ ] O servidor da aplicação já tem o agente do NetMonitor conectado
      (Servidores remotos → "Conectado").
- [ ] A central é alcançável pela internet num endereço `https://` (o GitHub
      precisa chamá-la). Basta expor a rota `/api/deploy-hooks/github`.
- [ ] A central consegue sair para a internet (busca as chaves do GitHub e a
      lista de commits).

---

## 3. Preparar o servidor (o agente)

### 3.1 Liberar o deploy na política do agente

A central **não** consegue ligar isso sozinha: quem decide é o servidor.

**Agente em container** (`docker/agent/docker-compose.yml`, o mais comum):

```yaml
environment:
  AGENT_ALLOW: read,lifecycle,monitor,discovery,update,compose,deploy
volumes:
  # Mesmo caminho dos dois lados — obrigatório.
  - /srv/netmonitor/stacks:/srv/netmonitor/stacks
```

Depois recrie o agente:

```bash
docker compose -f docker/agent/docker-compose.yml up -d
```

**Agente instalado por `docker run`:** as instalações novas já montam
`/srv/netmonitor/stacks`. Num agente antigo, remova e rode de novo o comando
de instalação da tela **Servidores remotos**, acrescentando `deploy` em
`AGENT_ALLOW`.

**Agente local da central** (o do `docker-compose.yml` do NetMonitor): já vem
com `deploy` e com a pasta montada. Só confira se `LOCAL_AGENT_ALLOW`, se você
o definiu no `.env`, inclui `deploy`.

**Agente por systemd:** acrescente `deploy` em `/etc/netmonitor-agent/agent.env`,
instale o `git` no host (`apt install git`) e reinicie
(`systemctl restart netmonitor-agent`).

> **Por que o mesmo caminho?** O agente roda o `docker compose` dentro do
> container dele, mas quem monta as pastas do projeto (`./docker/nginx`, por
> exemplo) é o Docker do servidor. Se o caminho for diferente, os containers
> sobem com pastas vazias.

### 3.2 Conferir

Em **Servidores remotos**, a linha do agente passa a mostrar o chip
**Deploy contínuo**. A checagem de prontidão (passo 6) confere o resto.

---

## 4. Conectar o GitHub

Uma vez por conta (ou organização) do GitHub. Faça neste navegador, logado no
GitHub com quem é dono da conta.

1. **Deploys → Conexões → Conectar GitHub**.
2. Escolha **Minha conta** ou **Uma organização** (informe o login dela).
3. Deixe marcado **Deixar o NetMonitor abrir o PR do workflow** — é o que
   permite colocar o workflow no repositório sem copiar nada.
4. **Continuar no GitHub** → o GitHub mostra o app que vai ser criado (um app
   privado, só desta central; dá para trocar o nome ali) → **Create GitHub App**.
5. O GitHub abre a instalação: escolha **Only select repositories** e marque o
   repositório da aplicação (ou todos) → **Install**.
6. Você volta para o NetMonitor com "GitHub conectado". Se estava no
   assistente, ele reabre.

A chave do app fica cifrada na central e nunca sai dela. A cada deploy o
servidor recebe um token de **uma hora**, só de leitura e só daquele
repositório. Para incluir outro repositório depois: Conexões → ícone de
repositórios da conexão.

### 4.1 Imagens privadas: um token clássico

O ghcr.io só aceita token **clássico** para baixar imagem privada (nem o
fine-grained nem o do GitHub App). Se as imagens forem públicas, pule.

1. **Deploys → Conexões → Novo token** → o link **Criar o token no GitHub**
   abre a tela do GitHub com `read:packages` já marcado. Gere e copie.
2. Preencha **Nome**, **Usuário do GitHub** (o login de quem gerou) e
   **Token**; deixe **Registry** vazio (ghcr.io).

O token fica cifrado e só vai ao servidor durante o `pull`.

---

## 5. (Recomendado) Cadastrar o banco para o backup antes do deploy

As migrations da versão nova rodam quando a API sobe. Se a versão der errado,
o NetMonitor volta a imagem, mas **não desfaz a migration**. Com o backup
antes, você tem como voltar o banco também.

1. **Backup → O que está protegido → Proteger um banco de dados**.
2. Em **Bancos em contêineres dos agentes**, escolha o PostgreSQL da
   aplicação. A tela testa os endereços e escolhe o que responde.
3. Se o banco está num servidor remoto, em **Acessar a partir de** escolha o
   agente de lá. No host do agente, libere `database` no `AGENT_ALLOW` e o
   destino em `AGENT_DATABASE_TARGETS` (a própria tela mostra as linhas).
4. Escolha **para onde vão as cópias** (um destino da aba **Destinos**). Sem
   destino, a conexão aparece desabilitada no deploy.

---

## 6. Automatizar o deploy (o assistente)

Abra **Deploys → Automatizar deploy**, ou vá em **Docker → Compose**, ache o
projeto e clique em **Automatizar deploy** (já vem com o servidor e o projeto
preenchidos).

### Passo 1 — De onde vem

Com o GitHub conectado:

1. **Conexão com o GitHub** já vem escolhida; em **Repositório**, digite parte
   do nome e escolha da lista.
2. A central lê o compose do repositório e **preenche sozinha** os arquivos
   compose, as imagens que o GitHub compila (nome, Dockerfile, contexto) e a
   variável da tag. O quadro verde diz o que foi preenchido; os avisos amarelos
   dizem o que ajustar no compose (serviço com `build:` sem `image:` publicada,
   imagem fora do ghcr.io).
3. Confira e ajuste o que quiser:

| Campo | O que pôr |
|---|---|
| **Nome** | Sugerido pelo repositório. |
| **Token para imagens privadas** | O do passo 4.1, se as imagens forem privadas. |
| **Arquivos compose** | Preenchido; a lista mostra os outros compose do repositório. |
| **Imagens que o GitHub compila** | Preenchidas. Servem para gerar o workflow; se o projeto já tem workflow próprio, pode deixar vazio. |
| **Avançado → Variável da tag** | Preenchida (a que o compose usa em `image: ...:${IMAGE_TAG}`). |
| **Avançado → Vários alvos no mesmo push** | **Um de cada vez** (recomendado): o servidor seguinte só recebe se o anterior subiu saudável. |

Sem conexão: escolha **Digitar o repositório**, informe `dono/repositorio` e,
se ele for público, use o botão de varinha ao lado para ler o compose.

### Passo 2 — Onde roda

| Campo | O que pôr |
|---|---|
| **Nome do alvo** | "Produção", "Homologação", "Cliente B"… |
| **Servidor (agente)** | O servidor da aplicação. A lista diz quem está conectado e quem libera deploy. |
| **Projeto compose** | O **mesmo** nome com que o projeto roda hoje (no servidor: `docker compose ls`). É isso que preserva containers e volumes. |
| **Acompanha** | A branch (`stable`) ou um padrão de tag (`v*`). A lista traz as branches do repositório (a padrão já vem marcada) e o `v*` quando há tags `v1.2.3`. |
| **Pedir aprovação** | **Ligado em produção.** Desligado em homologação. |
| **Janela: início / fim** | Ex.: `23:00` a `05:00`. Vazio = qualquer hora. |
| **Backup do banco antes** | A conexão do passo 5. |
| **Voltar a versão anterior se os healthchecks falharem** | Ligado. |
| **Avançado → Espera dos healthchecks** | Quanto esperar a aplicação ficar saudável. Com migrations demoradas, aumente (ex.: 300 s). |
| **Avançado → Versões antigas guardadas** | 3 é um bom número: são as que permitem voltar sem baixar nada. |
| **Avançado → Ordem** | Menor primeiro: homologação 0, produção 1. |

Embaixo aparece a **Checagem de prontidão**. Resolva o que estiver em
vermelho; amarelo é aviso.

| Item | Se estiver vermelho/amarelo |
|---|---|
| Agente conectado | O container do agente parou ou perdeu a rede. |
| Permissão de deploy | Faltou `deploy` no `AGENT_ALLOW` (passo 3.1). |
| Diretório das stacks | Faltou montar `/srv/netmonitor/stacks` no mesmo caminho. |
| Git no agente | Agente antigo: atualize a imagem. Systemd: instale o `git`. |
| Docker Compose | Agente antigo: atualize a imagem. |
| Espaço em disco | Menos de 5 GiB livres: libere espaço ou guarde menos versões. |
| Projeto da instalação | O nome escolhido é o do próprio NetMonitor: escolha outro. |
| Projeto em outro diretório | Normal na primeira vez: o projeto será **adotado**. Pastas montadas que não estão no Git (uploads, `./storage`) precisam ser copiadas antes. |
| Healthchecks | Serviços sem healthcheck só são conferidos como "de pé". |
| Acesso ao GitHub / Variáveis | Avisos do que ainda falta cadastrar. |

**Checar de novo** refaz tudo depois que você corrigir o servidor.

### Passo 3 — Variáveis

Cole o `.env` de produção do servidor (o arquivo inteiro, com comentários). Ele
fica cifrado na central e é gravado no servidor a cada deploy, legível só pelo
dono. Pode deixar para depois.

**Concluir** salva tudo de uma vez: se algo falhar, nada fica pela metade.

---

## 7. Ligar o GitHub

Logo depois de concluir, abre a janela **Ligar o GitHub** (depois, no botão do
mesmo nome no cartão da aplicação):

1. **Endereço público desta central** — vem com o endereço da aba. Se você
   acessa por IP interno, troque pelo público (a tela avisa quando o GitHub não
   alcança). Ele fica lembrado neste navegador e vai escrito no workflow.
2. **Abrir o PR com o workflow** — a central cria a branch
   `netmonitor/deploy-workflow` com o `.github/workflows/deploy.yml` e abre o
   PR. **Revisar no GitHub** e faça o merge. O arquivo não tem segredo nenhum:
   o GitHub prova quem é com um token OIDC.
3. A lista de cima mostra, depois do merge: **Workflow: no repositório e
   atualizado** e, a cada push, **Último build: passou / falhou / compilando**
   com o link para o Actions. **Conferir de novo** relê.
4. **Compose** — embaixo, a linha `image:` de cada imagem, para conferir o
   `docker-compose.yml`.

Sem conexão que escreva: abra **Criar o workflow**, e use **Criar no GitHub**
(abre o editor do GitHub já com o arquivo) ou **Copiar o YAML**. A variável
`NETMONITOR_URL` no repositório é opcional: serve para trocar o endereço sem
editar o arquivo.

> O SGCv4 já tem o workflow e o compose prontos no branch
> `feat/deploy-continuo-netmonitor`: lá o status mostra "diferente do gerado",
> o que é esperado (o dele tem as etapas de checagem e os build args).

---

## 8. O dia a dia

### Quando chega uma versão nova

1. Você faz merge na branch que o alvo acompanha.
2. O GitHub compila (alguns minutos). Ao terminar, o NetMonitor:
   - em **homologação** (sem aprovação): publica sozinho;
   - em **produção**: avisa — notificação no celular/navegador com o botão
     **Revisar e publicar**, e o aviso amarelo no topo da página **Deploys**.
3. Na tela do deploy você vê **o que muda** (os commits entre a versão no ar e
   a nova, com link para o GitHub) e escolhe:
   - **Publicar agora** — ignora a janela;
   - **Publicar na janela (23:00–05:00)** — fica agendado;
   - **Recusar** — com um motivo opcional, que fica no histórico.
4. Acompanhe no **Console**: as etapas acendem (Backup → Repositório →
   Variáveis → Baixar imagens → Subir e conferir saúde → Limpar) e cada linha do
   servidor aparece ao vivo.

Se duas versões chegam antes de você aprovar, a mais nova **substitui** a
anterior — você só aprova o que vale a pena.

### Mudar uma variável do `.env`

1. No alvo, **Variáveis**.
2. Edite no modo **Lista** (com busca e olho para senhas) ou **Texto (.env)**.
3. **Revisar mudanças** mostra só os **nomes** que entram, mudam e saem.
4. **Salvar para o próximo deploy** ou **Salvar e aplicar agora** (recria os
   containers com o `.env` novo, sem build).

Cada versão fica no **Histórico** da janela; **Abrir no editor** traz uma
versão antiga de volta para você revisar e salvar.

### Publicar uma versão específica

**Publicar** no alvo: republique a versão no ar (útil depois de mudar
variáveis), escolha uma versão anterior da lista ou informe um commit. Agora ou
na janela.

### Voltar uma versão

- **Automático:** se os healthchecks falharem, o NetMonitor volta sozinho a
  imagem **e** o `.env` da versão anterior. O deploy fica **Revertido**, com o
  motivo.
- **Manual:** **Histórico** → abra um deploy que ficou **No ar** →
  **Restaurar esta versão**.

### Quando o banco precisa voltar também

Se a versão nova rodou migrations, o banco pode ter ficado "à frente" da versão
restaurada. A tela do deploy revertido avisa e, havendo backup, lista **Cópias
do banco feitas antes deste deploy** com **Restaurar este banco**, que abre a
restauração de banco do NetMonitor (com confirmação).

### Pela IA

Pergunte, por exemplo, "por que o último deploy do SGC falhou?" ou "qual versão
está no ar em produção?". A IA também aprova, recusa, publica ou restaura —
**sempre** pedindo sua confirmação antes de cada ação.

---

## 9. Problemas comuns

| Sintoma | Causa provável | O que fazer |
|---|---|---|
| O GitHub Actions falha no passo "Avisar o NetMonitor" com 401 | `audience` diferente ou relógio muito errado | O workflow precisa de `audience=netmonitor-deploy` (ou o valor de `DEPLOY_OIDC_AUDIENCE` da central). |
| Falha com 404 "Nenhuma aplicação usa o repositório" | Repositório digitado diferente no NetMonitor | Confira `dono/repo` na aplicação. |
| O Actions passa, mas nenhum deploy aparece | Nenhum alvo acompanha aquela branch, ou ela já está no ar | Confira o campo **Acompanha** dos alvos. |
| Deploy fica **Na fila** e falha após 10 min | Agente desconectado | Veja o agente em Servidores remotos. |
| "A política do agente não libera deploy" | Falta `deploy` no `AGENT_ALLOW` | Passo 3.1. |
| Falha em "Baixar imagens" | Imagem com essa tag não existe, ou imagem privada sem token | Confira o build no Actions (Ligar o GitHub → Último build) e o token clássico com `read:packages` (passo 4.1). |
| "Este retorno do GitHub não foi pedido por esta central" | A volta do GitHub demorou mais de uma hora, veio de outra aba ou a central reiniciou no meio | Clique em **Conectar GitHub** de novo. Se o app já tinha sido criado, apague-o nas configurações do GitHub antes. |
| "O GitHub App não tem acesso ao repositório" | O repositório não foi marcado na instalação | Conexões → ícone de repositórios da conexão → marque o repositório. |
| O assistente não lista o repositório | Mesmo motivo acima | Idem; depois, o botão de atualizar da lista. |
| "Abrir o PR" não aparece | A conexão foi criada só de leitura | Crie o arquivo à mão (Criar no GitHub) ou conecte de novo marcando "abrir o PR do workflow". |
| **Revertido** com "Não ficaram saudáveis: …" | A aplicação não passou no healthcheck | O console mostra as últimas linhas do serviço que caiu. Se for só demora, aumente a **espera dos healthchecks**. |
| "O backup antes do deploy falhou e nada foi alterado" | Banco ou armazenamento inacessível | Corrija em Backup → O que está protegido e publique de novo. |
| "A central reiniciou durante o deploy" | A central caiu no meio | Confira a stack no Docker e publique de novo. |

---

## 10. Quem pode fazer o quê

| Ação | Quem |
|---|---|
| Ver deploys, console e histórico | Quem tem acesso à área Servidores |
| Cadastrar, aprovar, publicar, restaurar, abrir e salvar o `.env` | Administrador da área Servidores |
| Conectar o GitHub, listar repositórios, abrir o PR do workflow | Administrador da área Servidores |

Tudo fica na **Auditoria**, recurso "Deploy contínuo", inclusive quem abriu o
`.env`.
