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

        <div class="d-flex flex-wrap align-center ga-2 w-100 w-md-auto">
          <v-btn
            color="info"
            variant="tonal"
            prepend-icon="mdi-radar"
            size="small"
            :loading="detailStore.scanningSnmp"
            class="flex-grow-1 flex-md-grow-0"
            @click="openScanModal"
          >
            <span class="hidden-sm-and-down">Escanear SNMP</span>
            <span class="hidden-md-and-up">SNMP</span>
          </v-btn>

          <v-btn
            color="secondary"
            prepend-icon="mdi-refresh"
            size="small"
            :loading="detailStore.pollingSnmp"
            class="flex-grow-1 flex-md-grow-0"
            @click="detailStore.triggerSnmpPoll(deviceId)"
          >
            <span class="hidden-sm-and-down">Coletar SNMP agora</span>
            <span class="hidden-md-and-up">Coletar</span>
          </v-btn>

          <v-btn
            color="purple"
            variant="tonal"
            prepend-icon="mdi-lan-connect"
            size="small"
            class="flex-grow-1 flex-md-grow-0"
            @click="portScanOpen = true"
          >
            <span class="hidden-sm-and-down">Escanear Portas</span>
            <span class="hidden-md-and-up">Portas</span>
          </v-btn>

          <v-btn
            color="primary"
            variant="outlined"
            prepend-icon="mdi-pencil"
            size="small"
            class="flex-grow-1 flex-md-grow-0"
            @click="editDeviceDialog = true"
          >
            <span class="hidden-sm-and-down">Editar Dispositivo</span>
            <span class="hidden-md-and-up">Editar</span>
          </v-btn>
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
        <v-tab value="interfaces" prepend-icon="mdi-expansion-card">
          Interfaces SNMP ({{ detailStore.interfaces.length }})
        </v-tab>
        <v-tab value="metrics" prepend-icon="mdi-chart-line">Métricas & Tráfego</v-tab>
        <v-tab value="events" prepend-icon="mdi-history">Histórico de Eventos</v-tab>
        <v-tab v-if="vpnPeer" value="vpn" prepend-icon="mdi-shield-lock-outline">VPN</v-tab>
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
          </v-window-item>

          <!-- Aba Monitores -->
          <v-window-item value="monitors">
            <div class="d-flex justify-end pa-3">
              <v-btn
                color="primary"
                variant="tonal"
                size="small"
                prepend-icon="mdi-plus"
                @click="openMonitorDialog()"
              >
                Novo monitor neste equipamento
              </v-btn>
            </div>
            <MonitorsTable
              :monitors="detailStore.monitors"
              :loading="detailStore.loading"
              variant="device"
              no-data-text="Nenhum monitor configurado para este equipamento. Crie um acima ou use &quot;Escanear SNMP&quot; para descobrir automaticamente."
              @edit="openMonitorDialog"
              @changed="reloadMonitors"
            ></MonitorsTable>
          </v-window-item>

          <!-- Aba Interfaces SNMP -->
          <v-window-item value="interfaces">
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
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="intf in detailStore.interfaces" :key="intf.id">
                    <td>{{ intf.ifIndex ?? intf.snmpIndex ?? '-' }}</td>
                    <td class="font-weight-bold">{{ intf.ifName || intf.name || '-' }}</td>
                    <td>
                      <v-chip
                        :color="intf.adminStatus === 'up' ? 'primary' : 'grey'"
                        size="x-small"
                        variant="tonal"
                      >
                        {{ intf.adminStatus === 'up' ? 'MONITORADA' : 'NÃO MONITORADA' }}
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
                  </tr>
                  <tr v-if="detailStore.interfaces.length === 0">
                    <td colspan="6" class="text-center text-grey py-4">
                      Nenhuma interface SNMP encontrada. Clique em "Escanear SNMP".
                    </td>
                  </tr>
                </tbody>
              </v-table>
            </div>
          </v-window-item>

          <!-- Aba Métricas & Tráfego -->
          <v-window-item value="metrics">
            <!-- 1. KPIs de Recursos do Sistema -->
            <div
              class="text-subtitle-1 font-weight-bold mb-3 d-flex align-center ga-2"
              style="gap: 8px"
            >
              <v-icon color="primary">mdi-chip</v-icon>
              Recursos de Hardware & Processamento
            </div>

            <v-row class="mb-6">
              <!-- CPU Card -->
              <v-col cols="12" md="6">
                <v-tooltip location="top" color="#0F172A" :disabled="!isCpuMonitored">
                  <template #activator="{ props: tooltipProps }">
                    <v-card
                      v-bind="tooltipProps"
                      border
                      flat
                      class="pa-4 rounded-lg cursor-pointer"
                    >
                      <div class="d-flex align-center justify-space-between mb-2">
                        <span class="text-subtitle-2 font-weight-bold">Uso de CPU</span>
                        <v-chip
                          size="x-small"
                          :color="isCpuMonitored ? getCpuColor(cpuUsageValue) : 'grey'"
                        >
                          {{
                            isCpuMonitored
                              ? cpuUsageValue !== null
                                ? `${cpuUsageValue}%`
                                : 'Sem dados'
                              : 'Não Monitorado'
                          }}
                        </v-chip>
                      </div>
                      <v-progress-linear
                        :model-value="isCpuMonitored ? cpuUsageValue || 0 : 0"
                        height="10"
                        rounded
                        :color="isCpuMonitored ? getCpuColor(cpuUsageValue) : 'grey-lighten-2'"
                        class="mb-3"
                      ></v-progress-linear>
                      <MonitorSparkline
                        v-if="isCpuMonitored && cpuUsageHistory.length > 1"
                        :data="cpuUsageHistory"
                        :color="getCpuHexColor(cpuUsageValue)"
                        :width="220"
                        :height="32"
                        class="mb-3"
                      />
                      <div class="d-flex align-center justify-space-between text-caption text-grey">
                        <span v-if="isCpuMonitored"
                        >Load 1 min:
                          {{ cpuLoadValue !== null ? `${cpuLoadValue} load` : 'N/A' }}</span
                        >
                        <span v-else>Recurso desativado na varredura</span>
                        <span>Coleta: {{ cpuUsageMetric?.createdAt || 'N/A' }}</span>
                      </div>
                    </v-card>
                  </template>
                  <div class="pa-2 text-white" style="font-size: 12px">
                    <div class="font-weight-bold mb-1" style="font-size: 13px; color: #38bdf8">
                      Consumo de CPU: {{ cpuUsageValue !== null ? `${cpuUsageValue}%` : 'N/A' }}
                    </div>
                    <div style="font-size: 11px; color: #cbd5e1" class="mb-1">
                      <v-icon size="12" color="#94a3b8" class="mr-1">mdi-clock-outline</v-icon>
                      Data e Hora: {{ cpuUsageMetric?.createdAt || 'N/A' }}
                    </div>
                    <div style="font-size: 11px; color: #94a3b8">
                      Load 1 min: {{ cpuLoadValue !== null ? `${cpuLoadValue} load` : 'N/A' }}
                    </div>
                  </div>
                </v-tooltip>
              </v-col>

              <!-- Memória RAM Card -->
              <v-col cols="12" md="6">
                <v-tooltip location="top" color="#0F172A" :disabled="!isMemoryMonitored">
                  <template #activator="{ props: tooltipProps }">
                    <v-card
                      v-bind="tooltipProps"
                      border
                      flat
                      class="pa-4 rounded-lg cursor-pointer"
                    >
                      <div class="d-flex align-center justify-space-between mb-2">
                        <span class="text-subtitle-2 font-weight-bold">Uso de Memória RAM</span>
                        <v-chip
                          size="x-small"
                          :color="isMemoryMonitored ? getMemoryColor(memoryUsageValue) : 'grey'"
                        >
                          {{
                            isMemoryMonitored
                              ? memoryUsageValue !== null
                                ? `${memoryUsageValue}%`
                                : 'Sem dados'
                              : 'Não Monitorado'
                          }}
                        </v-chip>
                      </div>
                      <v-progress-linear
                        :model-value="isMemoryMonitored ? memoryUsageValue || 0 : 0"
                        height="10"
                        rounded
                        :color="
                          isMemoryMonitored ? getMemoryColor(memoryUsageValue) : 'grey-lighten-2'
                        "
                        class="mb-3"
                      ></v-progress-linear>
                      <MonitorSparkline
                        v-if="isMemoryMonitored && memoryUsageHistory.length > 1"
                        :data="memoryUsageHistory"
                        :color="getMemoryHexColor(memoryUsageValue)"
                        :width="220"
                        :height="32"
                        class="mb-3"
                      />
                      <div class="d-flex align-center justify-space-between text-caption text-grey">
                        <span v-if="isMemoryMonitored">Percentual Utilizado</span>
                        <span v-else>Recurso desativado na varredura</span>
                        <span>Coleta: {{ memoryUsageMetric?.createdAt || 'N/A' }}</span>
                      </div>
                    </v-card>
                  </template>
                  <div class="pa-2 text-white" style="font-size: 12px">
                    <div class="font-weight-bold mb-1" style="font-size: 13px; color: #38bdf8">
                      Consumo de Memória RAM:
                      {{ memoryUsageValue !== null ? `${memoryUsageValue}%` : 'N/A' }}
                    </div>
                    <div style="font-size: 11px; color: #cbd5e1">
                      <v-icon size="12" color="#94a3b8" class="mr-1">mdi-clock-outline</v-icon>
                      Data e Hora: {{ memoryUsageMetric?.createdAt || 'N/A' }}
                    </div>
                  </div>
                </v-tooltip>
              </v-col>
            </v-row>

            <!-- 1.5 Métricas do Template Zabbix (genérico) — apenas se o dispositivo tiver um template vinculado -->
            <template v-if="templateMetricCards.length > 0">
              <div
                class="text-subtitle-1 font-weight-bold mb-3 d-flex align-center ga-2"
                style="gap: 8px"
              >
                <v-icon color="primary">mdi-file-cog-outline</v-icon>
                {{ detailStore.device?.zabbixTemplate?.name || 'Métricas do Template' }}
              </div>

              <v-row class="mb-6">
                <v-col
                  v-for="card in templateMetricCards"
                  :key="card.label"
                  cols="12"
                  sm="6"
                  md="3"
                >
                  <v-card border flat class="pa-4 rounded-lg h-100">
                    <div class="d-flex align-center justify-space-between mb-2">
                      <span class="text-caption text-grey-darken-1 font-weight-medium">{{
                        card.label
                      }}</span>
                      <v-avatar color="info" variant="tonal" size="30">
                        <v-icon size="16">mdi-gauge</v-icon>
                      </v-avatar>
                    </div>
                    <div class="text-h6 font-weight-bold text-info">
                      {{ card.value }}
                    </div>
                    <div class="text-caption text-grey">{{ card.createdAt || 'Sem dados' }}</div>
                  </v-card>
                </v-col>
              </v-row>
            </template>

            <!-- 2. Tabela de Tráfego por Interface Monitorada -->
            <div
              class="text-subtitle-1 font-weight-bold mb-3 d-flex align-center ga-2"
              style="gap: 8px"
            >
              <v-icon color="primary">mdi-swap-horizontal</v-icon>
              Tráfego de Rede por Interface (Apenas Itens Monitorados)
            </div>

            <div class="table-responsive">
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
                          @click="openTrafficChart(item, 'combined')"
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
                          @click="openTrafficChart(item, 'inBps')"
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
                          @click="openTrafficChart(item, 'outBps')"
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
                          @click="openTrafficChart(item, 'inOctets')"
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
                          @click="openTrafficChart(item, 'outOctets')"
                        >
                          <v-icon size="16">mdi-chart-box-outline</v-icon>
                          <v-tooltip activator="parent" location="top">
                            Gráfico Volumetria Saída
                          </v-tooltip>
                        </v-btn>
                      </div>
                    </td>
                  </tr>
                  <tr v-if="interfaceTrafficSummaries.length === 0">
                    <td colspan="6" class="text-center text-grey py-6">
                      Nenhuma interface selecionada para monitoramento de tráfego. Clique em
                      "Escanear SNMP" para selecionar.
                    </td>
                  </tr>
                </tbody>
              </v-table>
            </div>

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
            <div class="text-caption text-grey">
              Consultando interfaces, uso de CPU/memória e itens do template Zabbix vinculado...
            </div>
          </div>

          <div v-else-if="detailStore.scanResult">
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

            <!-- Itens do Template Zabbix vinculado (tensão, corrente, etc.) -->
            <v-card
              v-if="detailStore.scanResult.zabbixTemplateItems.length > 0"
              variant="outlined"
              class="mb-6 rounded-lg pa-4"
            >
              <div
                class="text-subtitle-1 font-weight-bold mb-3 d-flex align-center ga-2"
                style="gap: 8px"
              >
                <v-icon color="primary">mdi-file-cog-outline</v-icon>
                Itens do Template Zabbix — {{ detailStore.device?.zabbixTemplate?.name }}
              </div>
              <div class="text-caption text-grey mb-3">
                Coletados automaticamente a cada ciclo de polling — não é preciso habilitar
                individualmente.
              </div>
              <v-row>
                <v-col
                  v-for="item in detailStore.scanResult.zabbixTemplateItems"
                  :key="item.id"
                  cols="12"
                  sm="6"
                  md="4"
                >
                  <div class="pa-3 border rounded-lg">
                    <div class="text-caption text-grey-darken-1">{{ item.name }}</div>
                    <div
                      class="text-subtitle-1 font-weight-bold"
                      :class="item.value != null ? 'text-primary' : 'text-grey'"
                    >
                      {{
                        item.value != null
                          ? `${item.value}${item.units ? ` ${item.units}` : ''}`
                          : 'Sem resposta'
                      }}
                    </div>
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
      :interface-id="selectedChartInterfaceId"
      :interface-name="selectedChartInterfaceName"
      :initial-metric="selectedChartMetricType"
      :metrics="detailStore.metrics"
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
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useDeviceDetailStore, type DeviceMetric, type DeviceMonitor } from '@/stores/deviceDetail'
import TrafficChartDialog from '@/components/TrafficChartDialog.vue'
import BaseMetricChart, { type ChartSeriesInput } from '@/components/BaseMetricChart.vue'
import MonitorSparkline from '@/components/MonitorSparkline.vue'
import VpnScriptViewer from '@/components/VpnScriptViewer.vue'
import VpnFirewallHintsDialog from '@/components/VpnFirewallHintsDialog.vue'
import PortScanDialog from '@/components/PortScanDialog.vue'
import MonitorFormDialog from '@/components/MonitorFormDialog.vue'
import DeviceDialog from '@/components/DeviceDialog.vue'
import MonitorsTable from '@/components/MonitorsTable.vue'
import { getStatusColor, gaugeHexColor } from '@/utils/monitorPresentation'
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
const selectedChartInterfaceId = ref<number | null>(null)
const selectedChartInterfaceName = ref('')
const selectedChartMetricType = ref<'inBps' | 'outBps' | 'inOctets' | 'outOctets' | 'combined'>(
  'inBps'
)

