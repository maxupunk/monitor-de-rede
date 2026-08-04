<template>
  <div>
    <!-- Botão de Voltar -->
    <v-btn variant="text" prepend-icon="mdi-arrow-left" class="mb-4" to="/devices">
      Voltar para Dispositivos
    </v-btn>

    <!-- Header do Dispositivo -->
    <v-card elevation="2" class="rounded-lg pa-4 mb-6">
      <div class="d-flex align-center justify-space-between flex-wrap ga-4">
        <div class="d-flex align-center ga-4" style="gap: 16px">
          <v-avatar color="primary" size="48" class="mr-2">
            <v-icon color="white">mdi-router-network</v-icon>
          </v-avatar>
          <div>
            <div class="d-flex align-center ga-3 flex-wrap" style="gap: 12px">
              <h1 class="text-h5 font-weight-bold mr-2">
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
            <div class="text-subtitle-2 text-grey mt-1">
              IP: {{ detailStore.device?.ipAddress || 'Não informado' }} | Tipo:
              {{ detailStore.device?.type }} | Fabricante:
              {{ detailStore.device?.vendor || 'Desconhecido' }}
            </div>
          </div>
        </div>

        <div class="d-flex align-center ga-3" style="gap: 12px">
          <v-btn
            color="info"
            variant="tonal"
            prepend-icon="mdi-radar"
            :loading="detailStore.scanningSnmp"
            @click="openScanModal"
          >
            Escanear SNMP
          </v-btn>

          <v-btn
            color="secondary"
            prepend-icon="mdi-refresh"
            :loading="detailStore.pollingSnmp"
            @click="detailStore.triggerSnmpPoll(deviceId)"
          >
            Poll SNMP Agora
          </v-btn>
        </div>
      </div>
    </v-card>

    <!-- Abas Interativas -->
    <v-card elevation="2" class="rounded-lg">
      <v-tabs v-model="activeTab" color="primary" align-tabs="title">
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
            <v-table hover>
              <thead>
                <tr>
                  <th>Nome</th>
                  <th>Tipo</th>
                  <th>Alvo / Porta</th>
                  <th>Intervalo</th>
                  <th>Status</th>
                  <th>Última Latência</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="mon in detailStore.monitors" :key="mon.id">
                  <td class="font-weight-bold">{{ mon.name }}</td>
                  <td>
                    <v-chip size="x-small" color="info">
                      {{ (mon.type || 'N/A').toUpperCase() }}
                    </v-chip>
                  </td>
                  <td>{{ mon.target }} {{ mon.port ? `:${mon.port}` : '' }}</td>
                  <td>{{ mon.intervalSeconds }}s</td>
                  <td>
                    <v-chip :color="getStatusColor(mon.status)" size="x-small">
                      {{ mon.status || 'UNKNOWN' }}
                    </v-chip>
                  </td>
                  <td>{{ mon.latencyMs !== undefined ? `${mon.latencyMs} ms` : 'N/A' }}</td>
                </tr>
                <tr v-if="detailStore.monitors.length === 0">
                  <td colspan="6" class="text-center text-grey py-4">
                    Nenhum monitor configurado para este equipamento. Clique em "Escanear SNMP".
                  </td>
                </tr>
              </tbody>
            </v-table>
          </v-window-item>

          <!-- Aba Interfaces SNMP -->
          <v-window-item value="interfaces">
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
                      :color="(intf.ifOperStatus || intf.operStatus) === 'up' ? 'success' : 'error'"
                      size="x-small"
                    >
                      Oper: {{ intf.ifOperStatus || intf.operStatus || 'unknown' }}
                    </v-chip>
                  </td>
                  <td>{{ intf.macAddress || 'N/A' }}</td>
                  <td>
                    <v-chip size="x-small" variant="tonal" color="info">
                      {{ formatSpeedNegotiation(intf.ifSpeed || intf.speed) }}
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

            <!-- 2. Tabela de Tráfego por Interface Monitorada -->
            <div
              class="text-subtitle-1 font-weight-bold mb-3 d-flex align-center ga-2"
              style="gap: 8px"
            >
              <v-icon color="primary">mdi-swap-horizontal</v-icon>
              Tráfego de Rede por Interface (Apenas Itens Monitorados)
            </div>

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
                    <v-chip :color="item.operStatus === 'up' ? 'success' : 'error'" size="x-small">
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
                    Nenhuma interface selecionada para monitoramento de tráfego. Clique em "Escanear
                    SNMP" para selecionar.
                  </td>
                </tr>
              </tbody>
            </v-table>

            <!-- 3. Tabela do Histórico Bruto de Registros Recentes -->
            <v-expansion-panels flat variant="inset">
              <v-expansion-panel class="rounded-lg border">
                <v-expansion-panel-title class="font-weight-bold text-subtitle-2">
                  <v-icon start color="primary">mdi-history</v-icon>
                  Histórico de Registros Brutos (Métricas de Itens Monitorados)
                </v-expansion-panel-title>
                <v-expansion-panel-text>
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
                      <tr v-for="met in detailStore.metrics" :key="met.id">
                        <td class="font-weight-medium">{{ met.metricName }}</td>
                        <td>{{ met.interfaceName || 'Sistema / Geral' }}</td>
                        <td class="font-weight-bold">{{ formatMetricValue(met) }}</td>
                        <td>{{ met.unit || '-' }}</td>
                        <td class="text-grey">{{ met.createdAt }}</td>
                      </tr>
                      <tr v-if="detailStore.metrics.length === 0">
                        <td colspan="5" class="text-center text-grey py-4">
                          Nenhum histórico de métricas capturado ainda.
                        </td>
                      </tr>
                    </tbody>
                  </v-table>
                </v-expansion-panel-text>
              </v-expansion-panel>
            </v-expansion-panels>
          </v-window-item>

          <!-- Aba Eventos -->
          <v-window-item value="events">
            <v-table hover>
              <thead>
                <tr>
                  <th>Severidade</th>
                  <th>Mensagem</th>
                  <th>Data/Hora</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="evt in detailStore.events" :key="evt.id">
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
                <tr v-if="detailStore.events.length === 0">
                  <td colspan="3" class="text-center text-grey py-4">
                    Nenhum evento registrado para este dispositivo.
                  </td>
                </tr>
              </tbody>
            </v-table>
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

            <div class="d-flex flex-wrap ga-3" style="gap: 12px">
              <v-btn
                color="primary"
                variant="tonal"
                prepend-icon="mdi-content-copy"
                @click="openVpnConfig"
              >
                Copiar configuração
              </v-btn>
              <v-btn
                color="warning"
                variant="tonal"
                prepend-icon="mdi-key-change"
                @click="rotateVpnKeys"
              >
                Rotacionar chaves
              </v-btn>
              <v-btn
                color="error"
                variant="tonal"
                prepend-icon="mdi-cancel"
                @click="revokeVpnAccess"
              >
                Revogar acesso
              </v-btn>
              <v-spacer></v-spacer>
              <v-btn variant="text" prepend-icon="mdi-open-in-new" :to="{ name: 'vpn-devices' }">
                Ver todos os dispositivos VPN
              </v-btn>
            </div>
          </v-window-item>
        </v-window>
      </v-card-text>
    </v-card>

    <!-- Modal de Escaneamento SNMP -->
    <v-dialog v-model="scanModalOpen" max-width="850px">
      <v-card class="rounded-lg">
        <v-card-title class="d-flex align-center justify-space-between pa-4 bg-primary text-white">
          <div class="d-flex align-center ga-2" style="gap: 8px">
            <v-icon>mdi-radar</v-icon>
            <span>Escaneamento & Descoberta SNMP (OpenWrt)</span>
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
              Consultando interfaces, uso de CPU e memória RAM...
            </div>
          </div>

          <div v-else-if="detailStore.scanResult">
            <!-- Dados do Sistema -->
            <v-alert
              type="info"
              variant="tonal"
              class="mb-4"
              prepend-icon="mdi-router"
              title="Dispositivo Conectado"
              :subtitle="detailStore.scanResult.systemInfo.sysDescr || 'OpenWrt / Router Device'"
            ></v-alert>

            <!-- Recursos de CPU & Memória -->
            <v-card variant="outlined" class="mb-6 rounded-lg pa-4">
              <div
                class="text-subtitle-1 font-weight-bold mb-3 d-flex align-center ga-2"
                style="gap: 8px"
              >
                <v-icon color="primary">mdi-chip</v-icon>
                Monitoramento de Recursos da CPU & Memória
              </div>
              <v-row>
                <v-col cols="12" md="6">
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
                    <span v-if="detailStore.scanResult.cpuInfo.usagePercent !== undefined">
                      - Uso Atual: {{ detailStore.scanResult.cpuInfo.usagePercent }}%
                    </span>
                  </div>
                </v-col>
                <v-col cols="12" md="6">
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
                    <span v-if="detailStore.scanResult.memoryInfo.usedPercent !== undefined">
                      - Uso: {{ detailStore.scanResult.memoryInfo.usedPercent }}%
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
                      {{ formatSpeedNegotiation(iface.ifSpeed) }}
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
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useDeviceDetailStore, type DeviceMetric } from '@/stores/deviceDetail'
import TrafficChartDialog from '@/components/TrafficChartDialog.vue'
import BaseMetricChart, { type ChartSeriesInput } from '@/components/BaseMetricChart.vue'
import VpnScriptViewer from '@/components/VpnScriptViewer.vue'
import VpnFirewallHintsDialog from '@/components/VpnFirewallHintsDialog.vue'
import {
  useVpnStore,
  vpnProfileIcon,
  vpnProfileLabel,
  vpnStatusColor,
  vpnStatusLabel,
  vpnRelativeTime,
} from '@/stores/vpn'

