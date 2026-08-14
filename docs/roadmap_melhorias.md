# Roadmap de Melhorias — Alertas de VPN, Reuso de UI e Descoberta por Bloco de IP

> **Status geral:** 🟡 Em andamento
> **Data:** Agosto de 2026
> **Escopo:** cinco frentes solicitadas — alerta de queda de túnel VPN, reuso do item de
> monitor entre telas, atalho de monitoramento no card de DNS, padronização do scroll
> infinito e descoberta de equipamentos por faixa CIDR cadastrada em `/networks`.

---

## 📊 Visão geral

| # | Item | Status |
| :---: | :--- | :---: |
| 1 | Alerta ao desconectar a VPN | ⬜ Pendente |
| 2 | Componente único do item de monitor (`/monitors` e `/devices/:id`) | ⬜ Pendente |
| 3 | "Adicionar ao Monitoramento" no card *Latência de DNS ▸ Comparar* | ⬜ Pendente |
| 4 | Scroll infinito com paginação no backend | 🟡 Parcial |
| 5 | Descoberta por bloco de IP a partir de `/networks` | ⬜ Pendente |

Legenda: ⬜ pendente · 🟡 parcial · 🟢 concluído

---

## 1. Alerta ao desconectar a VPN

### 1.1. Situação atual

O estado do túnel (`connected` · `unstable` · `disconnected` · `awaiting`) é **derivado em
tempo de leitura** pelo getter `connectionStatus` de [`VpnPeer`](../backend/src/models/vpn_peers.rs),
a partir de `lastHandshakeAt`/`lastSeenAt`. O [`VpnTrafficRecorder`](../backend/src/services/vpn/traffic_recorder.rs)
já sincroniza a telemetria a cada 10 s e publica `vpn:peers_updated` no SSE.

Como o status nunca é **persistido**, não existe "estado anterior" para comparar — logo não
há como detectar a *transição* `connected ➔ disconnected`, que é o fato que vira alerta.
É exatamente o mesmo problema já resolvido para interfaces SNMP em
[`interface_state_dataset.ts`](../backend/src/services/alerts/datasets/interface_state.rs).

### 1.2. Decisão de projeto

Seguir a arquitetura de alertas existente **sem criar um caminho paralelo**: o módulo VPN
publica *fatos* no vocabulário de `ALERT_FIELDS` e quem decide o que é alerta continua sendo
a regra cadastrada em "Regras Configuradas". Assim o operador pode ajustar severidade,
tolerância e até desligar a política sem tocar em código.

### 1.3. Tarefas

- [ ] Migration `add_last_connection_status_to_vpn_peers`: coluna `last_connection_status`
      (nullable) — a memória do ciclo anterior, análoga ao `previousOperStatus` das interfaces.
- [ ] `VpnPeer`: expor a coluna e manter o getter `connectionStatus` como fonte da verdade do
      estado *atual*.
- [ ] `ALERT_FIELDS`: novos campos `vpnPeerStatus`, `vpnStatusTransition`, `vpnPeerName`,
      `vpnSecondsSinceHandshake`.
- [ ] `backend/src/services/alerts/datasets/vpn_peer.rs`: builder + `describe_vpn_peer_state()` +
      `hasVpnTransition()` + `isVpnRecovery()`.
- [ ] `AlertScopeKey.vpnPeer(id)` para deduplicar o alerta por túnel.
- [ ] `backend/src/services/vpn/state_watcher.rs`: compara persistido × atual, publica
      `vpn:peer_status_change` no SSE e entrega o dataset ao `AlertManager`.
- [ ] Ligar o watcher ao ciclo do `VpnTrafficRecorder` (`syncAll` e `recordAll`).
- [ ] Catálogo de regras: categoria `vpn` com `vpn_peer_disconnected` (recomendada),
      `vpn_peer_unstable` e `vpn_peer_reconnected`.
- [ ] Frontend: métricas legíveis em `alertPresentation.ts` e ícone/rótulo do novo evento em
      `eventPresentation.ts`.
- [ ] Testes unitários do dataset e da detecção de transição.

---

## 2. Componente único do item de monitor

### 2.1. Situação atual

`/monitors` ([`MonitorsPage.vue`](../frontend/src/pages/MonitorsPage.vue)) tem a listagem
completa: linha do tempo, sparkline de uso, chip de tipo, switch de ativação e as ações
*Testar · Detalhes · Editar · Excluir*.

Já a aba **Monitores** de `/devices/:id` ([`DeviceDetailPage.vue`](../frontend/src/pages/DeviceDetailPage.vue))
tem uma `<v-table>` própria, só de leitura — sem ações e sem histórico. A causa está no
backend: `GET /api/devices/:id/monitors` devolve apenas o **último** resultado por monitor,
enquanto `GET /api/monitors` devolve 30 resultados, `gaugeMetric` e `gaugeHistory`.

