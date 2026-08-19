<template>
  <div>
    <!-- Botão de Voltar -->
    <v-btn variant="text" prepend-icon="mdi-arrow-left" class="mb-4" to="/devices">
      Voltar para Dispositivos
    </v-btn>

    <!-- Header do Dispositivo -->
    <v-card elevation="2" class="rounded-lg pa-4 mb-6">
      <div
        class="d-flex flex-column flex-md-row align-start align-md-center justify-space-between ga-4"
      >
        <div class="d-flex align-center ga-3">
          <v-avatar color="primary" size="48" class="mr-2">
            <v-icon color="white">mdi-router-network</v-icon>
          </v-avatar>
          <div>
            <div class="d-flex align-center ga-2 flex-wrap">
              <h1 class="text-h6 text-md-h5 font-weight-bold">
                {{ detailStore.device?.name || `Dispositivo #${deviceId}` }}
              </h1>
              <v-chip
                :color="getStatusColor(detailStore.device?.status)"
                size="small"
                variant="tonal"
                class="px-3"
              >
                <v-icon start size="14">mdi-circle</v-icon>
                {{ (detailStore.device?.status || 'UNKNOWN').toUpperCase() }}
              </v-chip>
            </div>
            <div class="text-caption text-md-subtitle-2 text-grey mt-1 text-break">
              IP: {{ detailStore.device?.ipAddress || 'Não informado' }} · Tipo:
              {{ detailStore.device?.type }} · Fabricante:
              {{ detailStore.device?.vendor || 'Desconhecido' }}
            </div>
          </div>
        </div>

        <div
          class="d-flex flex-wrap align-center justify-start justify-md-end ga-2 w-100 w-md-auto"
        >
          <v-btn
            v-if="can.createMonitor"
            color="primary"
            prepend-icon="mdi-plus"
            size="small"
            class="flex-grow-1 flex-sm-grow-0"
            @click="openMonitorDialog()"
          >
            Novo monitor
          </v-btn>

          <v-btn-group
            v-if="can.anyHeaderAction"
            color="primary"
            density="comfortable"
            variant="outlined"
            divided
            class="device-action-buttons"
          >
            <v-btn
              v-if="can.snmpScan"
              prepend-icon="mdi-radar"
              :loading="detailStore.scanningSnmp"
              aria-label="Configurar monitoramento"
              @click="openScanModal"
            >
              <span class="hidden-md-and-down">Configurar</span>
              <v-tooltip activator="parent" location="bottom" max-width="300">
                Varre o equipamento via SNMP e abre a tela onde você escolhe <b>o que</b> monitorar
                (interfaces, CPU e memória). Descobre portas novas.
              </v-tooltip>
            </v-btn>

            <v-btn
              v-if="can.snmpCollect"
              prepend-icon="mdi-refresh"
              :loading="detailStore.pollingSnmp"
              aria-label="Coletar agora"
              @click="detailStore.triggerSnmpPoll(deviceId)"
            >
              <span class="hidden-md-and-down">Coletar</span>
              <v-tooltip activator="parent" location="bottom" max-width="300">
                Executa agora uma leitura das métricas do que <b>já está</b> monitorado, sem alterar
                a configuração. É o mesmo que o agendador faz a cada ciclo.
              </v-tooltip>
            </v-btn>

            <v-btn
              v-if="can.scanPorts"
              prepend-icon="mdi-lan-connect"
              aria-label="Escanear portas"
              @click="portScanOpen = true"
            >
              <span class="hidden-md-and-down">Portas</span>
              <v-tooltip activator="parent" location="bottom">Escanear portas</v-tooltip>
            </v-btn>

            <v-btn
              v-if="can.editIdentity"
              prepend-icon="mdi-pencil"
              aria-label="Editar dispositivo"
              @click="editDeviceDialog = true"
            >
              <span class="hidden-md-and-down">Editar</span>
              <v-tooltip activator="parent" location="bottom">Editar dispositivo</v-tooltip>
            </v-btn>
          </v-btn-group>
        </div>
      </div>
    </v-card>

    <!-- Abas Interativas -->
    <v-card elevation="2" class="rounded-lg">
      <v-tabs
        v-model="activeTab"
        color="primary"
        align-tabs="title"
        show-arrows
        density="comfortable"
      >
        <v-tab value="overview" prepend-icon="mdi-information-outline">Visão Geral</v-tab>
        <v-tab value="monitors" prepend-icon="mdi-heart-pulse">
          Monitores ({{ detailStore.monitors.length }})
        </v-tab>
        <v-tab value="rules" prepend-icon="mdi-bell-cog-outline">Regras</v-tab>
        <v-tab v-if="can.interfaces" value="interfaces" prepend-icon="mdi-expansion-card">
          Interfaces SNMP ({{ detailStore.interfaces.length }})
        </v-tab>
        <v-tab v-if="can.events" value="events" prepend-icon="mdi-history">
          Histórico de Eventos
        </v-tab>
        <v-tab v-if="can.logs" value="logs" prepend-icon="mdi-text-box-search-outline">Logs</v-tab>
        <v-tab v-if="can.vpn" value="vpn" prepend-icon="mdi-shield-lock-outline">VPN</v-tab>
      </v-tabs>

      <v-divider></v-divider>

      <v-card-text class="pa-6">
        <v-window v-model="activeTab">
          <!-- Aba Visão Geral -->
          <v-window-item value="overview">
            <v-row>
              <v-col cols="12" md="6">
                <v-list border class="rounded-lg">
                  <v-list-item title="Nome" :subtitle="detailStore.device?.name"></v-list-item>
                  <v-list-item
                    title="Endereço IP"
                    :subtitle="detailStore.device?.ipAddress"
                  ></v-list-item>
                  <v-list-item
                    title="Endereço MAC"
                    :subtitle="detailStore.device?.macAddress || 'Não cadastrado'"
                  ></v-list-item>
                  <v-list-item
                    title="Fabricante / Modelo"
                    :subtitle="`${detailStore.device?.vendor || 'N/A'} - ${detailStore.device?.model || 'N/A'}`"
                  ></v-list-item>
                </v-list>
              </v-col>
              <v-col cols="12" md="6">
                <v-list border class="rounded-lg">
                  <v-list-item
                    title="SNMP Habilitado"
                    :subtitle="detailStore.device?.snmpEnabled ? 'Sim' : 'Não'"
                  ></v-list-item>
                  <v-list-item
                    title="Versão / Comunidade SNMP"
                    :subtitle="`${detailStore.device?.snmpVersion || 'v2c'} / ${detailStore.device?.snmpCommunity || 'public'}`"
                  ></v-list-item>
                  <v-list-item
                    title="Data de Cadastro"
                    :subtitle="detailStore.device?.createdAt || 'Desconhecida'"
                  ></v-list-item>
                </v-list>
              </v-col>
            </v-row>

            <!--
              Saúde do equipamento. A seção inteira só existe para quem publica
              séries de saúde — o Servidor NetMonitor as preenche todas, um
              roteador SNMP preenche as duas que o SNMP entrega, e um alvo que
              só responde ping não mostra nada. "Em dispositivos comuns, a
              Visão Geral mantém somente resumos aplicáveis."
            -->
            <template v-if="can.health">
              <v-divider class="my-6" />
              <DeviceHealthSummary :metrics="detailStore.metrics" />
            </template>

            <!--
              O resumo de tráfego é uma **métrica principal** do equipamento, e
              é isso que a Visão Geral apresenta. O detalhe por interface —
              inventário, estado, velocidade e o gráfico de cada porta — segue
              na aba Interfaces SNMP, que é onde ele tem contexto.
            -->
            <div
              v-if="interfaceTrafficSummaries.length > 0"
              class="text-subtitle-1 font-weight-bold mb-3 mt-6 d-flex align-center ga-2"
            >
              <v-icon color="primary">mdi-swap-horizontal</v-icon>
              Tráfego por interface monitorada
            </div>

            <div v-if="interfaceTrafficSummaries.length > 0" class="table-responsive">
              <v-table border hover class="rounded-lg mb-6">
                <thead>
                  <tr>
                    <th>Interface</th>
                    <th>Status Operacional</th>
                    <th>Taxa de Download (IN)</th>
                    <th>Taxa de Upload (OUT)</th>
                    <th>Volumetria Entrada</th>
                    <th>Volumetria Saída</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="item in interfaceTrafficSummaries" :key="item.ifIndex">
                    <td class="font-weight-bold">
                      <div class="d-flex align-center justify-space-between ga-1">
                        <span>
                          <v-icon size="18" class="mr-1">mdi-ethernet-cable</v-icon>
                          {{ item.ifName }}
                        </span>
                        <v-btn
                          icon
                          size="x-small"
                          variant="text"
                          color="primary"
                          @click="openInterfaceChart(item.source, 'combined')"
                        >
                          <v-icon size="16">mdi-chart-line</v-icon>
                          <v-tooltip activator="parent" location="top">
                            Ver Gráfico Combinado
                          </v-tooltip>
                        </v-btn>
                      </div>
                    </td>
                    <td>
                      <v-chip
                        :color="item.operStatus === 'up' ? 'success' : 'error'"
                        size="x-small"
                      >
                        {{ item.operStatus.toUpperCase() }}
                      </v-chip>
                    </td>
                    <td class="font-weight-medium text-success">
                      <div class="d-flex align-center justify-space-between ga-1">
                        <span>
                          <v-icon size="14" start>mdi-arrow-down-bold</v-icon>
                          {{ item.inBpsFormatted }}
                        </span>
                        <v-btn
                          icon
                          size="x-small"
                          variant="text"
                          color="success"
                          @click="openInterfaceChart(item.source, 'inBps')"
                        >
                          <v-icon size="16">mdi-chart-areaspline</v-icon>
                          <v-tooltip activator="parent" location="top">
                            Gráfico de Download (IN)
                          </v-tooltip>
                        </v-btn>
                      </div>
                    </td>
                    <td class="font-weight-medium text-primary">
                      <div class="d-flex align-center justify-space-between ga-1">
                        <span>
                          <v-icon size="14" start>mdi-arrow-up-bold</v-icon>
                          {{ item.outBpsFormatted }}
                        </span>
                        <v-btn
                          icon
                          size="x-small"
                          variant="text"
                          color="primary"
                          @click="openInterfaceChart(item.source, 'outBps')"
                        >
                          <v-icon size="16">mdi-chart-areaspline</v-icon>
                          <v-tooltip activator="parent" location="top">
                            Gráfico de Upload (OUT)
                          </v-tooltip>
                        </v-btn>
                      </div>
                    </td>
                    <td class="text-grey-darken-1">
                      <div class="d-flex align-center justify-space-between ga-1">
                        <span>{{ item.inBytesFormatted }}</span>
                        <v-btn
                          icon
                          size="x-small"
                          variant="text"
                          color="info"
                          @click="openInterfaceChart(item.source, 'inOctets')"
                        >
                          <v-icon size="16">mdi-chart-box-outline</v-icon>
                          <v-tooltip activator="parent" location="top">
                            Gráfico Volumetria Entrada
                          </v-tooltip>
                        </v-btn>
                      </div>
                    </td>
                    <td class="text-grey-darken-1">
                      <div class="d-flex align-center justify-space-between ga-1">
                        <span>{{ item.outBytesFormatted }}</span>
                        <v-btn
                          icon
                          size="x-small"
                          variant="text"
                          color="info"
                          @click="openInterfaceChart(item.source, 'outOctets')"
                        >
                          <v-icon size="16">mdi-chart-box-outline</v-icon>
                          <v-tooltip activator="parent" location="top">
                            Gráfico Volumetria Saída
                          </v-tooltip>
                        </v-btn>
                      </div>
                    </td>
                  </tr>
                </tbody>
              </v-table>
            </div>

            <!-- Funcionalidade indisponível não gera aba vazia: a ação para
                 habilitá-la fica aqui, com a explicação curta. -->
            <v-alert
              v-if="can.snmpConfigured && !can.snmpConnected"
              type="info"
              variant="tonal"
              density="comfortable"
              class="rounded-lg mt-4"
            >
              <div class="font-weight-medium">SNMP configurado, mas ainda sem resposta</div>
              <div class="text-caption">
                O inventário de interfaces e o tráfego aparecem depois da primeira comunicação
                bem-sucedida. Verifique a comunidade e o alcance de rede e execute uma varredura.
              </div>
              <template #append>
                <v-btn
                  size="small"
                  variant="tonal"
                  color="primary"
                  :loading="detailStore.scanningSnmp"
                  @click="openScanModal"
                >
                  Varrer agora
                </v-btn>
              </template>
            </v-alert>
            <!-- 3. Tabela do Histórico Bruto de Registros Recentes -->
            <v-card elevation="2" class="rounded-lg pa-4 border">
              <div class="d-flex align-center justify-space-between">
                <div class="font-weight-bold text-subtitle-2 d-flex align-center ga-2">
                  <v-icon color="primary">mdi-history</v-icon>
                  Histórico de Registros Brutos (Métricas de Itens Monitorados)
                </div>
                <v-btn icon size="small" variant="text" @click="toggleShowMetricsHistory">
                  <v-icon>{{ showMetricsHistory ? 'mdi-chevron-up' : 'mdi-chevron-down' }}</v-icon>
                  <v-tooltip activator="parent" location="top">
                    {{ showMetricsHistory ? 'Ocultar Histórico' : 'Mostrar Histórico' }}
                  </v-tooltip>
                </v-btn>
              </div>

              <v-expand-transition>
                <div v-if="showMetricsHistory">
                  <div
                    class="history-scroll-container rounded-lg border overflow-y-auto mt-3"
                    style="max-height: 450px"
                  >
                    <v-infinite-scroll
                      :key="metricsHistory.scrollKey.value"
                      :height="420"
                      @load="metricsHistory.load"
                    >
                      <div class="table-responsive">
                        <v-table density="compact" hover>
                          <thead>
                            <tr>
                              <th>Nome da Métrica</th>
                              <th>Interface / Contexto</th>
                              <th>Valor</th>
                              <th>Unidade</th>
                              <th>Data/Hora</th>
                            </tr>
                          </thead>
                          <tbody>
                            <tr v-for="met in metricsHistory.items.value" :key="met.id">
                              <td class="font-weight-medium">{{ met.metricName }}</td>
                              <td>{{ met.interfaceName || 'Sistema / Geral' }}</td>
                              <td class="font-weight-bold">{{ formatMetricValue(met) }}</td>
                              <td>{{ met.unit || '-' }}</td>
                              <td class="text-grey">{{ met.createdAt }}</td>
                            </tr>
                          </tbody>
                        </v-table>
                      </div>
                      <template #empty>
                        <div class="text-caption text-grey text-center py-3">
                          Nenhum outro registro no histórico de métricas.
                        </div>
                      </template>
                    </v-infinite-scroll>
                  </div>
                </div>
              </v-expand-transition>
            </v-card>
          </v-window-item>

          <!-- Aba Monitores -->
          <v-window-item value="monitors">
            <!--
              Quando não há monitor de alcance, o motivo vem do backend
              (`reachMonitorBlockedReason`) em vez de a tela deduzi-lo: são duas
              causas diferentes — o dispositivo do sistema e o cadastro sem IP —
              e cada uma pede uma ação diferente de quem está olhando.
            -->
            <v-alert
              v-if="detailStore.capabilities?.reachMonitorBlockedReason"
              type="info"
              variant="tonal"
              density="comfortable"
              class="mb-4 rounded-lg"
            >
              {{ detailStore.capabilities.reachMonitorBlockedReason }}
            </v-alert>
            <MonitorsTable
              :monitors="detailStore.monitors"
              :loading="detailStore.loading"
              variant="device"
              no-data-text='Nenhum monitor configurado para este equipamento. Use "Novo monitor" ou "Configurar Monitoramento" para descobrir automaticamente.'
              @edit="openMonitorDialog"
              @changed="reloadMonitors"
            ></MonitorsTable>
          </v-window-item>

          <!-- Aba Regras -->
          <v-window-item value="rules">
            <DeviceRulesTab
              :device-id="deviceId"
              :device-name="detailStore.device?.name"
              :monitor-names="monitorNames"
              :available-fields="detailStore.capabilities?.alertFields"
            />
          </v-window-item>

          <!-- Aba Interfaces SNMP -->
          <v-window-item value="interfaces">
            <div class="text-caption text-grey mb-3">
              Clique em uma interface para ver o histórico de tráfego e incluí-la ou removê-la do
              monitoramento.
            </div>
            <div class="table-responsive">
              <v-table hover>
                <thead>
                  <tr>
                    <th>Index</th>
                    <th>Nome Interface</th>
                    <th>Monitoramento</th>
                    <th>Status Operacional</th>
                    <th>MAC Address</th>
                    <th>Velocidade de Negociação</th>
                    <th style="width: 56px"></th>
                  </tr>
                </thead>
                <tbody>
                  <tr
                    v-for="intf in detailStore.interfaces"
                    :key="intf.id"
                    class="cursor-pointer"
                    @click="openInterfaceChart(intf)"
                  >
                    <td>{{ intf.ifIndex ?? intf.snmpIndex ?? '-' }}</td>
                    <td class="font-weight-bold">{{ interfaceLabel(intf) }}</td>
                    <td>
                      <v-chip
                        :color="intf.isMonitored ? 'primary' : 'grey'"
                        size="x-small"
                        variant="tonal"
                      >
                        {{ intf.isMonitored ? 'MONITORADA' : 'NÃO MONITORADA' }}
                      </v-chip>
                    </td>
                    <td>
                      <v-chip
                        :color="
                          (intf.ifOperStatus || intf.operStatus) === 'up' ? 'success' : 'error'
                        "
                        size="x-small"
                      >
                        Oper: {{ intf.ifOperStatus || intf.operStatus || 'unknown' }}
                      </v-chip>
                    </td>
                    <td>{{ intf.macAddress || 'N/A' }}</td>
                    <td>
                      <v-chip size="x-small" variant="tonal" color="info">
                        {{ formatLinkSpeed(intf.ifSpeed || intf.speed) }}
                      </v-chip>
                    </td>
                    <td>
                      <v-btn icon size="x-small" variant="text" color="primary">
                        <v-icon size="18">mdi-chart-line</v-icon>
                        <v-tooltip activator="parent" location="top">
                          Ver gráficos e gerenciar monitoramento
                        </v-tooltip>
                      </v-btn>
                    </td>
                  </tr>
                  <tr v-if="detailStore.interfaces.length === 0">
                    <td colspan="7" class="text-center text-grey py-4">
                      Nenhuma interface SNMP registrada ainda. Use "Configurar Monitoramento" para
                      descobri-las.
                    </td>
                  </tr>
                </tbody>
              </v-table>
            </div>
          </v-window-item>

          <!-- Aba Eventos -->
          <v-window-item value="events">
            <v-infinite-scroll :key="eventsHistory.scrollKey.value" @load="eventsHistory.load">
              <div class="table-responsive">
                <v-table hover density="comfortable" class="rounded-lg border">
                  <thead>
                    <tr>
                      <th>Severidade</th>
                      <th>Mensagem</th>
                      <th>Data/Hora</th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr v-for="evt in eventsHistory.items.value" :key="evt.id">
                      <td>
                        <v-chip
                          :color="
                            evt.severity === 'critical' || evt.severity === 'error'
                              ? 'error'
                              : 'warning'
                          "
                          size="x-small"
                        >
                          {{ (evt.severity || 'INFO').toUpperCase() }}
                        </v-chip>
                      </td>
                      <td>{{ evt.message }}</td>
                      <td>{{ evt.createdAt }}</td>
                    </tr>
                  </tbody>
                </v-table>
              </div>
              <template #empty>
                <div class="text-caption text-grey text-center py-4">
                  Nenhum outro evento registrado no histórico.
                </div>
              </template>
            </v-infinite-scroll>
          </v-window-item>

          <!-- Aba Logs: syslog recebido deste dispositivo -->
          <v-window-item value="logs">
            <!--
              Chamada para ação enquanto o equipamento nunca enviou nada. É o
              estado em que a aba nasce para todo dispositivo recém-cadastrado,
              e sem esta explicação ela parece quebrada em vez de vazia.
            -->
            <v-alert
              v-if="logsNaoConfigurados"
              type="info"
              variant="tonal"
              border="start"
              class="mb-4"
            >
              <div class="d-flex align-center flex-wrap ga-3">
                <div class="flex-grow-1">
                  <div class="font-weight-bold mb-1">
                    Este equipamento ainda não envia log para o servidor.
                  </div>
                  O envio de syslog é configurado no próprio roteador. O servidor pode fazer isso
                  sozinho: ele acessa o equipamento, aplica os comandos e confirma a chegada da
                  primeira mensagem.
                </div>
                <div class="d-flex ga-2">
                  <v-btn color="primary" variant="flat" @click="autoSetupDialog = true">
                    <v-icon start>mdi-flash</v-icon>
                    Ativar log
                  </v-btn>
                  <v-btn color="primary" variant="tonal" @click="setupDialog = true">
                    Ver comandos
                  </v-btn>
                </div>
              </div>
            </v-alert>

            <!--
              Mascaramento do Docker: o log pode estar chegando e mesmo assim
              não aparecer aqui, porque a origem não resolve para este
              dispositivo. Sem este aviso o operador refaz a configuração do
              roteador, que já estava certa.
            -->
            <v-alert
              v-if="logsStore.natMasking"
              type="warning"
              variant="tonal"
              border="start"
              class="mb-4"
              density="comfortable"
            >
              <div class="font-weight-bold mb-1">
                O Docker está reescrevendo o endereço de origem das mensagens.
              </div>
              Todos os equipamentos chegam como
              <strong>{{
                (logsStore.nat?.gateways ?? []).join(', ') || 'o gateway da bridge'
              }}</strong
              >, então o vínculo passa a depender do nome que cada um envia no syslog. Abra
              <RouterLink to="/logs">Logs</RouterLink> para vincular por nome, ou publique o
              servidor com <code>network_mode: host</code> para o endereço real chegar.
            </v-alert>

            <div class="d-flex align-center flex-wrap ga-3 mb-4">
              <v-select
                v-model="logSeverity"
                :items="logSeverityOptions"
                item-title="label"
                item-value="value"
                label="Severidade"
                hide-details
                clearable
                density="compact"
                variant="outlined"
                style="max-width: 240px"
                @update:model-value="applyLogFilters"
              ></v-select>
              <v-select
                v-model="logHours"
                :items="logWindowOptions"
                item-title="label"
                item-value="value"
                label="Período"
                hide-details
                density="compact"
                variant="outlined"
                style="max-width: 200px"
                @update:model-value="applyLogFilters"
              ></v-select>
              <v-spacer></v-spacer>
              <!--
                Continua disponível depois de configurado: é por aqui que se
                reaplica a configuração num roteador que foi trocado ou
                resetado.
              -->
              <v-btn color="primary" variant="tonal" size="small" @click="autoSetupDialog = true">
                <v-icon start>mdi-flash</v-icon>
                <span class="hidden-xs">Ativar log</span>
              </v-btn>
              <v-btn
                :color="logsStore.tailing ? 'success' : 'primary'"
                :variant="logsStore.tailing ? 'flat' : 'tonal'"
                size="small"
                @click="logsStore.toggleTail()"
              >
                <v-icon start>
                  {{ logsStore.tailing ? 'mdi-radiobox-marked' : 'mdi-play-circle-outline' }}
                </v-icon>
                {{ logsStore.tailing ? 'Ao vivo' : 'Acompanhar' }}
              </v-btn>
            </div>

            <LogTable
              :entries="logsStore.entries"
              :scroll-key="logsStore.scrollKey"
              :load="logsStore.load"
              :error="logsStore.error"
              :show-source="false"
              empty-hint="Este dispositivo ainda não enviou syslog para o servidor."
            />
          </v-window-item>

          <!-- Aba VPN -->
          <v-window-item v-if="vpnPeer" value="vpn">
            <v-alert
              v-if="vpnNeedsFirewallHint"
              type="warning"
              variant="tonal"
              class="mb-6"
              density="comfortable"
            >
              <div class="font-weight-bold mb-1">
                Túnel conectado, mas o dispositivo não responde a ping.
              </div>
              <div class="text-body-2 mb-2">
                Provavelmente falta liberar o tráfego na interface WireGuard.
              </div>
              <v-btn size="small" color="warning" variant="flat" @click="showVpnFirewallHints">
                Copiar regras de firewall
              </v-btn>
            </v-alert>

            <v-row class="mb-2">
              <v-col cols="12" md="6">
                <v-list border class="rounded-lg">
                  <v-list-item title="Perfil do equipamento">
                    <template #subtitle>
                      <v-chip size="small" variant="tonal" class="mt-1">
                        <v-icon start size="14">{{ vpnProfileIconValue }}</v-icon>
                        {{ vpnProfileLabelValue }}
                      </v-chip>
                    </template>
                  </v-list-item>
                  <v-list-item title="Status do túnel">
                    <template #subtitle>
                      <v-chip :color="vpnStatusColorValue" size="small" variant="flat" class="mt-1">
                        {{ vpnStatusLabelValue }}
                      </v-chip>
                    </template>
                  </v-list-item>
                  <v-list-item
                    title="Endereço na VPN"
                    :subtitle="detailStore.device?.ipAddress || 'Não informado'"
                  ></v-list-item>
                  <v-list-item
                    title="Último handshake"
                    :subtitle="vpnLastHandshakeText"
                  ></v-list-item>
                </v-list>
              </v-col>
              <v-col cols="12" md="6">
                <v-list border class="rounded-lg">
                  <v-list-item
                    title="Keepalive persistente"
                    :subtitle="`${vpnPeer.persistentKeepalive}s`"
                  ></v-list-item>
                  <v-list-item
                    title="Chave pública do peer"
                    :subtitle="vpnPeer.publicKey"
                    class="text-truncate"
                  ></v-list-item>
                  <v-list-item
                    title="Sub-rede da VPN"
                    :subtitle="vpnStore.state?.cidr || 'Não configurada'"
                  ></v-list-item>
                  <v-list-item
                    title="Acesso"
                    :subtitle="vpnPeer.enabled ? 'Habilitado' : 'Revogado'"
                  ></v-list-item>
                </v-list>
              </v-col>
            </v-row>

            <div
              class="text-subtitle-1 font-weight-bold mb-3 mt-4 d-flex align-center ga-2"
              style="gap: 8px"
            >
              <v-icon color="primary">mdi-swap-horizontal</v-icon>
              Tráfego do Túnel WireGuard
            </div>

            <v-row class="mb-4">
              <v-col cols="12" sm="6">
                <v-card border flat class="pa-4 rounded-lg text-center">
                  <div class="text-caption text-grey">Total Recebido (RX)</div>
                  <div class="text-h6 font-weight-bold text-success">
                    {{ formatBytes(vpnPeer.bytesRx) }}
                  </div>
                </v-card>
              </v-col>
              <v-col cols="12" sm="6">
                <v-card border flat class="pa-4 rounded-lg text-center">
                  <div class="text-caption text-grey">Total Enviado (TX)</div>
                  <div class="text-h6 font-weight-bold text-primary">
                    {{ formatBytes(vpnPeer.bytesTx) }}
                  </div>
                </v-card>
              </v-col>
            </v-row>

            <BaseMetricChart
              v-if="vpnTrafficSeries.length > 0"
              :series="vpnTrafficSeries"
              unit-type="bandwidth"
            />
            <div v-else class="text-center text-grey py-10 border rounded-lg bg-grey-lighten-5">
              <v-icon size="40" color="grey-lighten-1">mdi-chart-line-variant</v-icon>
              <div class="mt-2 text-subtitle-2">
                Sem amostras de tráfego ainda. O histórico é coletado a cada 30s pelo scheduler.
              </div>
            </div>

            <v-divider class="my-6"></v-divider>

            <div class="d-flex flex-column flex-md-row flex-wrap ga-3">
              <v-btn
                color="primary"
                variant="tonal"
                prepend-icon="mdi-content-copy"
                size="small"
                class="flex-grow-1 flex-md-grow-0"
                @click="openVpnConfig"
              >
                Copiar configuração
              </v-btn>
              <v-btn
                color="warning"
                variant="tonal"
                prepend-icon="mdi-key-change"
                size="small"
                class="flex-grow-1 flex-md-grow-0"
                @click="rotateVpnKeys"
              >
                Rotacionar chaves
              </v-btn>
              <v-btn
                color="error"
                variant="tonal"
                prepend-icon="mdi-cancel"
                size="small"
                class="flex-grow-1 flex-md-grow-0"
                @click="revokeVpnAccess"
              >
                Revogar acesso
              </v-btn>
              <v-spacer class="hidden-sm-and-down"></v-spacer>
              <v-btn
                variant="text"
                prepend-icon="mdi-open-in-new"
                size="small"
                class="flex-grow-1 flex-md-grow-0"
                :to="{ name: 'vpn-devices' }"
              >
                Ver todos os dispositivos VPN
              </v-btn>
            </div>
          </v-window-item>
        </v-window>
      </v-card-text>
    </v-card>

    <!-- Modal de Escaneamento SNMP -->
    <v-dialog
      v-model="scanModalOpen"
      :max-width="$vuetify.display.xs ? undefined : 850"
      :fullscreen="$vuetify.display.xs"
    >
      <v-card class="rounded-lg">
        <v-card-title class="d-flex align-center justify-space-between pa-4 bg-primary text-white">
          <div class="d-flex align-center ga-2" style="gap: 8px">
            <v-icon>mdi-radar</v-icon>
            <span>Escaneamento & Descoberta SNMP</span>
          </div>
          <v-btn icon variant="text" color="white" @click="scanModalOpen = false">
            <v-icon>mdi-close</v-icon>
          </v-btn>
        </v-card-title>

        <v-card-text class="pa-6">
          <div v-if="detailStore.scanningSnmp" class="text-center py-8">
            <v-progress-circular
              indeterminate
              color="primary"
              size="48"
              class="mb-4"
            ></v-progress-circular>
            <div class="text-subtitle-1">
              Escaneando dispositivo via SNMP em {{ detailStore.device?.ipAddress }}...
            </div>
            <div class="text-caption text-grey">Consultando interfaces e uso de CPU/memória...</div>
          </div>

          <div v-else-if="detailStore.scanResult">
            <v-alert
              v-if="Object.keys(detailStore.scanResult.collectorErrors || {}).length"
              type="warning"
              variant="tonal"
              density="compact"
              class="mb-4"
            >
              Coleta parcial:
              {{
                Object.entries(detailStore.scanResult.collectorErrors)
                  .map(([collector, error]) => `${collector}: ${error}`)
                  .join(' · ')
              }}
            </v-alert>
            <!-- Alerta de Ausência Total de Resposta SNMP -->
            <v-alert
              v-if="!detailStore.scanResult.snmpResponded"
              type="warning"
              variant="tonal"
              class="mb-4"
              prepend-icon="mdi-alert-circle-outline"
              title="Nenhuma resposta SNMP"
              text="O dispositivo não respondeu a nenhum OID consultado, mesmo os padrão (sysDescr/sysName). Confira: (1) SNMP está habilitado no próprio equipamento — não só aqui no cadastro; (2) a community configurada aqui bate com a community de leitura configurada no equipamento; (3) a versão SNMP (v1/v2c/v3) está correta; (4) a porta 161/UDP chega ao equipamento a partir deste servidor (sem firewall/NAT no meio)."
            ></v-alert>

            <!-- Dados do Sistema -->
            <v-alert
              v-else
              type="info"
              variant="tonal"
              class="mb-4"
              prepend-icon="mdi-router"
              title="Dispositivo Conectado"
              :subtitle="detailStore.scanResult.systemInfo.sysDescr || 'Dispositivo SNMP'"
            ></v-alert>

            <!-- Recursos de CPU & Memória (apenas se o dispositivo de fato expôs esses dados) -->
            <v-card
              v-if="hasCpuData || hasMemoryData"
              variant="outlined"
              class="mb-6 rounded-lg pa-4"
            >
              <div
                class="text-subtitle-1 font-weight-bold mb-3 d-flex align-center ga-2"
                style="gap: 8px"
              >
                <v-icon color="primary">mdi-chip</v-icon>
                Monitoramento de Recursos da CPU & Memória
              </div>
              <v-row>
                <v-col v-if="hasCpuData" cols="12" md="6">
                  <v-switch
                    v-model="selectedCpuMonitor"
                    color="primary"
                    label="Monitorar Uso de CPU (%)"
                    hide-details
                  ></v-switch>
                  <div class="text-caption text-grey ml-8">
                    {{
                      detailStore.scanResult.cpuInfo.coresCount
                        ? `${detailStore.scanResult.cpuInfo.coresCount} núcleos detectados`
                        : 'Medição via MIB'
                    }}
                    <span v-if="detailStore.scanResult.cpuInfo.usagePercent != null">
                      - Uso Atual: {{ detailStore.scanResult.cpuInfo.usagePercent.toFixed(1) }}%
                    </span>
                  </div>
                </v-col>
                <v-col v-if="hasMemoryData" cols="12" md="6">
                  <v-switch
                    v-model="selectedMemoryMonitor"
                    color="primary"
                    label="Monitorar Memória RAM (%)"
                    hide-details
                  ></v-switch>
                  <div class="text-caption text-grey ml-8">
                    <span v-if="detailStore.scanResult.memoryInfo.totalKb">
                      Total: {{ Math.round(detailStore.scanResult.memoryInfo.totalKb / 1024) }} MB
                    </span>
                    <span v-if="detailStore.scanResult.memoryInfo.usedPercent != null">
                      - Uso: {{ detailStore.scanResult.memoryInfo.usedPercent.toFixed(1) }}%
                    </span>
                  </div>
                </v-col>
              </v-row>
            </v-card>

            <!-- Lista de Interfaces Descobertas -->
            <div class="d-flex align-center justify-space-between mb-3">
              <div
                class="text-subtitle-1 font-weight-bold d-flex align-center ga-2"
                style="gap: 8px"
              >
                <v-icon color="primary">mdi-ethernet-cable</v-icon>
                Interfaces de Rede Descobertas ({{ detailStore.scanResult.interfaces.length }})
              </div>
              <div class="d-flex align-center ga-2" style="gap: 8px">
                <v-btn size="small" variant="text" color="primary" @click="selectAllInterfaces">
                  Selecionar Todas
                </v-btn>
                <v-btn size="small" variant="text" color="grey" @click="unselectAllInterfaces">
                  Desmarcar Todas
                </v-btn>
              </div>
            </div>

            <div class="table-responsive">
              <v-table border hover class="rounded-lg">
                <thead>
                  <tr>
                    <th style="width: 50px">Monitorar</th>
                    <th>Index</th>
                    <th>Nome Interface</th>
                    <th>MAC Address</th>
                    <th>Velocidade</th>
                    <th>Status Operacional</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="iface in detailStore.scanResult.interfaces" :key="iface.ifIndex">
                    <td>
                      <v-checkbox
                        :model-value="selectedIfIndexes.includes(iface.ifIndex)"
                        color="primary"
                        hide-details
                        @update:model-value="toggleInterface(iface.ifIndex)"
                      ></v-checkbox>
                    </td>
                    <td>{{ iface.ifIndex }}</td>
                    <td class="font-weight-bold">{{ iface.ifName }}</td>
                    <td>{{ iface.macAddress || 'N/A' }}</td>
                    <td>
                      <v-chip size="x-small" variant="tonal" color="info">
                        {{ formatLinkSpeed(iface.ifSpeed) }}
                      </v-chip>
                    </td>
                    <td>
                      <v-chip
                        :color="iface.ifOperStatus === 'up' ? 'success' : 'error'"
                        size="x-small"
                      >
                        {{ iface.ifOperStatus ? iface.ifOperStatus.toUpperCase() : 'DOWN' }}
                      </v-chip>
                    </td>
                  </tr>
                  <tr v-if="detailStore.scanResult.interfaces.length === 0">
                    <td colspan="6" class="text-center text-grey py-4">
                      Nenhuma interface respondeu na varredura SNMP.
                    </td>
                  </tr>
                </tbody>
              </v-table>
            </div>
          </div>
        </v-card-text>

        <v-divider></v-divider>

        <v-card-actions class="pa-4 justify-end">
          <v-btn variant="text" @click="scanModalOpen = false">Cancelar</v-btn>
          <v-btn
            color="primary"
            prepend-icon="mdi-check"
            :loading="savingMonitors"
            :disabled="!detailStore.scanResult || detailStore.scanningSnmp"
            @click="saveMonitors"
          >
            Salvar Configurações de Monitoramento
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <!-- Modal de Gráfico de Tráfego de Interface -->
    <TrafficChartDialog
      v-model="chartDialogOpen"
      :interface-id="selectedInterface?.id ?? null"
      :interface-name="selectedInterface ? interfaceLabel(selectedInterface) : ''"
      :initial-metric="selectedChartMetricType"
      :metrics="detailStore.metrics"
      can-manage-monitoring
      :is-monitored="selectedInterface?.isMonitored === true"
      :busy="detailStore.updatingInterfaceId === selectedInterface?.id"
      @toggle-monitoring="toggleInterfaceMonitoring"
    />

    <!-- Modais da aba VPN -->
    <VpnScriptViewer v-model="vpnViewerOpen" :artifact="vpnStore.lastArtifact" :qr-svg="null" />
    <VpnFirewallHintsDialog v-model="vpnFirewallOpen" :content="vpnFirewallContent" />

    <!-- Modal de Scanner de Portas TCP/UDP -->
    <PortScanDialog
      v-model="portScanOpen"
      :host="detailStore.device?.ipAddress"
      :device-name="detailStore.device?.name"
    />

    <!-- Monitor deste equipamento: o vínculo já vem definido e travado -->
    <MonitorFormDialog
      v-model="monitorDialog"
      :monitor="editingMonitor"
      :default-device-id="deviceId"
      lock-device
      @saved="onMonitorSaved"
    />

    <!-- Modal de Edição do Equipamento -->
    <DeviceDialog
      v-model="editDeviceDialog"
      :device-to-edit="detailStore.device"
      @saved="onDeviceSaved"
    />

    <!-- Modais da aba Logs -->
    <SyslogAutoSetupDialog
      v-model="autoSetupDialog"
      :device-id="deviceId"
      :device-name="detailStore.device?.name ?? ''"
      :host="detailStore.device?.ipAddress"
    />
    <SyslogSetupDialog v-model="setupDialog" />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import {
  useDeviceDetailStore,
  type DeviceInterface,
  type DeviceMetric,
  type DeviceMonitor,
} from '@/stores/deviceDetail'
import TrafficChartDialog from '@/components/TrafficChartDialog.vue'
import BaseMetricChart, { type ChartSeriesInput } from '@/components/BaseMetricChart.vue'
import VpnScriptViewer from '@/components/VpnScriptViewer.vue'
import VpnFirewallHintsDialog from '@/components/VpnFirewallHintsDialog.vue'
import PortScanDialog from '@/components/PortScanDialog.vue'
import MonitorFormDialog from '@/components/MonitorFormDialog.vue'
import DeviceDialog from '@/components/DeviceDialog.vue'
import MonitorsTable from '@/components/MonitorsTable.vue'
import DeviceHealthSummary from '@/components/devices/DeviceHealthSummary.vue'
import DeviceRulesTab from '@/components/devices/DeviceRulesTab.vue'
import { getStatusColor } from '@/utils/monitorPresentation'
import {
  formatBps,
  formatBytes,
  formatLinkSpeed,
  formatMeasuredValue,
  formatRelativeTime,
} from '@/utils/formatters'
import {
  useVpnStore,
  vpnProfileIcon,
  vpnProfileLabel,
  vpnStatusColor,
  vpnStatusLabel,
} from '@/stores/vpn'

