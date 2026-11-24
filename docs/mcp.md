# Servidor MCP — guia de configuração e uso

> **Status:** implementado (2026-11-24). Decisão técnica e modelo de segurança
> em [ADR 024](adr/024-servidor-mcp-seguro.md).  
> **Independe da IA local:** funciona mesmo com a Inteligência artificial do
> NetMonitor desligada e sem Ollama ou provedor LLM configurado.

## 1. Em uma frase

Conecte o agente da sua IDE (**Antigravity**, **Cursor**, **Claude Code** ou
**VS Code**) direto ao NetMonitor por **MCP Streamable HTTP** (`/mcp`) para
consultar o ambiente e disparar, acompanhar ou reverter deploys de dentro do
projeto — com token escopado por projeto, bloqueio de origem pública por padrão
e aprovação humana preservada.

```text
IDE / Agente (Antigravity, Cursor, Claude Code, VS Code)
   │  POST /mcp  (JSON-RPC 2.0 + Authorization: Bearer nmm_...)
   ▼
NetMonitor (crates/mcp)
   ├─ 1. Origem de rede: rede interna/VPN (ou HTTPS externo se liberado)
   ├─ 2. Token pessoal: sha256 + validade + não revogado + usuário ativo
   ├─ 3. Autorização ao vivo: catálogo seguro ∩ escopos ∩ papel do usuário
   ├─ 4. Trava por projeto: allowedApplicationIds (quando definida)
   └─ 5. Rate limit + Auditoria (VIA_MCP)
   ▼
Ferramentas canônicas (leitura, diagnóstico, deploys, builds, Docker, operações)
   └─ Deploys respeitam target.requires_approval (ADR 021 / ADR 023)
```

---

## 2. Pré-requisitos

- **Acesso de rede ao NetMonitor:** por padrão, o endpoint `/mcp` aceita apenas
  conexões vindas da **rede interna** ou da **VPN WireGuard** do NetMonitor.
  Conexões via internet/Cloudflare Tunnel só passam se um administrador ligar a
  chave específica (veja §3).
- **Conta ativa com o papel adequado:** o token nunca faz mais do que o seu
  usuário pode fazer no momento da chamada.
  - Para consultar saúde, alertas, logs e status de deploy: conta ativa com
    leitura na área correspondente.
  - Para compilar, publicar, cancelar ou reverter deploys (`deploys:write` /
    `deploys:approve`): papel **Admin** na área **Servidores**.
- **Para fluxos de deploy:** aplicação e alvos já configurados em **Deploys**
  (`/deploys`), com agente conectado e permissões `deploy` / `build` liberadas
  (veja [compilacao_no_servidor.md](compilacao_no_servidor.md)).
- **Nenhuma IA local necessária:** você não precisa ativar o chat interno nem
  instalar modelos no Ollama.

---

## 3. Ativar e configurar o Servidor MCP

1. No NetMonitor, abra **Configurações → Inteligência artificial**.
2. Localize o cartão **Servidor MCP (Agentes externos)** — ele fica disponível
   mesmo com a IA local desativada.
3. Para administradores, o painel de configuração oferece dois interruptores:
   - **Ativar servidor MCP:** liga ou desliga o endpoint `/mcp` para toda a
     instância.
   - **Permitir conexões fora da rede interna/VPN:** desligado por padrão.
     Só ative se a equipe precisar conectar IDEs fora da LAN/VPN através de um
     proxy reverso com **HTTPS** e `TRUSTED_PROXIES` configurado corretamente.
4. O botão **Como configurar** abre a qualquer momento o endereço do endpoint
   (`/mcp`) e os modelos de configuração prontos para copiar, avisando quando a
   URL atual parece pública com a chave de origem externa desligada.

---

## 4. Criar um token escopado por projeto

A forma recomendada de usar o MCP é emitir **um token dedicado por projeto**,
com o mínimo de escopos necessários e travado apenas na aplicação daquele
repositório.

1. No cartão **Servidor MCP (Agentes externos)**, clique em **Novo token**.
2. Preencha um nome claro que identifique ferramenta e projeto, por exemplo:
   `Antigravity — sgc-web`.
3. Escolha uma predefinição ou marque os escopos manualmente:
   - **Deploy de projeto** *(recomendada para desenvolvimento)*: marca
     `deploys:read` + `deploys:write`.
   - **Somente leitura**: marca `read` + `deploys:read`.
   - **Personalizado**: combina qualquer subconjunto permitido pelo seu papel.