### 2.2. Decisão de projeto

Duas extrações, uma de cada lado:

* **Backend** — `MonitorPresenter`: o enriquecimento (histórico por monitor via `groupLimit`,
  última leitura de CPU/memória e sua série) sai do `MonitorsController` para um módulo
  próprio, consumido também pelo `DevicesController`. Sem isso o componente de front
  receberia dados diferentes em cada tela.
* **Frontend** — `MonitorsTable.vue`: a tabela inteira (colunas, slots e ações) vira um
  componente com `variant` (`full` em `/monitors`, `compact` na aba do equipamento). É o
  "componente do item" pedido, no nível certo: o item de monitor não é só a célula do nome,
  é o conjunto célula + ações.

### 2.3. Tarefas

- [ ] `backend/src/services/monitoring/presenter.rs` extraído do controller de monitores.
- [ ] `MonitorsController.index` passa a usar o presenter (sem mudança de contrato).
- [ ] `DevicesController.monitors` passa a usar o presenter → mesmo payload de `/api/monitors`.
- [ ] `frontend/src/components/MonitorsTable.vue` com as ações completas.
- [ ] `MonitorsPage.vue` reescrita sobre o componente.
- [ ] Aba **Monitores** de `/devices/:id` reescrita sobre o componente.
- [ ] Store `deviceDetail`: monitores tipados como `Monitor` e recarregados após cada ação.
- [ ] Teste funcional garantindo a paridade de payload entre os dois endpoints.

---

## 3. "Adicionar ao Monitoramento" no card de Latência de DNS

### 3.1. Situação atual

No modo **Comparar**, o [`DnsLatencyCard`](../frontend/src/components/DnsLatencyCard.vue)
mede os resolvedores cadastrados e mostra o ranking. Para monitorar o vencedor, o operador
precisa sair do dashboard, abrir `/monitors`, criar um monitor DNS e redigitar o endereço e
o protocolo — três telas para uma informação que já está na frente dele.

### 3.2. Decisão de projeto

O `MonitorFormDialog` já sabe montar um monitor DNS completo (protocolo UDP/TCP/DoH,
hostnames, limiar). Falta apenas poder **abri-lo pré-preenchido**. Em vez de duplicar a
lógica de criação no card, adiciona-se uma prop `defaults` ao diálogo — o card só descreve o
que quer e o formulário continua sendo o único lugar que valida e salva monitores.

### 3.3. Tarefas

- [ ] `MonitorFormDialog`: prop `defaults?: Partial<MonitorFormModel>` aplicada na criação.
- [ ] `DnsLatencyCard`: botão *Monitorar* por item do ranking (Comparar **e** Histórico).
- [ ] Item já monitorado exibe selo *Monitorado* em vez do botão (evita duplicar monitor).
- [ ] Recarregar o ranking após salvar, para o selo aparecer sem F5.

---

## 4. Scroll infinito com paginação no backend

### 4.1. Situação atual

Parte do item **já está implementada**:

| Local | Scroll | Paginação no backend |
| :--- | :---: | :--- |
| `/monitors/:id` ▸ Histórico de Execuções Recentes | ✅ com `max-height: 450px` + `overflow-y` | ✅ `GET /api/monitors/:id/results` |
| `/discovery` ▸ Histórico de Escaneamento | ✅ altura livre | ✅ `GET /api/discovery/runs` |
| `/events` ▸ feed | ✅ altura livre | ✅ `GET /api/events` |
| `/devices/:id` ▸ Métricas | ✅ altura livre | ✅ `GET /api/devices/:id/metrics` |
| `/devices/:id` ▸ Eventos | ✅ altura livre | ✅ `GET /api/devices/:id/events` |
| `/discovery` ▸ Resultados Encontrados | ❌ | ❌ |
| `/alerts` ▸ histórico (encerrados) | ❌ (só existe a aba de ativos) | ❌ |

Ou seja: o card citado no pedido (execuções recentes) já tem limite de altura e scroll Y, e
os demais locais já seguem o padrão sem limite vertical. Restam dois pontos e uma dívida de
duplicação — o mesmo bloco de `loadMore` está copiado em cinco arquivos.

### 4.2. Tarefas

- [x] `/monitors/:id` ▸ histórico com altura limitada e scroll Y.
- [x] `/discovery` ▸ execuções, `/events` e as abas de `/devices/:id` sem limite vertical.
- [ ] `frontend/src/composables/useInfiniteList.ts` — um único `loadMore` paginado.
- [ ] Migrar os cinco usos existentes para o composable.
- [ ] `GET /api/discovery/results` paginado + scroll infinito na aba *Resultados Encontrados*.
- [ ] `GET /api/alerts` paginado + nova aba *Histórico* em `/alerts` com scroll infinito
      (a aba *Ativos* segue sem paginação: é uma lista curta e filtrada no cliente).