const vpnViewerOpen = ref(false)
const vpnFirewallOpen = ref(false)
const vpnFirewallContent = ref('')

function openTrafficChart(
  item: { id: number; ifName: string },
  metricType: 'inBps' | 'outBps' | 'inOctets' | 'outOctets' | 'combined'
) {
  selectedChartInterfaceId.value = item.id
  selectedChartInterfaceName.value = item.ifName
  selectedChartMetricType.value = metricType
  chartDialogOpen.value = true
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

const isCpuMonitored = computed(() =>
  detailStore.monitors.some((m) => m.name.toLowerCase().includes('cpu') && m.status !== 'disabled')
)

const isMemoryMonitored = computed(() =>
  detailStore.monitors.some((m) => {
    const name = m.name
      .normalize('NFD')
      .replace(/[\u0300-\u036f]/g, '')
      .toLowerCase()
    return (
      (name.includes('memoria') || name.includes('memory')) &&
      m.enabled !== false &&
      m.status !== 'disabled'
    )
  })
)

// Métricas de CPU
const cpuUsageMetric = computed(() => detailStore.metrics.find((m) => m.metricName === 'cpu_usage'))
const cpuUsageValue = computed(() =>
  cpuUsageMetric.value !== undefined ? Number(cpuUsageMetric.value.metricValue) : null
)

const cpuLoadMetric = computed(() =>
  detailStore.metrics.find((m) => m.metricName === 'cpu_load_1min')
)
const cpuLoadValue = computed(() =>
  cpuLoadMetric.value !== undefined ? Number(cpuLoadMetric.value.metricValue) : null
)

// Métricas de Memória
const memoryUsageMetric = computed(() =>
  detailStore.metrics.find((m) => m.metricName === 'memory_usage')
)
const memoryUsageValue = computed(() =>
  memoryUsageMetric.value !== undefined ? Number(memoryUsageMetric.value.metricValue) : null
)

/** Teto de amostras exibidas na mini tendência dos cards de CPU/Memória */
const GAUGE_SPARKLINE_LIMIT = 30

// `detailStore.metrics` vem do mais recente para o mais antigo (ver devices_controller.ts) —
// a mini tendência precisa do sentido contrário para o tempo fluir da esquerda para a direita.
function gaugeSparklineHistory(metricName: string) {
  return detailStore.metrics
    .filter((m) => m.metricName === metricName)
    .slice(0, GAUGE_SPARKLINE_LIMIT)
    .reverse()
    .map((m) => ({ value: Number(m.metricValue) || 0, recordedAt: m.createdAt }))
}

const cpuUsageHistory = computed(() => gaugeSparklineHistory('cpu_usage'))
const memoryUsageHistory = computed(() => gaugeSparklineHistory('memory_usage'))

function getCpuHexColor(usage?: number | null): string {
  return gaugeHexColor(usage ?? null, 'cpu_usage')
}

function getMemoryHexColor(usage?: number | null): string {
  return gaugeHexColor(usage ?? null, 'memory_usage')
}

// --- Métricas dirigidas por Template Zabbix (genérico) ---
// O dispositivo pode ter um Template Zabbix importado vinculado (ver /zabbix-templates);
// cada item do template vira um card com o valor mais recente coletado via SNMP
// (ver modules/zabbix/zabbix_template_collector.ts).
interface TemplateMetricCard {
  label: string
  value: string
  createdAt?: string
}

const templateMetricCards = computed<TemplateMetricCard[]>(() => {
  const items = detailStore.device?.zabbixTemplate?.items
  if (!items || items.length === 0) return []

  return items.map((item) => {
    const metric = detailStore.metrics.find((m) => m.metricName === item.key)
    const value = metric ? Number(metric.metricValue) : NaN
    return {
      label: item.name,
      value: !isNaN(value) ? `${value}${item.units ? ` ${item.units}` : ''}` : 'N/A',
      createdAt: metric?.createdAt,
    }
  })
})

// Resumo de tráfego por Interface — exibe apenas interfaces selecionadas para monitoramento (adminStatus === 'up')
const interfaceTrafficSummaries = computed(() => {
  return detailStore.interfaces
    .filter((intf) => intf.adminStatus === 'up')
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
        ifIndex: intf.snmpIndex ?? intf.ifIndex ?? 0,
        ifName: intf.ifName || intf.name || `if-${intf.id}`,
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

function getCpuColor(usage?: number | null) {
  if (usage === null || usage === undefined) return 'grey'
  if (usage > 85) return 'error'
  if (usage > 65) return 'warning'
  return 'success'
}

function getMemoryColor(usage?: number | null) {
  if (usage === null || usage === undefined) return 'grey'
  if (usage > 90) return 'error'
  if (usage > 75) return 'warning'
  return 'success'
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
