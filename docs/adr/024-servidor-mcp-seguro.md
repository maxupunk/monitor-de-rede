# ADR 024 — Servidor MCP seguro: IDEs e agentes externos sem depender da IA local

## Status

**Aceito** (2026-11-24) e implementado. Guia prático de configuração e uso em
[mcp.md](../mcp.md). Reutiliza o catálogo canônico de ferramentas da IA
(`AiToolHandler` em `netmonitor-core`), o modelo de papéis por área
([ADR 019](019-papel-por-area.md)), o extrator único de origem de rede
([ADR 022](022-registry-proprio.md)) e o fluxo de deploy e compilação
([ADR 021](021-deploy-continuo-pelo-agente.md),
[ADR 023](023-compilacao-no-servidor.md)), respeitando o isolamento modular em
crates ([ADR 017](017-arquitetura-modular.md)) e as fronteiras de acesso a
equipamentos ([ADR 012](012-plugins-de-dispositivo.md)).

## Contexto

Quem desenvolve aplicações hospedadas no NetMonitor já usa agentes dentro da
própria IDE (Antigravity, Cursor, Claude Code, VS Code). Sem uma interface
padronizada, consultar a prontidão de um servidor, disparar uma compilação,
acompanhar logs de deploy ou acionar um rollback exige sair do editor, abrir a
interface web e repetir passos à mão.

O backend já possui um catálogo tipado de ferramentas (`AiToolHandler` em
`netmonitor-core`), com políticas por área e nível de risco, mas esse catálogo
só era alcançável pelo chat interno em `crates/assistant`:

- exigia `AiSettings.enabled = true` e um provedor LLM configurado (Ollama ou
  API externa);
- assumia a presença de um usuário interativo na tela do chat (incluindo
  `ask_user` e o fluxo de confirmação por chamada de `DeviceAccess`);
- dependia de controles que viviam fora do `ToolRegistry`: o registro não
  checa `handler.admin_only()` sozinho, `AreaAccess::writable()` sempre inclui
  `Area::Core` (confiando no `auth_guard` das rotas HTTP) e
  `orchestrator::request_manual` autoaprova alvos com `requires_approval`.

Precisamos expor o NetMonitor como um servidor **MCP (Model Context Protocol)**
para clientes externos com **defesa em profundidade**, **sem exigir a IA local
ligada** e sem abrir brechas sobre equipamentos, segredos ou aprovação em
produção.

## Decisão

### 1. Crate dedicado e desacoplado da IA local (`backend/crates/mcp`)

- Novo crate `netmonitor-mcp` em `backend/crates/mcp`, dependendo **apenas** de
  `netmonitor-core` (ADR 017), sem dependência em `crates/assistant` nem nos
  crates de área (`nucleo`, `servers`, `network`, `vpn`).
- Reutiliza o catálogo global de `AiToolHandler` instalado em `netmonitor-core`
  no boot (`backend/src/registries.rs`).
- Para listar as aplicações de deploy na tela de criação de tokens sem acoplar
  `mcp` a `servers`, `netmonitor-core` declara a trait `McpDeployAppCatalog`
  (`McpDeployAppSummary`), implementada por `ServersDeployAppCatalog` em
  `crates/servers` e ligada na montagem em `backend/src/registries.rs`.
- O servidor MCP **não** consulta `AiSettings.enabled` e funciona com a IA
  local completamente desligada e sem nenhum modelo ou provedor LLM configurado.

### 2. Transporte MCP Streamable HTTP nativo sobre Axum

- Implementação nativa de **MCP Streamable HTTP** com **JSON-RPC 2.0**
  diretamente sobre `axum` e `serde_json` já presentes no workspace, sem
  dependência externa pesada de SDK MCP:
  - `POST /mcp`: aceita requisições JSON-RPC 2.0 nas versões de protocolo
    `2025-06-18`, `2025-03-26` e `2024-11-05` (`initialize`,
    `notifications/initialized`, `ping`, `tools/list`, `tools/call`), em
    mensagem única ou em lote (exigido pela `2025-03-26`; `initialize` não vale
    em lote). Notificação e resposta do cliente recebem `202` sem corpo; o
    cabeçalho `MCP-Protocol-Version`, quando vem, precisa ser uma versão
    aceita (senão `400`);
  - `DELETE /mcp`: encerramento explícito de sessão pelo cliente;
  - `GET /mcp`: responde `405 Method Not Allowed`.
- A rota `/mcp` é montada fora do `business_auth` de sessão JWT da SPA (mesmo
  padrão do `/v2` do registry no ADR 022) e autentica exclusivamente por token
  pessoal MCP.
- A gestão administrativa e de tokens usa rotas REST autenticadas
  (`/api/mcp/overview`, `/api/mcp/settings`, `/api/mcp/tokens`,
  `DELETE /api/mcp/tokens/{id}`) e atualiza a interface em tempo real
  exclusivamente pelo SSE global (`mcp:changed`, sem polling — AGENTS §9).

### 3. Modelo de segurança em camadas

