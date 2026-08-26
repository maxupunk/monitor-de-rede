# NetMonitor

Monitoramento de rede com backend em **Rust (Loco.rs)** e frontend em **Vue 3 +
Vuetify**. A stack inteira é **um container**: API, interface web, ciclo de
monitores, banco (SQLite) e servidor WireGuard.

## Principais recursos

| Área | Recursos e funções principais |
| :--- | :--- |
| **Dashboard** | Painel personalizável, widgets de disponibilidade, tráfego, latência, eventos, alertas e serviços SaaS, com atualização em tempo real. |
| **Inventário** | Organização por sites e redes, cadastro de dispositivos, identificação assistida de nome, sistema, fabricante, modelo e forma de acesso, interfaces e relacionamentos entre equipamentos. |
| **Descoberta de rede** | Varredura de CIDRs IPv4 e IPv6, descoberta por ICMP, TCP e SNMP, histórico e cancelamento de varreduras e conversão dos resultados em dispositivos monitorados. |
| **Monitoramento** | Checagens de ICMP, HTTP, TCP, DNS e SNMP, execução manual ou agendada, disponibilidade, latência, perda, métricas, histórico e consolidação horária. |
| **SNMP** | Teste de credenciais, coleta de interfaces e saúde, leitura de tráfego e estado operacional e criação assistida de monitores. |
| **Syslog** | Servidor UDP/TCP, ativação automática por acesso ao equipamento, identificação por hostname, associação isolada por dispositivo, filtros, acompanhamento em tempo real e retenção por idade e tamanho. |
| **Alertas** | Catálogo de regras, regras personalizadas, histerese, recuperação, detecção de instabilidade, correlação, análise de causa raiz, reconhecimento, silêncio e janelas de manutenção. |
| **Notificações** | Web Push, e-mail, Telegram, Discord e webhooks, respeitando silêncios, correlação e política de recuperação. |
| **Topologia** | Mapa de dispositivos e enlaces, vínculos automáticos ou manuais e nível de confiança da relação encontrada. |
| **VPN WireGuard** | Servidor e peers, geração de configurações e QR Code, rotação de chaves, dicas de firewall, telemetria do túnel e acesso a redes remotas. |
| **Probes remotos** | Agentes autenticados para monitorar outros sites, fila persistente, heartbeat, buffer quando o servidor está indisponível e retomada automática. |
| **Ferramentas de rede** | Scanner de portas, consultas e benchmark DNS e diagnóstico complementar quando ICMP é filtrado. |
| **Operação e segurança** | Usuários com perfis `admin`, `operator` e `viewer`, auditoria, backup e restauração das configurações, primeiro acesso protegido e segredos cifrados em repouso. |
| **Plataforma** | SPA instalável como PWA, eventos via SSE, SQLite por padrão, PostgreSQL opcional e automonitoramento do próprio servidor. |

Para detalhes de componentes, fluxos e decisões técnicas, consulte
[`docs/arquitetura.md`](docs/arquitetura.md). O estado das entregas e dos itens
planejados está em [`docs/roadmap.md`](docs/roadmap.md).

## Estrutura

| Diretório | O que é | Toolchain |
| :--- | :--- | :--- |
| `backend/` | API, scheduler, probe e migrations | `cargo` |
| `frontend/` | SPA Vue 3, servida pela própria API em produção | `npm` |
| `docker/` | Entrypoint e watcher do WireGuard | — |
| `docs/` | Arquitetura, roadmaps e ADRs | — |

O `package.json` da raiz existe **só** para atalhos do frontend. Não há comando
de backend em npm: o backend é `cargo`, rodado de dentro de `backend/`.

## Subir a stack

```powershell
cp .env.example .env   # ajuste os segredos
docker compose up -d --build
```

Interface e API na mesma porta: <http://localhost:3333>. A UDP 51820 é do
WireGuard e só importa a quem usa a VPN.

### Usar a rede do host

O modo padrão usa a bridge do Docker e publica portas do container no host. O
modo host remove essa tradução: API, SPA, Syslog e WireGuard usam diretamente a
pilha de rede da máquina. Ele é especialmente útil para preservar o IP de origem
dos equipamentos que enviam Syslog.

#### 1. Ative o override no `.env`

Copie `.env.example` para `.env`, configure `JWT_SECRET` e `ENCRYPTION_KEY` e
descomente/ajuste estas variáveis:

```dotenv
COMPOSE_PATH_SEPARATOR=:
COMPOSE_FILE=docker-compose.yml:docker-compose.host.yml
APP_PORT=3334
SYSLOG_LISTEN_PORT=5514
```

`COMPOSE_PATH_SEPARATOR=:` permite usar a mesma declaração de `COMPOSE_FILE` no
Windows e no Linux. O arquivo principal continua definindo volumes, segurança e
variáveis; [`docker-compose.host.yml`](docker-compose.host.yml) apenas troca a
rede e remove as publicações incompatíveis.

#### 2. Escolha as portas

- `APP_PORT` é a porta real da interface web e da API. Escolha uma porta livre
  no host; `APP_EXTERNAL_PORT` não é usada neste modo.