---

## 5. Descoberta por bloco de IP a partir de `/networks`

### 5.1. Situação atual

`POST /api/networks/:id/scan` é um **stub**: responde `"Varredura iniciada"` e não faz nada.
O botão *Escanear* em `/networks` chama esse endpoint e não produz resultado algum.

O motor, porém, existe e funciona: [`DiscoveryService`](../backend/src/services/discovery/service.rs)
combina varredura ICMP da faixa, tabela ARP e port scan, mescla os achados e grava
`discovery_runs` / `discovery_results`. Ele só nunca é acionado a partir de uma rede
cadastrada.

Há ainda um defeito no aceite: `DiscoveryController.accept` grava `siteId: run.networkId` —
o dispositivo nasce vinculado ao site errado.

### 5.2. Avaliação: nmap × solução nativa em Node

| Critério | `nmap` (via `child_process`) | Nativo em Node (atual) |
| :--- | :--- | :--- |
| Instalação | Binário externo, precisa entrar no `Dockerfile` e existir no host do probe | Zero dependência |
| Privilégio | `-sS`/`-PR` exigem **root/CAP_NET_RAW**; o container roda sem privilégio | Ping do SO e tabela ARP bastam |
| Superfície de risco | Executar binário com argumentos vindos do banco é injeção de comando em potencial — vai contra a §18 ("o probe não recebe comandos de shell livres") | Sem shell |
| Portabilidade | Windows/macOS exigem pacote separado; o projeto roda em ambos no dev | `ping` e `arp` já são abstraídos pelos scanners |
| Ganho real no alvo do produto | Notável em /16+; irrelevante em /24 (254 hosts, ~15 s no scanner atual com lote de 20) | Suficiente |
| Fingerprint de SO/serviço | Superior (`-O`, `-sV`) | Limitado a portas abertas + OUI do MAC |

**Decisão: manter a solução nativa.** O alvo do produto são redes residenciais e de pequenas
empresas — blocos /24, no máximo /22. O ganho do nmap aparece só acima disso, e o custo é
alto justamente onde o projeto é mais rígido: container sem privilégio e proibição de shell
livre no probe. A vantagem que o nmap teria de fato (identificação de serviço) é coberta em
boa parte pelo `PortScanner` + `DeviceIdentifier` já existentes.

> Reavaliar se surgir demanda por blocos maiores que /22 ou por fingerprint de SO. Nesse
> caso, a porta de entrada é um `NmapScanner` implementando a mesma interface dos scanners
> atuais, ativado por configuração e com a lista de argumentos fixa no código.

### 5.3. Como a varredura é disparada

O documento de arquitetura (§4.1) é explícito: *"o servidor HTTP não deverá executar scans"*.
Então o endpoint **não** roda a varredura — ele apenas **enfileira**:

```text
POST /api/networks/:id/scan
   ↓
DiscoveryRun (status: pending)          ← resposta 202 imediata
   ↓
scheduler:run (a cada ciclo)
   ↓
DiscoveryService.runDiscovery(cidr)
   ↓
discovery_results → SSE → /discovery
```

O mesmo laço do scheduler passa a criar execuções periódicas para as redes com
`scan_enabled`, respeitando `scan_interval` — fechando o requisito "agendar scans periódicos"
da §4.3 da arquitetura.

### 5.4. Tarefas

- [ ] Migration `add_scan_tracking_to_networks`: `last_scan_at` e `next_scan_at`.
- [ ] `NetworksController.scan` real: valida o CIDR, evita execução duplicada e cria a
      `DiscoveryRun` pendente (HTTP 202).
- [ ] `DiscoveryService.runDiscovery` aceita uma run já criada (não duplica registro).
- [ ] `DiscoveryQueue` no `scheduler:run`: executa runs pendentes e agenda as redes vencidas.
- [ ] `DiscoveryResult` ganha o vínculo de rede na listagem (IP, CIDR e nome da rede).
- [ ] Corrigir `DiscoveryController.accept` para usar `network.siteId` e `network.id`.
- [ ] `/networks`: feedback do disparo (run criada, link para `/discovery`).
- [ ] `/discovery`: seletor de rede + botão *Escanear bloco* e coluna de rede nos resultados.
- [ ] Testes: funcional do endpoint de scan e unitário da expansão de CIDR.

---

## ✅ Validação obrigatória

Conforme [`AGENTS.md`](../AGENTS.md) e [`diretrizes_testes.md`](diretrizes_testes.md):

```powershell
cd backend
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test

cd ..
npm --prefix frontend run typecheck
npm --prefix frontend run lint
npm --prefix frontend run build
```