### Escopos disponíveis

| Escopo | O que permite | Exige Admin? |
| :--- | :--- | :---: |
| `read` | Consultas somente leitura: resumo do sistema, equipamentos, monitores, alertas, logs, hosts/containers Docker, topologia e auditoria (esta última só para admin) | Não |
| `diagnostics` | Testes ativos de rede: `ping_host`, `traceroute`, `dns_lookup`, `scan_ports` e `run_playbook` | Não (acesso à área) |
| `deploys:read` | Listar aplicações, alvos e execuções (`get_deploys`), checar prontidão (`get_deploy_readiness`), consultar compilações (`get_builds`) e ler logs (`get_deploy_log`, `get_build_log`) | Não (leitura em Servidores) |
| `deploys:write` | Disparar compilação (`build`, com opção `noCache`), publicar (`deploy`), restaurar versão anterior (`restore`) e cancelar execuções/compilações pendentes (`cancel`; execução em **Aguarda aprovação** só com `deploys:approve`, porque cancelá-la é recusá-la) | Sim (Servidores) |
| `deploys:approve` | Aprovar ou recusar execuções paradas em **Aguarda aprovação** (`awaiting_approval`) | Sim (Servidores) |
| `docker:write` | Iniciar, parar ou reiniciar containers via `docker_container_action` (containers protegidos seguem bloqueados pelo agente) | Sim (Servidores) |
| `operations:write` | Ações no Núcleo: reconhecer/silenciar alertas, criar janela de manutenção, monitores, regras de alerta, tarefas agendadas e layout do dashboard | Não (escrita no Núcleo) |

4. Em **Restringir a aplicações de deploy** (exibido sempre que um escopo de
   deploy está selecionado), desmarque "Todas as aplicações permitidas" e
   selecione **apenas a(s) aplicação(ões) daquele projeto**. O servidor
   bloqueará qualquer consulta ou ação voltada a outra aplicação.
5. Defina a **Validade** (`30`, `90`, `180` ou `365` dias; a API exige entre
   `1` e `366` dias).
6. Confirme a criação. O token (`nmm_...`) e os snippets prontos aparecem no
   banner de sucesso **uma única vez**. Copie imediatamente — o banco armazena
   apenas o hash `sha256` e não há como exibir o segredo depois.

---

## 5. Configurar sua IDE ou agente

Substitua `https://netmonitor.sua-rede.local/mcp` pelo endereço interno/VPN da
sua central (por exemplo `http://10.8.0.1:3333/mcp` na VPN WireGuard ou o
domínio HTTPS interno) e `nmm_SEU_TOKEN_AQUI` pelo segredo exibido na criação.

> **Importante:** nunca faça commit de arquivos com token `nmm_...` no
> repositório Git. Use o arquivo de configuração global da IDE ou adicione o
> arquivo local ao `.gitignore`.

### Antigravity e Cursor (`mcp.json`)

```json
{
  "mcpServers": {
    "netmonitor": {
      "url": "https://netmonitor.sua-rede.local/mcp",
      "headers": {
        "Authorization": "Bearer nmm_SEU_TOKEN_AQUI"
      }
    }
  }
}
```

### VS Code (`.vscode/mcp.json`)

```json
{
  "servers": {
    "netmonitor": {
      "type": "http",
      "url": "https://netmonitor.sua-rede.local/mcp",
      "headers": {
        "Authorization": "Bearer nmm_SEU_TOKEN_AQUI"
      }
    }
  }
}
```

### Claude Code (CLI)

```bash
claude mcp add --transport http netmonitor https://netmonitor.sua-rede.local/mcp \
  --header "Authorization: Bearer nmm_SEU_TOKEN_AQUI"
```

Para conferir se a conexão está ativa no Claude Code:

```bash
claude mcp list
```

---

## 6. Fluxo típico de deploy direto do projeto

Com o token configurado (`deploys:read` + `deploys:write` + aplicação do
projeto), você pode conduzir todo o ciclo sem sair do editor:

1. **Conferir prontidão antes de publicar**
   - O agente chama `get_deploys` e `get_deploy_readiness`.
   - Verifica se o agente de build e o agente de destino estão conectados, com
     permissões `build`/`deploy`, espaço em disco e qual revisão está no ar.