import { useInfiniteList } from '@/composables/useInfiniteList'
import LogTable from '@/components/logs/LogTable.vue'
import SyslogAutoSetupDialog from '@/components/logs/SyslogAutoSetupDialog.vue'
import SyslogSetupDialog from '@/components/logs/SyslogSetupDialog.vue'
import { useLogsStore, SEVERITY_OPTIONS, WINDOW_OPTIONS } from '@/stores/logs'

interface DeviceEventItem {
  id: number
  deviceId: number
  eventType: string
  severity: string
  message: string
  createdAt: string
}

const route = useRoute()
const router = useRouter()
const detailStore = useDeviceDetailStore()
const vpnStore = useVpnStore()
const activeTab = ref('overview')

/**
 * O que esta página pode mostrar e oferecer — respondido pelo backend.
 *
 * A mesma projeção governa **abas e botões**. Antes, o cabeçalho oferecia
 * "Configurar", "Coletar", "Portas" e "Editar" para qualquer dispositivo: no
 * Servidor NetMonitor isso significava escanear as próprias portas e editar o
 * IP de um equipamento protegido — ações que só podiam devolver erro.
 *
 * Enquanto as capacidades não chegam, o padrão é conservador: nada de SNMP e
 * nada de abas condicionais. Uma aba que aparece e some meio segundo depois é
 * pior do que uma que demora meio segundo para aparecer.
 */