- `SYSLOG_LISTEN_PORT` é simultaneamente a porta escutada e a porta que deve ser
  informada aos equipamentos. Use uma porta livre e não privilegiada
  (`>= 1024`), pois a aplicação roda sem capabilities. `SYSLOG_EXTERNAL_PORT`
  não é usada no modo host.
- Se a VPN estiver habilitada, a porta configurada em **VPN → Servidor → Porta
  UDP** também precisa estar livre no host. No modo host não existe mapeamento
  para corrigir uma diferença entre porta interna e externa.

Exemplo: com `APP_PORT=3334` e `SYSLOG_LISTEN_PORT=5514`, a interface fica em
`http://IP-DO-HOST:3334` e o roteador deve enviar Syslog para
`IP-DO-HOST:5514`. O campo “Endereço que o equipamento vai usar” recebe o IP ou
hostname do **host Docker**, nunca `localhost` nem o IP interno do container.

#### 3. Confira e inicie

```powershell
docker compose config
docker compose up -d --build
docker compose ps
curl.exe -fsS http://localhost:3334/_health
```

No resultado de `docker compose config`, o serviço `netmonitor` deve ter
`network_mode: host` e não deve ter um bloco `ports`. O healthcheck deve devolver
`{"ok":true}`. O volume `netmonitor-data` é o mesmo nos dois modos, portanto a
troca de rede não apaga configurações nem históricos.

Libere no firewall do host somente o necessário para as redes que usarão o
sistema:

- TCP `APP_PORT` para a interface/API;
- UDP e, se utilizado, TCP `SYSLOG_LISTEN_PORT` para Syslog;
- UDP da porta configurada no servidor WireGuard, quando a VPN estiver ativa.

#### Voltar ao modo bridge

Comente `COMPOSE_FILE`, ajuste as portas publicadas e recrie o serviço:

```dotenv
# COMPOSE_FILE=docker-compose.yml:docker-compose.host.yml
APP_EXTERNAL_PORT=3333
APP_PORT=3333
SYSLOG_EXTERNAL_PORT=514
SYSLOG_LISTEN_PORT=5514
```

```powershell
docker compose up -d --force-recreate
```

Na bridge, `APP_EXTERNAL_PORT` é a porta acessada pelo navegador e
`SYSLOG_EXTERNAL_PORT` é a porta usada pelos equipamentos; elas são traduzidas
para `APP_PORT` e `SYSLOG_LISTEN_PORT` dentro do container. Se o ambiente
mascarar os IPs de origem, o NetMonitor ainda associa os logs pela identidade do
equipamento obtida durante a ativação automática, mas o modo host preserva mais
informação de rede para diagnóstico.

### Primeiro acesso

O servidor aplica as migrations pendentes ao subir. Banco novo não tem usuário,
e é o próprio sistema que pede o cadastro: abra o endereço acima e ele leva a
`/setup`, onde se informa nome, e-mail, senha e o **token de instalação**.

O token sai no log do boot — a linha "instalação pendente":

```powershell
docker compose logs netmonitor | Select-String setup_token
```

Se a linha já rolou para fora do terminal:

```powershell
docker compose exec netmonitor backend-cli task auth_setup_token
```

Para fixá-lo de antemão (provisionamento automatizado), defina `SETUP_TOKEN` no
`.env`. Concluído o cadastro o token deixa de ser aceito, e novos usuários
passam a ser criados por quem já está autenticado.

## Backend — comandos (`cd backend`)

```powershell
cargo run --bin backend-cli -- start      # sobe a API
cargo run --bin backend-cli -- db migrate # aplica migrations
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Tarefas de CLI: `task auth_setup_token`, `task user:create`, `task probe_register`,
`task vpn_probe_register`, `task probe_run` (agente de um probe remoto),
`task scheduler_loop` (o ciclo em processo próprio, quando não se quer o
in-process) e `task scheduler_run` (um ciclo só, para depurar).

### Probe remoto

É o único caso que pede um segundo container — um agente onde o servidor não
alcança. Mesma imagem, outro comando:

```yaml
services:
  probe:
    image: netmonitor:latest
    command: ["backend-cli", "task", "probe_run"]
    environment:
      PROBE_SERVER_URL: http://IP-DO-SERVIDOR:3333
      PROBE_TOKEN: <token gerado por `task probe_register`>
    sysctls:
      net.ipv4.ping_group_range: "0 2147483647"
    restart: unless-stopped
```

## Frontend — comandos

```powershell
npm --prefix frontend run dev
npm --prefix frontend run typecheck
npm --prefix frontend run lint
npm --prefix frontend run build
```

Os mesmos atalhos existem na raiz como `npm run dev:frontend`, `build:frontend`,
`typecheck:frontend`, `lint:frontend` e `format:frontend`.

## Configuração

`.env.example` lista todas as variáveis lidas — e só elas. A configuração viva
do backend está em `backend/config/{development,test,production}.yaml`;
o `.env` apenas preenche os `get_env(...)` desses arquivos e as substituições do
compose. O banco é apontado por **`DATABASE_URL`** (uma URL só, não campos
separados): o padrão é o SQLite do volume, e apontar a variável para um
Postgres é o que basta para trocar — o código atende aos dois dialetos.