2. **Compilar a nova versão**
   - Após o `git push`, o agente chama `deploy_action` com `action: "build"`
     (podendo informar `branch`, `commitSha` ou `noCache: true`).
   - A compilação roda no builder BuildKit isolado do agente (ADR 023).
3. **Acompanhar o build e inspecionar logs**
   - O agente usa `get_builds` e `get_build_log` para acompanhar o andamento ou
     diagnosticar uma falha no Dockerfile ou no estágio de teste.
4. **Publicar e acompanhar o deploy**
   - O agente chama `deploy_action` com `action: "deploy"` (e acompanha via
     `get_deploys` e `get_deploy_log`).
   - **Homologação (sem aprovação):** entra na fila e sobe direto.
   - **Produção (`requires_approval = true`):** entra em **Aguarda aprovação**
     (`awaiting_approval`) no NetMonitor. A aprovação continua sendo feita por
     uma pessoa na tela **Deploys** (ou explicitamente por um token separado
     que possua `deploys:approve` via `action: "approve"`).
   - Se precisar interromper antes de iniciar, o agente pode chamar
     `deploy_action` com `action: "cancel"` (para deploy pendente ou build na
     fila/em andamento).
5. **Voltar versão anterior (rollback)**
   - Se a versão nova falhar após subir, peça ao agente para reverter: ele
     chama `deploy_action` com `action: "restore"`, acionando o mesmo fluxo de
     rollback auditado da tela **Deploys**.

### Exemplos de pedidos na IDE

- *"Confira se o servidor de homologação do `meu-app` está pronto para deploy."*
- *"Dispare a compilação da branch `main` do `meu-app` e me diga se passou nos testes."*
- *"Mostre o log da última falha de build do `meu-app`."*
- *"Suba a versão compilada do `meu-app` em homologação e acompanhe até ficar no ar."*
- *"Abra o deploy de produção do `meu-app` para eu aprovar no NetMonitor."*
- *"Volte o `meu-app` para a versão anterior."*

---

## 7. O que o servidor MCP nunca expõe

Independentemente dos escopos marcados no token ou do papel do usuário, o
servidor MCP **nunca** libera:

- **Variáveis de ambiente e segredos:** `.env` das aplicações (`env_versions`),
  segredos de build (`build-secrets`), credenciais de deploy
  (`deploy_credentials`) e chaves privadas do GitHub App.
- **Acesso direto a equipamentos de rede (`DeviceAccess`):** SSH, HTTP e
  execução de plugins em roteadores/switches continuam exclusivos da interface
  com confirmação humana por chamada ([ADR 012](adr/012-plugins-de-dispositivo.md)).
- **Revelação de senhas (`reveals: true`)** e **perguntas interativas do chat
  (`ask_user`)**.
- **Testes de plugin (`run_plugin_tests`):** gravam o resultado e movem o
  plugin entre rascunho e testado — o passo antes da revisão do ADR 012 — e
  não podem caber no escopo `read`.
- **Containers protegidos** (`com.netmonitor.protected=true`), sempre recusados
  pelo agente.

---

## 8. Boas práticas de segurança

- **Separe publicação de aprovação em produção:** em tokens usados no dia a dia
  da IDE, libere `deploys:read` + `deploys:write` e **não** inclua
  `deploys:approve`. Assim, o agente pode compilar, subir em homologação e
  preparar o deploy de produção, mas a aprovação final permanece humana no
  NetMonitor.
- **Trave cada token na aplicação do projeto:** sempre use
  `Restringir a aplicações de deploy`. Se um token vazar de uma estação, o
  alcance fica limitado àquele único projeto.
- **Mantenha o acesso na rede interna ou VPN:** deixe **Permitir conexões fora
  da rede interna/VPN** desligado sempre que possível. Caso precise ligar,
  exija HTTPS ponta a ponta no proxy reverso, confira `TRUSTED_PROXIES` e use
  tokens de validade mais curta (30 dias).
- **Rotacione e revogue sem hesitar:** acompanhe as colunas **Último uso** e
  **IP** no cartão do Servidor MCP. Se trocar de máquina, encerrar um projeto
  ou suspeitar de exposição do segredo, clique em revogar imediatamente — o
  bloqueio é instantâneo.
- **Acompanhe a trilha de auditoria:** toda emissão/revogação de token
  (`McpToken`) e toda execução de ferramenta via agente externo (`McpTool`,
  marcador `VIA_MCP`) ficam registradas em **Auditoria** no NetMonitor.