const can = computed(() => {
  const caps = detailStore.capabilities
  const snmpConnected = caps?.snmpConnected ?? false
  const isSystem = caps?.isSystem ?? false
  const snmpScan = caps?.canSnmpScan ?? !isSystem
  const snmpCollect = caps?.canSnmpCollect ?? false
  const scanPorts = caps?.canScanPorts ?? false
  const editIdentity = caps?.canEditIdentity ?? !isSystem
  return {
    isSystem,
    snmpConfigured: caps?.snmpConfigured ?? false,
    snmpConnected,
    interfaces: caps?.interfaces ?? false,
    events: caps?.events ?? false,
    logs: caps?.logs ?? false,
    vpn: caps?.vpn ?? Boolean(detailStore.device?.vpnPeer),
    health: caps?.health ?? false,
    snmpScan,
    snmpCollect,
    scanPorts,
    editIdentity,
    createMonitor: caps?.canCreateMonitor ?? !isSystem,
    anyHeaderAction: snmpScan || snmpCollect || scanPorts || editIdentity,
  }
})

/** As abas que existem hoje para este dispositivo. */
const abasAplicaveis = computed(() => {
  const abas = ['overview', 'monitors', 'rules']
  if (can.value.interfaces) abas.push('interfaces')
  if (can.value.events) abas.push('events')
  if (can.value.logs) abas.push('logs')
  if (can.value.vpn) abas.push('vpn')
  return abas
})