1. **Origem de rede fechada por padrão.**
   - Toda requisição em `/mcp` passa por
     `shared::client_addr::ClientOrigin::is_internal()` (resolvido a partir de
     `TRUSTED_PROXIES`, ADR 022).
   - Por padrão, apenas conexões de origem interna ou VPN WireGuard são aceitas;
     chamadas vindas da borda pública (incluindo Cloudflare Tunnel) são
     recusadas.
   - Um `Origin` de outro site é recusado com `403` (DNS rebinding, exigência
     da especificação). IDEs e terminais não enviam `Origin`; um navegador só
     passa com o da própria central.
   - Um IP que esgotou as tentativas inválidas (20 por minuto) é barrado
     **antes** de o token ser conferido — senão o palpite certo entraria
     depois do limite.
   - Um administrador pode habilitar explicitamente a chave
     `mcp.allow_public_origin` em **Configurações → Inteligência artificial →
     Servidor MCP** caso deseje liberar o acesso via HTTPS externo.

2. **Autenticação por Personal Access Token (`mcp_tokens`).**
   - Emissão, validação de nome/validade e "ainda vale?" são as do
     `shared::personal_token`, as mesmas do token do registry (ADR 022).
   - Formato `nmm_<64-hex>` (32 bytes aleatórios de CSPRNG), enviado em
     `Authorization: Bearer <token>`.
   - Exibido **uma única vez** na criação; o banco armazena apenas o hash
     `sha256` do segredo, o sufixo curto (`hint` dos últimos 4 caracteres), os escopos,
     as aplicações permitidas, `expires_at`, `last_used_at`, `last_used_ip` e
     `revoked_at`.
   - Expiração obrigatória entre `1` e `366` dias, revogação imediata e
     checagem ao vivo de `user.active` em toda requisição.

3. **Autorização por interseção ao vivo.**
   - Tanto `tools/list` quanto `tools/call` recalculam a permissão efetiva no
     momento da chamada:
     ```text
     ferramentas seguras para MCP  ∩  escopos do token  ∩  AreaAccess::of_user(&user)
     ```
   - A interseção de áreas é a mesma do chat (`ToolPolicy::with_access` +
     `admits`), não uma cópia dela. Além disso, a camada MCP impõe:
     - `handler.admin_only()` exige `Role::Admin` na área da ferramenta;
     - ferramentas de escrita em `Area::Core` exigem `access.global().can_write()`
       (um usuário `Viewer` global nunca executa ações no Núcleo).
   - Desativar o usuário ou reduzir seu papel surte efeito imediato em todos os
     tokens já emitidos.

4. **Escopos granulares (`McpScope`) e trava por aplicação de deploy.**

   O escopo de cada ferramenta sai do que **ela declara**
   (`AiToolHandler::mcp_exposure`), nunca do nome: o padrão segue o
   `ToolKind`; as de deploy declaram `DeploysRead`/`DeploysAction`; uma
   consulta que grava declara `Hidden`. O teste do catálogo
   (`ferramenta_de_deploy_declara_a_exposicao_mcp`) barra uma ferramenta de
   deploy nova que esqueça de declarar — ela cairia em `read`.

   | Escopo | O que libera | Exige papel Admin? |
   | :--- | :--- | :---: |
   | `read` | Consultas de leitura (resumo, dispositivos, monitores, alertas, logs, Docker, topologia, auditoria se admin) | Não (`get_audit_log` segue `admin_only`) |
   | `diagnostics` | Testes ativos de rede (`ping_host`, `traceroute`, `dns_lookup`, `scan_ports`, `run_playbook`) | Não (requer acesso à área) |
   | `deploys:read` | Consulta de aplicações, alvos, prontidão (`get_deploy_readiness`), execuções, compilações e logs | Não (requer leitura em Servidores) |
   | `deploys:write` | Disparar compilação (`build`, com `noCache` opcional), publicar (`deploy`), restaurar (`restore`) e cancelar pendentes (`cancel`; o que aguarda aprovação só com `deploys:approve`, porque cancelá-lo é recusá-lo) | Sim (Servidores) |
   | `deploys:approve` | Aprovar ou recusar execuções paradas em `awaiting_approval` | Sim (Servidores) |
   | `docker:write` | Ações em containers via `docker_container_action` (containers protegidos seguem recusados pelo agente) | Sim (Servidores) |
   | `operations:write` | Ações operacionais no Núcleo (silenciar/reconhecer alertas, manutenção, monitores, regras, tarefas agendadas, layout) | Não (requer escrita no Núcleo) |

   - O campo opcional `allowedApplicationIds` no token restringe todas as
     ferramentas de deploy (`get_deploys`, `get_deploy_readiness`,
     `get_deploy_log`, `get_builds`, `get_build_log`, `deploy_action`) somente
     às aplicações de deploy daquele projeto. A verificação é aplicada dentro
     dos próprios serviços/ferramentas via `ToolArgs.mcp: Option<McpToolContext>`.

