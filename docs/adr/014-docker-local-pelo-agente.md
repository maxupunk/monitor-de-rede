# ADR 014 — Docker da central atendido por um agente local

## Status

**Aceito** (2026-10-15) e implementado. Substitui a parte do ADR 010 que
montava o `docker.sock` no processo da API; o restante do 010 (bollard como
única fronteira, sem CLI na API, mutações administrativas e auditadas) vale.

## Contexto

Até aqui o Docker tinha dois caminhos:

- **o da central** — o processo da API montava `/var/run/docker.sock` e falava
  com a Engine pelo `bollard` (`LocalEngine`);
- **o dos servidores remotos** — o agente (ADR 011) monta o socket no host
  dele e a central o comanda pelo WebSocket (`AgentEngine`).

O código de acesso já era um só: o agente executa o **mesmo** `LocalEngine`, e
os dois traits (`DockerEngine`, `DockerMaintenance`) escondem a origem. O que
sobrava eram as diferenças do lado da central, e a mais grave delas era de
segurança:

| Aspecto | Central (antes) | Agente remoto |
|---|---|---|
| Quem segura o `docker.sock` | o processo da API, exposto à rede | o agente, que só disca para fora |
| Política | fixa no código (`LOCAL_PERMISSIONS`) | `AGENT_ALLOW`, soberana, decidida no host |
| Guarda de containers | nenhuma (só `update` recusava o próprio) | recusa ações sobre o próprio container |
| Compose | impossível: a API não tem CLI (§7) | disponível com o CLI do agente |
| Exportar volume | só aqui, pelo socket | indisponível: não cabia em JSON |
| Tempo real | coletor próprio de 3 s + cache de métricas + `refresh()` após mutação | `SetLive` + eventos do agente |
| Histórico | amostrador próprio gravando `local` | rollups do agente pelo canal |
| Rotas | `/docker/...` e `/docker/hosts/{host}/...` | só a segunda |

O socket da Engine equivale a root no host (o próprio ADR 010 diz isso). Com
ele na API, uma falha de execução de código ou de SSRF na API virava root na
máquina. O §7 do AGENTS.md já tinha tirado o `wg` da API pelo mesmo motivo; o
Docker tinha ficado para trás.

## Decisão

**Todo host Docker é um agente.** O desta central é o **agente local**: o
serviço `netmonitor-agent` do `docker-compose.yml`, um container ao lado da
API, com a mesma imagem e o mesmo binário dos agentes remotos.

1. **A API não monta o socket.** Quem monta é o agente local, sob
   `LOCAL_AGENT_ALLOW` (padrão `read,lifecycle,update,compose,volume_export,monitor`
   — `monitor` porque os monitores de container desta central passam a ser
   despachados a ele). A central só usa a política que ele anuncia para
   esconder botões; quem decide, a cada pedido, é o agente.
2. **Registro zero-config, sem segredo conhecido.** No boot a API cadastra (ou
   reaproveita) o agente local — `probes.configuration = {role: "agent",
   local: true}` — e grava o token dele em `LOCAL_AGENT_TOKEN_FILE`, num volume
   que só os dois containers montam. O agente lê o arquivo
   (`AGENT_TOKEN_FILE`) a cada tentativa de conexão. Token válido é mantido
   entre boots; arquivo perdido gera outro e o antigo para de autenticar. Não
   existe token padrão: diferente do `vpn-probe` (§6), o token dá controle do
   Docker, e um valor conhecido seria uma porta aberta.
3. **Uma chave só.** A variante `HostKey::Local` e o alias `local` saem; todo
   host é `agent-<id>`. As rotas sem `{host}` saem. Monitores de container sem
   probe e o histórico gravado como `local` são adotados pelo agente local na
   primeira subida.