/** Nome de cada monitor, para a aba de regras descrever o escopo. */
const monitorNames = computed<Record<number, string>>(() =>
  Object.fromEntries(detailStore.monitors.map((monitor) => [monitor.id, monitor.name]))
)
const scanModalOpen = ref(false)
const savingMonitors = ref(false)
const portScanOpen = ref(false)
const editDeviceDialog = ref(false)

async function onDeviceSaved() {
  if (deviceId.value) {
    await detailStore.loadDeviceDetails(deviceId.value)
  }
}

// Histórico paginado de métricas SNMP
const showMetricsHistory = ref(false)
const metricsHistory = useInfiniteList<DeviceMetric>(() => `/devices/${deviceId.value}/metrics`, {
  label: 'histórico de métricas',
})

function toggleShowMetricsHistory() {
  showMetricsHistory.value = !showMetricsHistory.value
  if (showMetricsHistory.value) metricsHistory.reset()
}

// Histórico paginado de eventos do dispositivo
const eventsHistory = useInfiniteList<DeviceEventItem>(() => `/devices/${deviceId.value}/events`, {
  label: 'histórico de eventos',
})

const chartDialogOpen = ref(false)
const selectedInterface = ref<DeviceInterface | null>(null)
const selectedChartMetricType = ref<'inBps' | 'outBps' | 'inOctets' | 'outOctets' | 'combined'>(
  'inBps'
)