5. **Exclusões rígidas (nunca expostas ao MCP).**
   - `ask_user` (`ToolKind::Interactive`): exclusiva do chat interativo.
   - Todas as ferramentas `ToolKind::DeviceAccess` (`fingerprint_device`,
     `device_ssh_exec`, `device_http_request`, `save_plugin_draft`,
     `run_plugin_action`, `validate_plugin`): o ADR 012 exige aprovação humana
     por chamada e não admite acesso headless de agentes externos.
   - O que declara `McpExposure::Hidden`, como `run_plugin_tests` (grava o
     resultado e move o plugin entre rascunho e testado).
   - Segredos e credenciais: variáveis `.env` (`env_versions`), segredos de
     build (`build-secrets`), `deploy_credentials`, chaves privadas do GitHub
     App e ações de plugin com `reveals: true`.

6. **Aprovação humana preservada nos deploys (ADR 021).**
   - Execuções via MCP trafegam `McpToolContext` com
     `respect_target_approval = true`.
   - Ao disparar um deploy manual (`deploy_action` com `action: "deploy"`),
     quando `target.requires_approval` está ativo, o orquestrador coloca a
     execução em `awaiting_approval` em vez de autoaprovar — mantendo o portão
     humano na tela do NetMonitor.
   - Aprovar ou recusar via MCP exige o escopo dedicado `deploys:approve`
     (`can_approve_deploys = true`), separado de `deploys:write`, além de papel
     `Admin` em Servidores (verificado também dentro de `deploy_action` como
     defesa em profundidade).

7. **Rate limit e auditoria completa.**
   - `SlidingWindowRateLimiter` aplicado por token e por IP de origem no
     endpoint `/mcp`.
   - Toda criação/revogação de token, alteração de configuração e invocação de
     ferramenta é registrada no `AuditService` usando o marcador `VIA_MCP` e os
     tipos fechados `ResourceType::McpToken` e `ResourceType::McpTool`.

## Alternativas consideradas

| Alternativa | Por que não |
| :--- | :--- |
| Adotar um crate externo de SDK MCP (ex.: `rmcp`) | Traz árvore de dependências extra com risco de conflito frente a `loco-rs 1.0` / `axum 0.8` e dificulta encaixar exatamente `client_addr`, PAT, `AreaAccess`, rate limit e auditoria. O subconjunto servidor de Streamable HTTP JSON-RPC 2.0 é pequeno, estável e fácil de testar direto no Axum. |
| Hospedar o servidor MCP dentro de `crates/assistant` | Mistura agentes externos com o ciclo da IA local (`AiSettings.enabled` e provedores LLM) e viola o objetivo de usar o MCP sem IA local configurada. |
| Autenticar clientes MCP com JWT de login ou cookie de sessão | Sessão interativa carrega todas as permissões do usuário, não permite escopo por projeto nem trava em `allowedApplicationIds` e não oferece revogação isolada por estação de trabalho. |
| Expor transporte `stdio` ou abrir uma porta TCP dedicada | `stdio` não atende uma central remota compartilhada pela equipe; porta dedicada duplica configuração de firewall, TLS e proxy reverso. Streamable HTTP em `/mcp` herda TLS, `TRUSTED_PROXIES` e `shared::client_addr`. |
| Liberar `DeviceAccess` ou autoaprovar deploys por padrão no MCP | Quebra invariantes de segurança já firmados: o ADR 012 exige confirmação humana por chamada em equipamento e o ADR 021 protege produção com `requires_approval`. |

## Consequências

- **Estrutura de crates:** `backend/crates/mcp` (`netmonitor-mcp`) passa a
  compor o workspace (`Cargo.toml`, `app.rs`, `migrator` e guarda arquitetural
  `tests/conventions/architecture.rs`), dependendo exclusivamente de `core`.
- **Banco de dados:** nova tabela `mcp_tokens` (registrada em `tables.rs`,
  compatível com PostgreSQL 18 e SQLite, escrita via
  `shared::transaction::begin_write`, entidade alinhada ao PostgreSQL).
- **Contratos no núcleo:** `ToolArgs` ganha `mcp: Option<McpToolContext>`
  (padrão `None` no chat interno e nas tarefas agendadas, sem regressão);
  `AuditService` ganha `VIA_MCP`, `ResourceType::McpToken` e
  `ResourceType::McpTool`; `crates/servers` ganha `get_deploy_readiness`,
  `action: "cancel"`, `noCache` em `action: "build"` e validação de
  `McpToolContext` nas ferramentas de deploy.
- **Interface (Flutter):** cartão **Servidor MCP (Agentes externos)** em
  **Configurações → Inteligência artificial**, sempre visível mesmo com a IA
  local desligada, oferecendo controles administrativos, emissão de tokens com
  predefinições e trava por aplicação, exibição única com snippets prontos
  (`mcp.json` e Claude Code) e atualização ao vivo via SSE `mcp:changed`.
- **Risco assumido:** se um administrador ativar `mcp.allow_public_origin` sem
  HTTPS adequado na borda ou emitir um token combinando `deploys:write` e
  `deploys:approve` sem restringir aplicações, o vazamento desse token na
  máquina do desenvolvedor permite publicar em produção. As mitigações são o
  bloqueio de origem pública por padrão, a separação de `deploys:approve`, a
  trava `allowedApplicationIds`, a validade obrigatória, o rate limit e a
  auditoria completa.
