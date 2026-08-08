# Roadmap: Dispositivos, Monitores e Descoberta de Rede

Roadmap de melhorias para as telas `/devices/{id}`, `/monitors/{id}` e `/discovery`.

---

## 1. `/devices/{id}` — Ação "Coletar SNMP agora"

### Situação implementada

- **Frontend:** `frontend/src/pages/DeviceDetailPage.vue:49-56`
- **Backend:** `app/controllers/devices_controller.ts:18-83` e `scheduleSnmpPoll`
- **Service:** `modules/snmp/snmp_service.ts:130-377`

O botão foi renomeado para **"Coletar SNMP agora"** e a coleta SNMP agora também é disparada automaticamente em segundo plano ao criar ou editar um dispositivo com `snmpEnabled === true`. Falhas são logadas e não quebram o fluxo de cadastro.

### Arquivos alterados

- `frontend/src/pages/DeviceDetailPage.vue`
- `app/controllers/devices_controller.ts`

### Tarefas

- [x] Renomear botão no frontend para "Coletar SNMP agora".
- [x] Adicionar `pollDevice` assíncrono no `store`/`update` do dispositivo quando SNMP estiver habilitado.
- [x] Validar que falhas de SNMP não quebram o fluxo de cadastro/edição.
- [x] Testar build e testes (`npx tsc --noEmit`, `node ace test`).

---

## 2. `/monitors/{id}` — Histórico de Alertas e Recuperação

### Situação implementada

- **Frontend:** `frontend/src/pages/MonitorDetailPage.vue`
- **Backend:** `app/controllers/monitors_controller.ts` → método `alerts`
- **Rota:** `GET /api/monitors/:id/alerts`

A tela de detalhes do monitor ganhou uma nova seção "Histórico de Alertas" com tabela paginada. As colunas exibem severidade, status (ativo/resolvido), título da regra, mensagem, início do alerta e data de normalização (quando ficou OK). A lista recarrega em tempo real via SSE quando chegam eventos `alert:triggered` ou `alert:resolved` para o monitor atual.

### Arquivos alterados

- `start/routes.ts`
- `app/controllers/monitors_controller.ts`
- `frontend/src/pages/MonitorDetailPage.vue`
- `frontend/src/utils/alertPresentation.ts` (reutilizado)

### Tarefas

- [x] Criar endpoint `GET /api/monitors/:id/alerts`.
- [x] Adicionar seção na tela de detalhes do monitor.
- [x] Implementar tabela paginada com severidade, status, início e recuperação.
- [x] Conectar SSE para recarregar histórico em tempo real.
- [x] Adicionar ações de reconhecer/silenciar na linha.
- [x] Testar build e testes.

---

## 3. `/discovery` — Resultados do Último Scan e Cadastro via DeviceDialog

### Situação implementada

- **Frontend:** `frontend/src/pages/DiscoveryPage.vue`, `frontend/src/components/DeviceDialog.vue`
- **Backend:** `app/controllers/discovery_controller.ts`
- **Rotas:** `GET /api/discovery/results/latest`, `POST /api/discovery/results/:id/mark-accepted`

A aba "Resultados Encontrados" agora mostra **apenas os resultados da varredura mais recente**, independentemente do status. O botão "Aceitar" foi renomeado para **"Adicionar"** e agora abre o formulário `DeviceDialog` pré-preenchendo nome, IP, tipo, fabricante, MAC e site vindos da descoberta. Itens já adicionados (`accepted`/`merged`) não exibem o botão "Adicionar"; em vez disso, mostram um chip "Já adicionado". A aba "Histórico de Escaneamento" foi mantida.

### Arquivos alterados

- `start/routes.ts`
- `app/controllers/discovery_controller.ts`
- `frontend/src/pages/DiscoveryPage.vue`
- `frontend/src/components/DeviceDialog.vue`
- `frontend/src/stores/discovery.ts`

### Tarefas

- [x] Manter aba "Histórico de Escaneamento".
- [x] "Resultados Encontrados" mostrar apenas o último scan.
- [x] Renomear "Aceitar" para "Adicionar" e abrir `DeviceDialog` ao clicar.
- [x] Esconder botão "Adicionar" para itens já adicionados.
- [x] Criar endpoint `mark-accepted` para marcar resultado após cadastro manual.
- [x] Testar build e testes.

---

## 4. `/discovery` — Enriquecimento de Dados e Dialog de Detalhes

### Situação implementada

O scanner agora executa múltiplas fontes de descoberta em paralelo:

- **ARP ativo**: os IPs respondentes ao ICMP recebem probe TCP antes da leitura da tabela ARP, forçando resolução de MAC.
- **Lookup OUI**: base embutida de fabricantes identifica vendor a partir do MAC.
- **mDNS/Bonjour**: socket UDP multicast na porta 5353 descobre hostnames `.local`.
- **SNMP discovery**: para hosts com porta 161 aberta, tenta conexão SNMP v1/v2c e extrai `sysName`/`sysDescr`.
- **SSDP/UPnP**: socket UDP multicast na porta 1900 descobre dispositivos UPnP.
- **DeviceIdentifier**: agora considera vendor, hostname e portas para classificar o tipo.
- **Dialog de detalhes**: `DiscoveryResultDialog.vue` mostra todos os dados do item, incluindo JSON bruto.
- **Limpeza**: endpoint `DELETE /api/discovery/cleanup` e botão na UI para apagar varreduras antigas.

### Arquivos alterados/criados

- `modules/discovery/scanners/arp_scanner.ts`
- `modules/discovery/scanners/mdns_scanner.ts`
- `modules/discovery/scanners/ssdp_scanner.ts`
- `modules/discovery/scanners/snmp_discovery_scanner.ts` (novo)
- `modules/discovery/oui_lookup.ts` (novo)
- `modules/discovery/discovery_merger.ts`
- `modules/discovery/discovery_service.ts`
- `modules/discovery/device_identifier.ts`
- `frontend/src/components/DiscoveryResultDialog.vue` (novo)
- `frontend/src/pages/DiscoveryPage.vue`
- `frontend/src/stores/discovery.ts`
- `app/controllers/discovery_controller.ts`
- `start/routes.ts`
- `tests/unit/discovery.spec.ts`

### Tarefas

- [x] Melhorar captura de MAC (ARP ativo).
- [x] Adicionar lookup de vendor por OUI.
- [x] Implementar mDNS para hostname.
- [x] Implementar SNMP discovery para IPs com porta 161 aberta.
- [x] Implementar SSDP/UPnP.
- [x] Criar `DiscoveryResultDialog.vue` e abrir ao clicar no item.
- [x] Adicionar rotina/endpoint de limpeza de resultados antigos.
- [x] Testar build e testes.

> **Nota:** NetBIOS-NS/LLMNR e HTTP banner/fingerprinting não foram implementados nesta etapa; podem ser adicionados futuramente se a cobertura atual não for suficiente.

---

## Resumo do estado

- [x] Dispositivos: botão renomeado e poll SNMP automático no cadastro/edição.
- [x] Monitores: histórico de alertas com status e normalização.
- [x] Discovery: resultados do último scan, botão "Adicionar" abrindo DeviceDialog, itens adicionados identificados.
- [x] Discovery: enriquecimento de dados (MAC, vendor, mDNS, SNMP, SSDP) e dialog de detalhes.

---

*Roadmap atualizado após implementação das melhorias.*