const vpnViewerOpen = ref(false)
const vpnFirewallOpen = ref(false)
const vpnFirewallContent = ref('')

function interfaceLabel(intf: DeviceInterface): string {
  return intf.ifName || intf.name || `if-${intf.id}`
}

function openInterfaceChart(
  intf: DeviceInterface,
  metricType: 'inBps' | 'outBps' | 'inOctets' | 'outOctets' | 'combined' = 'combined'
) {
  selectedInterface.value = intf
  selectedChartMetricType.value = metricType
  chartDialogOpen.value = true
}

/**
 * Inclui/remove a interface aberta no diálogo. O recarregamento troca os
 * objetos de `detailStore.interfaces`, então a seleção precisa ser reapontada
 * para o registro novo — sem isso o diálogo continuaria mostrando o estado
 * anterior.
 */
async function toggleInterfaceMonitoring(enabled: boolean) {
  const target = selectedInterface.value
  if (!target) return

  const success = await detailStore.setInterfaceMonitoring(deviceId.value, target.id, enabled)
  if (success) {
    selectedInterface.value =
      detailStore.interfaces.find((intf) => intf.id === target.id) ?? selectedInterface.value
  }
}

const selectedCpuMonitor = ref(true)
const selectedMemoryMonitor = ref(true)

// O equipamento pode simplesmente não expor essas MIBs (ex: controlador solar sem CPU/RAM) —
// só faz sentido oferecer o toggle quando a varredura de fato encontrou o dado correspondente.
const hasCpuData = computed(() => {
  const cpu = detailStore.scanResult?.cpuInfo
  return Boolean(
    cpu && (cpu.usagePercent != null || cpu.coresCount != null || cpu.load1min != null)
  )
})
const hasMemoryData = computed(() => {
  const mem = detailStore.scanResult?.memoryInfo
  return Boolean(mem && (mem.usedPercent != null || mem.totalKb != null))
})
const selectedIfIndexes = ref<number[]>([])