4. **Containers protegidos.** A label `com.netmonitor.protected=true` (posta
   no compose na API e no agente) faz o agente recusar parar, reiniciar,
   recriar, remover ou desconectar de rede; o projeto compose que contém um
   container protegido — ou o próprio agente — só aceita `pull`. A regra é
   pura (`docker/protection.rs`) e a central a usa para marcar a tela.
5. **Exportar volume pela ponte.** `Command::VolumeExport { name }`, com a
   permissão `volume_export` fora do padrão dos remotos, leva o tar pelos
   quadros binários da ponte de banco (ADR 013) — mesma janela de crédito,
   mesma fila de menor prioridade, mesmo limite de 2 pontes por agente. A
   central fecha a escrita do seu lado na hora e lê até o agente fechar o
   dele. `PROTOCOL_VERSION` sobe para **3**.
6. **O que era só da central sai.** O coletor de 3 s, o cache de métricas, o
   `refresh()` pós-mutação e o amostrador da central: o agente local faz tudo
   isso pelo caminho dos remotos (`SetLive`, eventos, rollups, `TELEMETRY_ENABLED`
   no ambiente dele). O compose também deixa de ser "remoto": o `LocalEngine`
   implementa `compose()` e o executor só chama o trait.

```
navegador ──HTTP/SSE──▶ API (sem socket, sem caps)
                           │  WS (canal dos agentes)
                           ▼
                     netmonitor-agent ──bollard──▶ /var/run/docker.sock
                     (AGENT_ALLOW local, guarda, compose CLI)
```

## Consequências

**Ganhos**

- Um comprometimento da API não vira root no host: só alcança o que o agente
  local aceita, nunca um container protegido e nunca uma operação fora do enum
  fechado do protocolo.
- Um caminho só para todo host. Some o que era especial da central no
  backend (variante local, permissões fixas, rotas duplicadas, coletor, cache,
  amostrador) e no frontend (`LOCAL_HOST_KEY`, `kind === 'local'` como regra).
- Compose e exportação de volume passam a existir em qualquer host com agente,
  o desta central inclusive.

**Custos e riscos aceitos**

- **Mais um container.** A instalação padrão sobe dois serviços em vez de um.
  Quem não quer o Docker gerenciado remove o serviço; a tela fica só com os
  remotos.
- **Ponto de falha novo.** Agente local fora do ar = Docker desta central
  indisponível na tela (o histórico continua, é do banco). Monitores de
  container caem na execução local de reserva (§6), que responde que o Docker
  daqui é do agente e reporta `unknown`.
- **Desenvolvimento fora do Docker.** Sem `LOCAL_AGENT_TOKEN_FILE` não há
  agente local; para ver o Docker da máquina, sobe-se o binário
  `netmonitor-agent` apontando para a API (`AGENT_SERVER_URL`,
  `AGENT_ENROLL_CODE` de um agente cadastrado na tela).
- **Latência de um salto.** Cada consulta atravessa o WebSocket local; no
  loopback o custo é desprezível perto do da Engine.
- **Agente e central sobem juntos** (handshake estrito, ADR 011): a versão 3 do
  protocolo exige atualizar os agentes remotos junto com a central.

## Como ficou

- `services/agents/local.rs` — cadastro idempotente, token no arquivo,
  adoção de monitores e histórico, recusa de revogar/remover/reinstalar.
- `services/docker/hosts.rs` — `HostKey(i64)`, `resolve`, `default_key`,
  `default_host`, `list` com o agente local primeiro (`kind: "local"`).
- `services/docker/protection.rs` — label, inspeção e "próprio container".
- `services/agents/bridge.rs` — `OpenTunnel` genérico e `download_volume`;
  `services/agent_runtime/{session,tunnel}.rs` — a ponta do volume.
- `services/shared/secret_file.rs` — leitura e escrita 0600 do token, usada
  pela central e pelo agente.
- `docker-compose.yml` / `docker-compose.host.yml` — serviço
  `netmonitor-agent`, volume `netmonitor-local-agent`, labels de proteção; o
  entrypoint da API deixa de tratar o GID do socket.
