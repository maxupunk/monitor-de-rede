# ADR 022 — Registry próprio: OCI em Rust na central, envio em partes por OIDC, entrega pelo agente

## Status

**Aceito** (2026-11-09) e implementado nas fases 0 a 5 de
[registry_proprio.md §9](../registry_proprio.md#9-fases): extrator único de
origem (`shared::client_addr`), registry OCI (`crates/servers-registry`),
trânsito de imagens pelo canal do agente, tela, retenção e varredura. As fases
6 e 7 (envio do GitHub Actions por OIDC e attestation Sigstore) são do modo
Actions, opcional, e seguem planejadas. Estende o ADR 021 (deploy contínuo) e
o ADR 013 (quadros binários pelo canal do agente).

**Como ficou a entrega ao host (SPIKE-REG-01):** a central monta o tar que o
`docker load` lê (o `manifest.json` clássico e o layout OCI, como o
`docker save` do Engine 25) só com a plataforma do host, e o agente carrega
com o nome do `image:` do compose. A imagem que o host já tem (rollback,
segunda tentativa) não trafega. A transferência só das camadas que faltam —
o ganho que o spike mediria — fica como otimização: hoje a imagem vai inteira
quando falta.

**Revisão (2026-11-09):** com o [ADR 023](023-compilacao-no-servidor.md), quem
compila passa a ser o agente no servidor, como no Dokploy e no Coolify. O
caminho principal de entrada no registry virou o canal do agente (item 3). O
envio em partes por OIDC (item 4) ficou para o modo GitHub Actions, que é
opcional.

## Contexto

O deploy contínuo (ADR 021) puxa as imagens do ghcr.io. Isso deixa cada deploy
e cada rollback dependentes de um serviço de fora. Também obriga quem tem
repositório privado a guardar um token clássico com `read:packages`, e não há
lugar para a equipe guardar imagens que não passam pelo GitHub.

A central já tem as peças que um registry pede:

- verificação do OIDC do GitHub;
- canal autenticado até cada host, que só disca para fora;
- cofre cifrado, papéis por área, auditoria, SSE e rotinas de retenção.

O que falta é o protocolo OCI, o armazenamento por conteúdo e o envio que
atravessa o Cloudflare Tunnel, que limita cada requisição a 100 MB.

## Decisão

1. **Um registry OCI escrito em Rust, no processo da API**, no crate
   `crates/servers-registry` da área Servidores.
   - Atende a OCI Distribution Spec inteira (pull, push, discovery, management)
     em `/v2`, na raiz do host.
   - Armazena por digest atrás do trait `BlobStore`: disco local agora, S3 depois.
   - O deploy conversa com ele por um trait da base da área, injetado no ponto
     de montagem. `crates/servers` não depende do registry.
2. **Cada aplicação escolhe ghcr.io ou NetMonitor.** O ghcr.io continua de
   primeira classe no modo GitHub Actions. Quando o NetMonitor compila
   (ADR 023), as imagens ficam no registry da central.
3. **Caminho principal: o agente que compilou envia pelo canal.**
   - Quadros binários com janela de crédito, a mesma peça da entrega (item 6)
     no sentido inverso.
   - A central só aceita a imagem de uma compilação que ela pediu.
   - Camada que a central já tem não trafega.
4. **Modo GitHub Actions: o CI envia em partes pelo túnel, autenticado pelo OIDC.**
   - O token endpoint troca o JWT do GitHub por um token de 5 minutos, que só
     faz push no namespace da aplicação ligada àquele repositório.
   - As partes seguem o envio em partes da própria especificação OCI, até 64 MiB cada.
   - Nenhum segredo no repositório.
5. **Pessoas usam token de acesso pessoal (PAT), só de origem interna ou VPN.**
   - O PAT tem escopo, validade e é revogável.
   - O escopo efetivo é a interseção com o papel do usuário em Servidores (ADR 019).
   - Pela VPN, o Docker usa `insecure-registries`, e o WireGuard cifra o tráfego.
   - O IP de origem vem de um extrator único que só confia em
     `TRUSTED_PROXIES`, um pré-requisito que corrige a confiança atual em
     qualquer `X-Forwarded-For`.
6. **O host recebe a imagem pelo canal do agente**, nunca chamando a central:
   - comando novo no enum fechado do `agent-protocol`, sob a permissão `deploy`;
   - quadros binários com janela de crédito, na fila de menor prioridade, como no ADR 013;
   - `PROTOCOL_VERSION` sobe;
   - o agente carrega no Docker local e sobe com `--pull never`.
7. **Tag de commit imutável**, com proveniência gravada pela central: a da
   compilação que ela pediu (ADR 023) ou, no modo Actions, a do OIDC do envio
   (repositório, commit, workflow, execução).
8. **Varredura de vulnerabilidades sem binário de terceiros.**
   - Usa o SBOM que o BuildKit anexa à imagem e, sem ele, um extrator
     `dpkg`/`apk` em Rust.
   - Cruza com uma cópia local do OSV.dev, atualizada por rotina.
   - A política é por alvo (avisar, ou bloquear a partir de crítica/alta), com
     exceção registrada.
9. **Attestation Sigstore do GitHub** como etapa posterior, só onde o GitHub a
   oferece: repositório público ou plano Enterprise Cloud.

## Alternativas consideradas

| Alternativa | Por que não |
| :--- | :--- |
| `registry:2` (CNCF distribution) ou Zot como container no compose | Em Go, outro processo para autenticar, monitorar e atualizar. A autenticação por OIDC, a política por área, a proveniência e a ligação com o deploy teriam de ser costuradas por fora. |
| Processo Rust separado só para o registry | Isola uploads grandes, mas duplica autenticação, configuração e observabilidade. O fluxo contínuo (RNF-01) resolve o isolamento de memória dentro da API. |
| CI do GitHub como caminho principal de entrada | Revisto pelo ADR 023: com a compilação no servidor, nada precisa subir pela internet. O caminho continua como modo opcional. |
| `docker push` por uma porta pública própria com TLS | Abre mais uma porta à internet e exige certificado. Recusada na entrevista; volta só se o envio pelo túnel se mostrar inviável (SPIKE-REG-03). |
| Runner do Actions entrando na VPN | Exige a chave de um peer como segredo no repositório. |
| Host fazendo `docker pull` da central | Cada host precisaria alcançar a central e confiar no TLS dela, invertendo o sentido "o agente só disca para fora" (ADR 011). |
| Trivy para a varredura | Binário Go com base própria e atualização própria. O SBOM do BuildKit mais o OSV cobrem o caso das imagens de aplicação, em Rust e offline. |

## Consequências

- Tabelas novas na migration da área Servidores e em `CREATION_ORDER`:
  repositórios, manifests, tags, vínculos de blob, sessões de envio, PATs,
  proveniência, SBOM, achados, exceções e espelho do OSV.
- `/v2` é a segunda superfície pública da central depois do webhook do deploy.
  A regra "pela origem pública só o OIDC" precisa de teste próprio.
- O disco da central passa a guardar imagens. Cota, aviso e retenção não são
  opcionais.
- No modo GitHub Actions, o workflow gerado ganha uma segunda variante, e o
  snapshot `insta` cobre as duas.
- `AGENTS.md` §7 ganha a regra do registry: origem pública só com OIDC, PAT
  só interno, entrega só pelo canal do agente, varredura sem executar
  conteúdo.