const deviceId = computed(() => Number(route.params.id))

onMounted(() => {
  if (deviceId.value) {
    detailStore.loadDeviceDetails(deviceId.value)
    // Traz CIDR e keepalive do servidor VPN para contextualizar a aba VPN, se existir.
    vpnStore.fetchServer()
  }
})

// --- Aba de logs -----------------------------------------------------------
//
// A store de logs é a mesma da tela `/logs`, com o filtro fixado neste
// dispositivo. Reaproveitar em vez de duplicar mantém uma única definição de
// paginação por cursor e de live tail.

const logsStore = useLogsStore()
const logSeverity = ref<number | null>(null)
const logHours = ref<number | null>(24)
const logSeverityOptions = SEVERITY_OPTIONS
const logWindowOptions = WINDOW_OPTIONS

function applyLogFilters(): void {
  const estavaAoVivo = logsStore.tailing
  if (estavaAoVivo) logsStore.stopTail()
  logsStore.applyFilters({
    deviceId: deviceId.value,
    severity: logSeverity.value,
    hours: logHours.value,
    search: '',
  })
  if (estavaAoVivo) logsStore.startTail()
}

const autoSetupDialog = ref(false)
const setupDialog = ref(false)

/**
 * Se este equipamento nunca enviou log.
 *
 * "Sem registros na janela" não basta: um roteador configurado pode passar
 * horas em silêncio, e oferecer "ative o log" a quem já ativou seria ruído. O
 * critério inclui a lista de origens — se alguma já resolveu para este
 * dispositivo, ele está configurado, ainda que a janela atual esteja vazia.
 *
 * Enquanto as origens não carregaram, nada é afirmado: um aviso que aparece e
 * some depois de meio segundo é pior do que aparecer meio segundo mais tarde.
 */
