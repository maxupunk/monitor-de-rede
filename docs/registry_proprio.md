# Registry próprio do NetMonitor — requisitos e plano

> **Status:** implementado nas fases 0 a 5 (2026-11-09); as fases 6 e 7
> (modo GitHub Actions) seguem planejadas. Decisão em
> [ADR 022](adr/022-registry-proprio.md).
> Área: **Servidores** ([ADR 017](adr/017-arquitetura-modular.md)).
>
> **Revisão (2026-11-09, plano B):** quem compila passa a ser o **servidor**,
> como no Dokploy e no Coolify: o push chega, o agente compila e o deploy sobe
> ([ADR 023](adr/023-compilacao-no-servidor.md),
> [requisitos](compilacao_no_servidor.md)). O registry recebe as imagens
> **do agente, pelo canal**. O envio do GitHub Actions pelo túnel continua,
> mas virou modo opcional da aplicação.

## 1. Objetivo

Ter, dentro da central, um registry de imagens OCI escrito em Rust. Ele guarda
o que os servidores compilam (ADR 023), atende o deploy contínuo
([ADR 021](adr/021-deploy-continuo-pelo-agente.md)) e recebe o push manual da
equipe, para que nada dependa só do ghcr.io ou do Docker Hub. O ghcr.io
continua suportado: **cada aplicação escolhe** onde as imagens dela ficam.

O que **não** é objetivo:

- Compilar imagens. Isso é do ADR 023 (o agente compila); o registry só guarda
  e serve.
- Ser proxy ou cache do Docker Hub.
- Espelhar automaticamente imagens que vêm de outro registry.

## 2. Decisões da entrevista (2026-11-09)

| Tema | Decisão |
| :--- | :--- |
| Papel | Destino das imagens compiladas pelo agente (ADR 023), do CI no modo GitHub Actions e do push/pull manual de pessoas |
| Build | ~~GitHub Actions, como hoje~~ → **revisto:** o agente compila no servidor (ADR 023); o Actions virou modo opcional |
| Entrega ao servidor | Pelo canal do agente, que só disca para fora (ADR 011): o host nunca chama a central |
| Exposição | A de hoje, o Cloudflare Tunnel; nenhuma porta nova |
| Recebimento | Do agente, pelo canal (quadros binários); no modo GitHub Actions, em partes de ~50 MB pelo túnel, autenticado pelo OIDC |
| Convivência | Cada aplicação escolhe: ghcr.io ou NetMonitor |
| Código | Crate próprio da área Servidores (`crates/servers-registry`), no processo da API |
| Armazenamento | Disco local da central atrás de um trait de storage (S3 depois, sem mexer no resto) |
| Acesso de pessoas | Token de acesso pessoal (PAT) com escopo e validade; nunca a senha do painel |
| Rede do acesso manual | Só rede interna e VPN; pela VPN, com `insecure-registries` (o WireGuard já cifra) |
| Retenção | As últimas N de cada repositório, mais tudo que algum alvo usa ou pode precisar no rollback, mais cota |
| Primeira versão | Tela, tag de commit imutável, varredura de vulnerabilidades e proveniência/assinatura |
| Política de CVE | Por alvo: avisar, ou bloquear a partir de crítica/alta; exceção registrada pelo administrador |
| Base de CVE | Cópia local do OSV.dev, atualizada por rotina |
| Assinatura | Proveniência registrada pela central (ela pediu a compilação, ADR 023) ou pelo OIDC do envio (modo Actions); depois, attestation Sigstore do GitHub no modo Actions |
| Arquiteturas | O registry aceita índice multiplataforma; o agente compila a arquitetura do próprio host, e o workflow gerado compila só amd64 |

## 3. Restrições externas verificadas