const route = useRoute()
const router = useRouter()
const detailStore = useDeviceDetailStore()
const vpnStore = useVpnStore()
const activeTab = ref('overview')
const scanModalOpen = ref(false)
const savingMonitors = ref(false)

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
const selectedIfIndexes = ref<number[]>([])

const deviceId = computed(() => Number(route.params.id))

onMounted(() => {
  if (deviceId.value) {
    detailStore.loadDeviceDetails(deviceId.value)
    // Traz CIDR e keepalive do servidor VPN para contextualizar a aba VPN, se existir.
    vpnStore.fetchServer()
  }
})

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
  vpnPeer.value ? vpnRelativeTime(vpnPeer.value.lastHandshakeAt) : 'nunca'
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

function formatBytes(bytes?: number): string {
  if (bytes === undefined || bytes === null || isNaN(bytes)) return '0 B'
  if (bytes === 0) return '0 B'
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i]
}

function formatBps(bps?: number): string {
  if (bps === undefined || bps === null || isNaN(bps)) return '0 bps'
  if (bps === 0) return '0 bps'
  const k = 1000
  const sizes = ['bps', 'Kbps', 'Mbps', 'Gbps']
  const i = Math.floor(Math.log(bps) / Math.log(k))
  return parseFloat((bps / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i]
}

function formatSpeedNegotiation(bps?: number | null): string {
  if (!bps || bps <= 0) return 'N/A'
  if (bps >= 1_000_000_000) {
    const gbps = bps / 1_000_000_000
    return `${Number.isInteger(gbps) ? gbps : gbps.toFixed(1)} Gbps`
  }
  if (bps >= 1_000_000) {
    const mbps = bps / 1_000_000
    return `${Number.isInteger(mbps) ? mbps : mbps.toFixed(1)} Mbps`
  }
  if (bps >= 1_000) {
    const kbps = bps / 1_000
    return `${Number.isInteger(kbps) ? kbps : kbps.toFixed(1)} Kbps`
  }
  return `${bps} bps`
}

function formatMetricValue(metric: DeviceMetric): string {
  const val = Number(metric.metricValue)
  if (isNaN(val)) return String(metric.metricValue)

  if (metric.unit === 'bytes') return formatBytes(val)
  if (metric.unit === 'bps') return formatBps(val)
  return `${val} ${metric.unit || ''}`.trim()
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
    selectedCpuMonitor.value = res.hasCpuMonitor || true
    selectedMemoryMonitor.value = res.hasMemoryMonitor || true
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

function getStatusColor(status?: string) {
  switch (status?.toLowerCase()) {
    case 'online':
    case 'up':
      return 'success'
    case 'offline':
    case 'down':
      return 'error'
    case 'warning':
      return 'warning'
    default:
      return 'grey'
  }
}
</script>