const logsNaoConfigurados = computed(() => {
  if (!logsStore.sourcesLoaded) return false
  if (logsStore.sources.some((fonte) => fonte.deviceId === deviceId.value)) return false
  return logsStore.isEmpty
})

/**
 * A aba pedida na URL, quando ainda for aplicável.
 *
 * Se deixar de ser — SNMP que nunca respondeu, VPN removida —, a página volta
 * para `overview` **sem erro e sem conteúdo vazio**, que é a regra de layout do
 * roadmap. Um link antigo continua funcionando; ele só não abre uma aba que não
 * existe mais.
 */
watch(
  [abasAplicaveis, () => route.query.tab],
  ([abas, pedida]) => {
    const alvo = typeof pedida === 'string' ? pedida : activeTab.value
    activeTab.value = abas.includes(alvo) ? alvo : 'overview'
  },
  { immediate: true }
)

// A store é compartilhada com a tela `/logs`: entrar na aba sem fixar o
// dispositivo mostraria o log do parque inteiro dentro da página de um
// aparelho. Sair da aba desliga o tail, que senão seguiria empilhando.
watch(activeTab, (aba, anterior) => {
  if (aba === 'logs') {
    applyLogFilters()
    // As origens dizem se este equipamento já é conhecido pela ingestão, e o
    // diagnóstico de NAT vem junto na mesma resposta.
    void logsStore.fetchSources()
  } else if (anterior === 'logs') logsStore.stopTail()
})

onUnmounted(() => logsStore.stopTail())

// --- Monitores do próprio equipamento --------------------------------------

const monitorDialog = ref(false)
const editingMonitor = ref<DeviceMonitor | null>(null)

function openMonitorDialog(monitor?: DeviceMonitor) {
  editingMonitor.value = monitor ?? null
  monitorDialog.value = true
}

async function onMonitorSaved() {
  if (deviceId.value) await detailStore.loadDeviceDetails(deviceId.value)
}

/** Ações da listagem (testar, ativar/desativar, excluir) só mexem em monitores. */
async function reloadMonitors() {
  if (deviceId.value) await detailStore.reloadMonitors(deviceId.value)
}

// --- Aba VPN ---------------------------------------------------------------

const vpnPeer = computed(() => detailStore.device?.vpnPeer ?? null)
const vpnProfileLabelValue = computed(() =>
  vpnPeer.value ? vpnProfileLabel(vpnPeer.value.deviceProfile) : ''
)
const vpnProfileIconValue = computed(() =>
  vpnPeer.value ? vpnProfileIcon(vpnPeer.value.deviceProfile) : ''
)
const vpnStatusLabelValue = computed(() =>
  vpnPeer.value ? vpnStatusLabel(vpnPeer.value.connectionStatus) : ''
)
const vpnStatusColorValue = computed(() =>
  vpnPeer.value ? vpnStatusColor(vpnPeer.value.connectionStatus) : 'grey'
)
const vpnLastHandshakeText = computed(() =>
  vpnPeer.value ? formatRelativeTime(vpnPeer.value.lastHandshakeAt) : 'nunca'
)