| Fato | Consequência no desenho | Fonte |
| :--- | :--- | :--- |
| O Cloudflare limita o corpo da requisição a 100 MB nos planos Free e Pro (200 MB no Business). | O `docker push` manda cada camada numa requisição só, e camadas grandes falham. O CI precisa enviar em partes. | [Cloudflare limits](https://developers.cloudflare.com/workers/platform/limits/) |
| A OCI Distribution Spec define envio em partes (`POST` → `PATCH` com `Content-Range` → `PUT ?digest=`), retomável pelo `GET` do status da sessão. | O envio em partes é o da própria especificação, não um protocolo nosso. Qualquer cliente OCI que fatie o envio funciona. | [distribution-spec](https://github.com/opencontainers/distribution-spec/blob/main/spec.md) |
| Um registry conforme precisa no mínimo da categoria **Pull**. Push, Content Discovery e Content Management são opcionais, mas cada uma vale inteira. | Implementar as quatro e validar com a suíte de conformidade da OCI. | idem |
| Attestations do BuildKit (SBOM e proveniência) vão como manifests no índice da imagem. Os exporters `local` e `tar` não as guardam. | O SBOM chega junto com a imagem e a varredura não precisa abrir camadas. O comportamento do exporter `oci` precisa de spike. | [Docker attestations](https://docs.docker.com/build/metadata/attestations/) |
| Artifact attestations do GitHub em repositório **privado** exigem o plano Enterprise Cloud. | A assinatura Sigstore vale para repositório público ou GHEC. Para os privados (SGC, GameMath), a proveniência vem do OIDC do envio. | [actions/attest](https://github.com/actions/attest) |
| O OSV publica `https://storage.googleapis.com/osv-vulnerabilities/<ECOSYSTEM>/all.zip` e um `modified_id.csv` para atualização incremental. | A base de CVE local baixa só os ecossistemas que aparecem nas imagens e atualiza por diferença. | [OSV data](https://google.github.io/osv.dev/data/) |
| A partir do Docker Engine 25, o `docker image save` gera um arquivo OCI. | O formato que o agente monta para o `docker load` precisa de spike (ver §8). | [Engine 25 release notes](https://docs.docker.com/engine/release-notes/25.0/) |
| O Docker só fala HTTP com `localhost` ou com endereços listados em `insecure-registries`. | O acesso manual usa o IP da central na VPN (10.8.0.1) nessa lista. A tela mostra o trecho do `daemon.json`. | — |

## 4. Arquitetura

```
Agente com `build` (ADR 023) ── BuildKit (SBOM + proveniência) ──► imagem OCI
      │  quadros binários pelo canal (agente → central); HEAD antes de cada blob
      ▼
   [registry]   ◄── principal

Modo GitHub Actions (opcional):
GitHub Actions ── buildx (amd64, SBOM) ──► arquivo OCI
      │  token OIDC ──► POST /v2/token  (central confere o OIDC)
      │  envio em partes ──► /v2/<ns>/<img>/blobs/uploads/…   (Cloudflare Tunnel)
      │  PUT manifest :<sha>  (imutável)  + proveniência gravada
      ▼
Central (processo da API)
  crates/servers-registry
    ├─ api/        OCI Distribution (/v2), token endpoint, envio em partes
    ├─ storage/    trait BlobStore → LocalDisk (S3 depois)
    ├─ catalog/    repositórios, manifests, tags, vínculos, cota
    ├─ access/     PAT, OIDC, escopos, origem (rede interna/VPN)
    ├─ scan/       SBOM → pacotes → base OSV local → achados
    ├─ provenance/ registro da compilação (ADR 023) ou OIDC do envio; Sigstore depois
    └─ retention   rotina (Routine/RetentionJob do core)
      │
      │  deploy: Command (canal do agente, quadros binários com janela de crédito;
      │  nada trafega quando a imagem foi compilada no próprio destino)
      ▼
Agente (host) ── docker load / tag ──► docker compose up --no-build --pull never
Pessoas (VPN) ── docker login 10.8.0.1:3333 com PAT ──► /v2 (push/pull)
```

### 4.1 Fronteiras de crate (ADR 017)

- O `crates/servers-registry` depende do `core` e da base de Servidores
  (`crates/servers`), como os submódulos `network-*` da Rede.
- O deploy, em `crates/servers`, **não** depende do registry. Ele conversa por
  um trait declarado na base da área, por exemplo `deploys::images::ImageSource`,
  com as operações "esta imagem existe?", "qual o veredito da varredura?",
  "qual a proveniência?" e "transfira para o agente". O trait é implementado
  no registry e injetado pelo ponto de montagem (`src/registries.rs`).
- O `tests/conventions/architecture.rs` passa a conhecer o crate novo. O crate
  também entra em `WORKSPACE_CRATES`, senão o log dele some (ADR 018).

### 4.2 Rotas

- **`/v2/…` fica na raiz**, porque o Docker exige `/v2/` no host. A rota é
  registrada antes do prefixo `/api`, como `_ping`. Ela não usa a sessão do
  painel: a autenticação é a do Docker, um Bearer obtido no token endpoint.
- `GET /v2/token` segue o *Docker Registry token auth*:
  - recebe Basic com usuário e PAT, ou usuário `oidc` com o JWT do GitHub;
  - devolve um JWT de 5 minutos, com escopo `repository:<ns>/<img>:pull,push`.
- A tela usa `/api/registry/*`, que entra nos prefixos do `AreaPolicy` de
  Servidores (ADR 019). Nenhum `if` próprio de área.

### 4.3 Nomes

`<host-da-central>/<namespace>/<imagem>:<tag>`

- Namespace de aplicação de deploy: o slug da aplicação. O CI só envia para o
  namespace da aplicação cujo `repository` coincide com o do OIDC.
- Namespace livre: criado na tela para o push manual.
- O `image:` do compose usa o host público da central. Para quem faz pull pela
  VPN, a tela mostra o nome equivalente com o IP da VPN.

## 5. Requisitos funcionais

### Registry OCI

- **RF-01** Atender a OCI Distribution Spec nas quatro categorias: pull, push
  (monolítico e em partes, com retomada), content discovery (lista de tags e
  `referrers`) e content management (apagar tag, manifest e blob).
- **RF-02** Montagem entre repositórios (`POST …/uploads/?mount=&from=`), sem
  copiar bytes, só quando quem pede pode ler a origem.
- **RF-03** Aceitar manifest de imagem, índice multiplataforma, artefato
  (`artifactType`/`subject`) e os tipos Docker v2 (`schema2` e manifest list).
- **RF-04** Tag de commit imutável:
  - uma tag no formato de SHA de commit (40 caracteres hex) nunca muda de digest;
  - reenviar o mesmo digest é idempotente, e um digest diferente é recusado com `DENIED`;
  - tags móveis (`latest`, `main`) são permitidas e marcadas como móveis na tela.

### Recebimento do agente (principal)

- **RF-04a** O agente que compilou (ADR 023) envia a imagem à central pelo
  canal, em quadros binários com janela de crédito, na fila de menor
  prioridade. É a mesma peça da entrega do RF-10, no sentido inverso (DRY).
  Antes de cada blob, pergunta se a central já o tem: camada repetida não
  trafega.
- **RF-04b** A central só aceita a imagem de uma compilação que ela mesma
  pediu: o agente, o commit, os nomes e as tags vêm do pedido, não do envio.
  A proveniência é gravada a partir do pedido.

### Envio pelo CI (modo GitHub Actions, opcional)

- **RF-05** O workflow gerado ganha a variante "registry do NetMonitor":
  - `buildx` com `--platform linux/amd64`, `sbom=true` e saída em arquivo OCI;
  - um passo de envio usa o OIDC já existente e manda as partes de ~50 MB;
  - nenhum segredo no repositório.
- **RF-06** Antes de cada blob, o passo de envio faz `HEAD`. Camada que a
  central já tem não é reenviada. É isso que torna o envio barato depois do
  primeiro deploy.
- **RF-07** No envio, a central grava a **proveniência** do manifest, de forma
  imutável. Ela vem do OIDC: `repository`, `repository_id`, `ref`, `sha`,
  `workflow_ref`, `run_id`, `run_attempt`, `actor` e `event_name`.
- **RF-08** O OIDC só autoriza push para o namespace da aplicação ligada
  àquele repositório. Se o alvo restringe branch, só para refs permitidos.
  Nunca autoriza pull, delete nem outro namespace.

### Entrega ao servidor

- **RF-09** A origem das imagens decorre de "Quem compila?" (ADR 023):
  - **NetMonitor** (padrão): as imagens ficam no registry da central, sem escolha a fazer;
  - **GitHub Actions**: a aplicação escolhe **ghcr.io** ou **NetMonitor**, e o
    workflow gerado, a checagem de prontidão e o plano do deploy seguem essa escolha;
  - trocar depois é editar a aplicação. No modo Actions, também é preciso
    reabrir o PR do workflow.
- **RF-10** Quando a imagem é do NetMonitor, a central a transfere ao agente
  pelo canal dele:
  - um comando novo no enum fechado do `agent-protocol`, sob a permissão `deploy`;
  - quadros binários com janela de crédito, na fila de menor prioridade, como a
    ponte de banco do ADR 013;
  - nada vai no canal de controle;
  - o agente carrega a imagem no Docker local, aplica a tag do compose e sobe
    com `--pull never`.
  - A mudança sobe `PROTOCOL_VERSION`.
- **RF-11** Só vai ao agente a plataforma do host (`linux/amd64`, `arm64`…)
  e, quando der, só as camadas que o Engine não tem (§8, SPIKE-REG-01).

### Acesso de pessoas

- **RF-12** Token de acesso pessoal (PAT):
  - criado pelo usuário em "Tokens do registry", com nome, validade obrigatória
    (no máximo 1 ano), namespaces e ações (`pull`, `push`, `delete`);
  - exibido uma vez só; revogável; mostra o último uso e o IP;
  - o escopo efetivo é a interseção com o papel do usuário em Servidores:
    `viewer` só faz pull, `operator` faz push, e `delete` exige administrador.
- **RF-13** O PAT só é aceito de origem interna: rede privada ou VPN do
  NetMonitor. Pelo túnel público, só o OIDC do CI (RS-04).
- **RF-14** A tela entrega, prontos para copiar:
  - o trecho do `daemon.json` com `insecure-registries` para o IP da central na VPN;
  - o `docker login`, o `docker push` e o `docker pull` com os nomes reais.

### Retenção e espaço

- **RF-15** Cada repositório mantém as **N** versões mais novas (padrão 10). Também fica:
  - todo digest que algum alvo usa ou de que o rollback precisa (versão atual e
    as anteriores guardadas pelo ADR 021);
  - toda versão **fixada** à mão.
- **RF-16** Uma rotina de coleta (marca e varre):
  - roda como `Routine`/`RetentionJob` da área Servidores;
  - apaga manifests fora da retenção;
  - apaga blobs sem referência com mais de 24 h (carência para envios em
    andamento) e sessões de envio abandonadas.
- **RF-17** Cota total configurável:
  - aviso pelos canais de notificação a 80%;
  - acima de 95%, recusa envios novos com mensagem clara, sem quebrar pull nem deploy.

### Tela (menu Servidores → Registry)

- **RF-18** Repositórios:
  - nome, origem (aplicação ou livre), número de tags, tamanho real (camadas
    deduplicadas), último envio e por quem ou qual workflow;
  - barra de uso da cota.
- **RF-19** Detalhe da versão:
  - digest, plataformas, camadas e tamanho;
  - cartão de proveniência (commit e execução, com link para o GitHub);
  - resumo do SBOM e achados de CVE por severidade;
  - **quais alvos usam**;
  - ações: copiar o pull, fixar, apagar (bloqueado quando em uso).
- **RF-20** Atualização ao vivo pelo SSE global: envio concluído, varredura
  concluída, coleta concluída. Nenhum polling (AGENTS §9).
- **RF-21** No deploy, o passo de aprovação mostra o veredito da varredura e a
  proveniência da imagem que vai entrar.

### Varredura de vulnerabilidades

- **RF-22** Fonte primária: o SBOM SPDX que o BuildKit anexou à imagem. Os
  pacotes vêm com `purl`.
- **RF-23** Imagem sem SBOM, como no push manual: um extrator em Rust lê só
  `var/lib/dpkg/status`, `lib/apk/db/installed` e
  `usr/lib/os-release`/`etc/os-release` das camadas, com limites (§6). Sem
  executar nada. Lockfiles de linguagem ficam para depois.
- **RF-24** A base OSV fica local:
  - baixa por ecossistema só o que aparece nas imagens (Debian, Alpine,
    Ubuntu, npm, PyPI, crates.io, Go, Maven…);
  - atualiza por `modified_id.csv`;
  - funciona sem internet entre uma atualização e outra;
  - nunca manda a lista de pacotes para fora.
- **RF-25** A comparação de versões respeita a regra de cada ecossistema:
  `dpkg`, `apk`, semver, PEP 440 e Maven. Severidade pelo vetor CVSS (crate
  `cvss`); sem vetor, a severidade é "desconhecida".
- **RF-26** Quando a base muda, as versões **em uso** são revarridas. CVE nova
  crítica numa imagem no ar vira aviso pelos canais de notificação.
- **RF-27** Política por alvo: `avisar` (padrão), `bloquear ≥ crítica` ou
  `bloquear ≥ alta`. Severidade desconhecida só avisa.
  - Exceção: CVE + aplicação, com motivo e validade, criada só pelo
    administrador e auditada.
  - O deploy bloqueado mostra o porquê e o caminho da exceção.

### Proveniência e assinatura

- **RF-28** O alvo pode exigir **"só imagem de origem conhecida"**:
  - compilada pela central a partir deste repositório (RF-04b), ou enviada
    pelo workflow dele (RF-07);
  - em ambos os casos, `repository_id` e branch permitida;
  - push manual nunca passa nessa exigência.
- **RF-29** Fase posterior: conferir a attestation Sigstore do GitHub (crate
  `sigstore`), para repositório público ou GHEC. O workflow gerado acrescenta
  `actions/attest` com `push-to-registry`, e a attestation chega como
  referrer. O alvo pode exigir "proveniência verificada".

## 6. Requisitos de segurança

- **RS-01** **Pré-requisito: um extrator único de origem.**
  - Hoje `rate_limit::extract_client_ip` e o `audit` confiam no primeiro
    `X-Forwarded-For` de qualquer cliente, uma lógica duplicada e falsificável.
  - O extrator novo parte do endereço do socket e só aceita cabeçalho
    encaminhado vindo de um proxy listado em `TRUSTED_PROXIES` (o `cloudflared`
    e o proxy reverso). Nesse caso usa o IP mais à direita que não é proxy
    confiável, ou o `CF-Connecting-IP`.
  - O rate limiter, a auditoria e o registry passam a usá-lo (DRY).
- **RS-02** O PAT é um token aleatório de 32 bytes com prefixo `nmr_`.
  - Guardado como hash SHA-256. O token tem alta entropia, então não precisa
    de Argon2, e a conferência acontece uma vez por login, no token endpoint.
  - Nunca vai a log, à IA nem ao SSE.
- **RS-03** O JWT do registry:
  - dura 5 minutos e tem escopo mínimo (repositório e ações);
  - é assinado com chave própria do registry, guardada cifrada pela `ENCRYPTION_KEY`;
  - o `aud` é o serviço do registry, então não serve para a API do painel nem o contrário.
- **RS-04** Pela origem pública (túnel), só o OIDC é aceito, e o OIDC só faz
  push no namespace da aplicação daquele repositório (RF-08). PAT vindo de
  origem pública é recusado, com uma mensagem que explica o porquê.
- **RS-05** Integridade:
  - todo blob é verificado pelo digest enquanto chega (hash incremental),
    antes de virar visível;
  - manifest que referencia blob ausente é recusado;
  - nome de repositório e digest são validados pelas regex da especificação,
    então não há caminho de arquivo vindo do cliente.
- **RS-06** Isolamento entre repositórios: o blob é guardado uma vez, mas só
  é legível pelos repositórios que o vinculam, por envio ou por mount
  autorizado. Conhecer um digest não dá acesso a ele.
- **RS-07** Limites:
  - tamanho de manifest (4 MiB) e de blob (configurável, padrão 10 GiB);
  - parte do envio (máximo de 64 MiB, abaixo do Cloudflare);
  - sessões simultâneas por token, sessão de envio expirando em 24 h;
  - rate limit no token endpoint.
- **RS-08** A varredura não executa conteúdo da imagem. O extrator lê arquivos
  pontuais com teto de tamanho e de entradas por camada, contra tar-bomb, e
  ignora links e caminhos fora da raiz.
- **RS-09** Auditoria: push, delete, fixar, criação e revogação de PAT,
  exceção de CVE e liberação de deploy bloqueado.
- **RS-10** Escrita atômica: arquivo temporário, `fsync`, `rename`. Transação
  que escreve abre com `begin_write`. Toda consulta vale em PostgreSQL e
  SQLite.
- **RS-11** Os blobs não são cifrados em repouso pela aplicação. A proteção é
  a do disco e do volume, e o documento de operação diz isso explicitamente.

## 7. Requisitos não funcionais

- **RNF-01** Fluxo contínuo de ponta a ponta: nenhum blob inteiro em memória
  (upload, download, transferência ao agente e varredura). Pull com suporte a
  `Range`.
- **RNF-02** Conformidade validada pela suíte oficial da OCI
  (`distribution-spec/conformance`) no CI do projeto, nas quatro categorias.
- **RNF-03** Desempenho:
  - pull pela LAN ou VPN no limite do link;
  - envio do CI limitado só pelo túnel;
  - dedupe por digest em todo o armazenamento.
- **RNF-04** `BlobStore` é um trait (`put_stream`, `get_range`, `exists`,
  `delete`, `size`). O driver local vem primeiro; S3 entra depois sem mexer em
  API, catálogo ou varredura (aberto/fechado).
- **RNF-05** Testes:
  - funções puras com teste unitário no módulo (parse de nomes, faixas de
    versão, comparadores, política);
  - API por `request_with_config`;
  - `#[serial]` onde houver estado de processo;
  - snapshot `insta` do workflow gerado;
  - teste provando que a tela se atualiza pelo SSE sem chamar a API de novo.
- **RNF-06** Esquema: tabelas novas entram na migration da área (sem produção,
  edita-se a migration) e em `CREATION_ORDER`. As entidades são geradas contra
  o PostgreSQL.

## 8. Spikes antes da implementação

| Spike | Pergunta | Plano B |
| :--- | :--- | :--- |
| **SPIKE-REG-01** | O `docker load` de um tar montado pelo agente pula as camadas que o Engine já tem, tanto no store `overlay2` quanto no containerd (padrão nas instalações novas)? Quanto tráfego economiza num deploy típico do SGC? | O agente abre um registry só em `127.0.0.1`, atendido pelo canal, e o Docker faz `pull` nativo, com dedupe garantido. Exige exceção explícita à regra "o agente não abre porta" num ADR. |
| **SPIKE-REG-02** *(só modo Actions)* | O exporter `oci` do buildx põe o SBOM no arquivo? | `docker/build-push-action` com `push` para um registry efêmero no runner (`localhost:5000`) e cópia em partes de lá; ou `anchore/sbom-action` enviando o SBOM como referrer. |
| **SPIKE-REG-03** *(só modo Actions)* | Envio de uma camada de 1 GiB em partes pelo Cloudflare Tunnel do plano Free: tempo, retomada após queda, limite de duração da requisição. | Partes menores (25 MB) e mais tentativas por parte. |
| **SPIKE-REG-04** | O `docker login`/`push` com token auth Bearer funciona contra a central pela VPN com `insecure-registries` (Docker Desktop e Engine Linux)? | Basic auth direto em `/v2` (o Docker também aceita), com cache do hash do PAT. |
| **SPIKE-REG-05** *(só modo Actions)* | O enviador do CI: script `bash` + `curl` gerado no workflow (auditável, sem binário) contra um binário Rust pequeno servido pela central. | Ficar no script e reavaliar se ele passar de ~150 linhas. |

## 9. Fases

| Fase | Entrega | Aceite |
| :--- | :--- | :--- |
| **0. Base** | RS-01 (extrator de origem com `TRUSTED_PROXIES`, usado por rate limit, auditoria e quem mais lê IP); spikes REG-01 e REG-04 e BLD-01 a BLD-04 | Teste prova que `X-Forwarded-For` de cliente não confiável é ignorado; os spikes têm resultado escrito nos ADRs 022 e 023 |
| **1. Registry OCI** | Crate `servers-registry`; `BlobStore` local; catálogo; `/v2` completo; token endpoint; PAT com escopo e origem; tela de tokens com `daemon.json` e comandos | Suíte de conformidade OCI verde; push/pull manual pela VPN; PAT de origem pública recusado |
| **2. Transferência pelo agente** | Quadros binários nos dois sentidos (agente → central para guardar, central → agente para o deploy), com `HEAD` antes de cada blob; tag de commit imutável; plataforma do host; protocolo novo | Imagem enviada à mão sobe num alvo pelo canal; a segunda transferência só leva as camadas novas |
| **3. Compilação no servidor** (ADR 023) | Webhook `push` do GitHub App; fila de compilação; `DockerCall::ImageBuild`; builder BuildKit dedicado; estágio de teste; segredos de build; "Quem compila?" e "Onde compilar?" no assistente; console e histórico | Push no NumBlaster compila no destino, guarda na central e sobe, sem workflow no repositório; o rollback usa a imagem guardada |
| **4. Tela e retenção** | Registry no menu Servidores; detalhe da versão; "quem usa"; fixar e apagar; cota; rotina de coleta; SSE | Coleta preserva o que o rollback precisa; a cota avisa e recusa como descrito; zero polling |
| **5. Varredura** | Base OSV local e rotina de atualização; leitura de SBOM; extrator `dpkg`/`apk`; comparadores; política por alvo; exceções; revarredura e aviso | Uma imagem com CVE conhecida bloqueia o alvo configurado; a exceção libera e fica auditada |
| **6. Modo GitHub Actions no registry** (opcional) | Spikes REG-02, 03 e 05; workflow com envio em partes por OIDC; proveniência pelo OIDC | Workflow gerado publica na central pelo túnel |
| **7. Assinatura** (modo Actions) | `actions/attest` no workflow; verificação Sigstore; exigência por alvo | Repositório público com attestation passa; imagem sem attestation é recusada no alvo que exige |

O índice multiplataforma vale desde a fase 1. O agente escolhe a plataforma do
host na fase 2. As fases 6 e 7 só importam para quem mantiver o GitHub
Actions compilando.

## 10. Riscos e pontos em aberto

- **O túnel vira o gargalo do envio (só no modo Actions).** Na compilação no
  servidor nada sobe pela internet. Se o SPIKE-REG-03 mostrar que o túnel é
  lento demais, a alternativa é uma porta própria com TLS, que é a opção recusada na
  entrevista. Ela voltaria como decisão nova, não como atalho.
- **Disco:** dedupe e retenção resolvem o caso comum. Imagens grandes e muitos
  repositórios pedem o driver S3 cedo.
- **A base OSV não cobre tudo:** RHEL/UBI e imagens distroless sem banco de
  pacotes ficam com cobertura parcial. A tela mostra "cobertura parcial" em
  vez de "sem vulnerabilidades".
- **Repositório privado sem GHEC** não tem attestation do GitHub. A garantia
  que resta é a proveniência do OIDC do envio, forte, mas registrada pela
  própria central.