const vpnNeedsFirewallHint = computed(() => {
  const peer = vpnPeer.value
  if (!peer || peer.connectionStatus !== 'connected') return false
  const pingMonitor = detailStore.monitors.find((m) => m.type === 'ping')
  return pingMonitor?.status === 'down'
})

// Série de bps do túnel a partir do histórico gravado pelo scheduler (vpn_rx_bps / vpn_tx_bps).
const vpnTrafficSeries = computed<ChartSeriesInput[]>(() => {
  const rx = detailStore.metrics
    .filter((m) => m.metricName === 'vpn_rx_bps')
    .sort((a, b) => new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime())
  const tx = detailStore.metrics
    .filter((m) => m.metricName === 'vpn_tx_bps')
    .sort((a, b) => new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime())

  const series: ChartSeriesInput[] = []
  if (rx.length > 0) {
    series.push({
      id: 'vpn_rx_bps',
      label: 'Recebido (RX)',
      color: '#4CAF50',
      fillArea: false,
      data: rx.map((m) => ({ time: m.createdAt, value: Number(m.metricValue) || 0 })),
    })
  }
  if (tx.length > 0) {
    series.push({
      id: 'vpn_tx_bps',
      label: 'Enviado (TX)',
      color: '#2196F3',
      fillArea: false,
      data: tx.map((m) => ({ time: m.createdAt, value: Number(m.metricValue) || 0 })),
    })
  }
  return series
})

async function openVpnConfig() {
  if (!vpnPeer.value) return
  const artifact = await vpnStore.fetchConfig(vpnPeer.value.id)
  if (artifact) vpnViewerOpen.value = true
}

async function rotateVpnKeys() {
  if (!vpnPeer.value) return
  if (
    !confirm(
      `Gerar novas chaves para "${detailStore.device?.name}"? A configuração atual deixará de funcionar.`
    )
  ) {
    return
  }

  const artifact = await vpnStore.rotateKeys(vpnPeer.value.id)
  if (artifact) {
    vpnViewerOpen.value = true
    await detailStore.loadDeviceDetails(deviceId.value)
  }
}

async function revokeVpnAccess() {
  if (!vpnPeer.value) return
  if (
    !confirm(
      `Revogar o acesso VPN de "${detailStore.device?.name}"? O túnel cai imediatamente, o IP é liberado e este dispositivo será removido.`
    )
  ) {
    return
  }

  const success = await vpnStore.revokePeer(vpnPeer.value.id)
  if (success) {
    router.push({ name: 'vpn-devices' })
  }
}

async function showVpnFirewallHints() {
  if (!vpnPeer.value) return
  const content = await vpnStore.fetchFirewallHints(vpnPeer.value.id)
  if (!content) return

  vpnFirewallContent.value = content
  vpnFirewallOpen.value = true
}

// Resumo de tráfego por Interface — só as que de fato têm monitor coletando.
// `adminStatus` não serve de filtro aqui: o próprio equipamento o preenche na
// primeira coleta, e toda porta ligada apareceria como monitorada.
const interfaceTrafficSummaries = computed(() => {
  return detailStore.interfaces
    .filter((intf) => intf.isMonitored)
    .map((intf) => {
      const inOctetsMetric = detailStore.metrics.find(
        (m) =>
          (m.metricName === 'ifHCInOctets' || m.metricName === 'ifInOctets') &&
          m.interfaceId === intf.id
      )
      const outOctetsMetric = detailStore.metrics.find(
        (m) =>
          (m.metricName === 'ifHCOutOctets' || m.metricName === 'ifOutOctets') &&
          m.interfaceId === intf.id
      )
      const inBpsMetric = detailStore.metrics.find(
        (m) => m.metricName === 'inBps' && m.interfaceId === intf.id
      )
      const outBpsMetric = detailStore.metrics.find(
        (m) => m.metricName === 'outBps' && m.interfaceId === intf.id
      )

      const inOctets = inOctetsMetric ? Number(inOctetsMetric.metricValue) : 0
      const outOctets = outOctetsMetric ? Number(outOctetsMetric.metricValue) : 0
      const inBps = inBpsMetric ? Number(inBpsMetric.metricValue) : 0
      const outBps = outBpsMetric ? Number(outBpsMetric.metricValue) : 0

      return {
        id: intf.id,
        // O registro de origem viaja junto: é dele que o diálogo tira o estado
        // de monitoramento para oferecer a remoção.
        source: intf,
        ifIndex: intf.snmpIndex ?? intf.ifIndex ?? 0,
        ifName: interfaceLabel(intf),
        operStatus: intf.ifOperStatus || intf.operStatus || 'unknown',
        inBpsFormatted: formatBps(inBps),
        outBpsFormatted: formatBps(outBps),
        inBytesFormatted: formatBytes(inOctets),
        outBytesFormatted: formatBytes(outOctets),
      }
    })
})

function formatMetricValue(metric: DeviceMetric): string {
  return formatMeasuredValue(metric.metricValue, metric.unit)
}

async function openScanModal() {
  scanModalOpen.value = true
  const res = await detailStore.scanDeviceSnmp(deviceId.value)
  if (res) {
    // Reflete o estado real: já monitorado (true), detectado agora mas ainda não habilitado
    // (default ligado, só quando há dado de fato), ou não suportado pelo equipamento (false).
    selectedCpuMonitor.value = res.hasCpuMonitor || res.cpuInfo.usagePercent != null
    selectedMemoryMonitor.value = res.hasMemoryMonitor || res.memoryInfo.usedPercent != null
    selectedIfIndexes.value = res.interfaces.filter((i) => i.isMonitored).map((i) => i.ifIndex)
  }
}

function toggleInterface(ifIndex: number) {
  const idx = selectedIfIndexes.value.indexOf(ifIndex)
  if (idx > -1) {
    selectedIfIndexes.value.splice(idx, 1)
  } else {
    selectedIfIndexes.value.push(ifIndex)
  }
}

function selectAllInterfaces() {
  if (detailStore.scanResult) {
    selectedIfIndexes.value = detailStore.scanResult.interfaces.map((i) => i.ifIndex)
  }
}

function unselectAllInterfaces() {
  selectedIfIndexes.value = []
}

async function saveMonitors() {
  savingMonitors.value = true
  try {
    const success = await detailStore.applySnmpMonitors(deviceId.value, {
      enableCpuMonitor: selectedCpuMonitor.value,
      enableMemoryMonitor: selectedMemoryMonitor.value,
      monitoredIfIndexes: selectedIfIndexes.value,
    })
    if (success) {
      scanModalOpen.value = false
    }
  } finally {
    savingMonitors.value = false
  }
}
</script>

<style scoped>
@media (max-width: 599.98px) {
  .device-action-buttons {
    width: 100%;
  }

  .device-action-buttons :deep(.v-btn) {
    flex: 1 1 0;
    min-width: 0;
  }
}
</style>
