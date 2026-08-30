<template>
  <div>
    <!-- Informações Básicas -->
    <v-row>
      <v-col cols="12" md="6">
        <v-list border class="rounded-lg">
          <v-list-item title="Nome" :subtitle="detailStore.device?.name"></v-list-item>
          <v-list-item title="Endereço IP" :subtitle="detailStore.device?.ipAddress"></v-list-item>
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
          <v-list-item title="Sub-rede / Segmento">
            <template #subtitle>
              <div v-if="deviceNetwork" class="d-flex align-center ga-1">
                <span class="font-weight-medium text-primary">{{ deviceNetwork.name }}</span>
                <span class="font-mono text-caption text-grey">({{ deviceNetwork.cidr }})</span>
                <v-chip
                  v-if="isDeviceNetworkGateway"
                  size="x-small"
                  color="success"
                  variant="tonal"
                  class="ml-1"
                >
                  GATEWAY
                </v-chip>
              </div>
              <span v-else class="text-grey">Não associado a uma sub-rede</span>
            </template>
          </v-list-item>
          <v-list-item title="Está atrás de (Dispositivo Pai)">
            <template #subtitle>
              <router-link
                v-if="detailStore.device?.parent"
                :to="'/devices/' + detailStore.device.parent.id"
                class="text-decoration-none text-primary font-weight-medium d-inline-flex align-center ga-1"
              >
                <v-icon size="16">mdi-sitemap</v-icon>
                {{ detailStore.device.parent.name }}
              </router-link>
              <span v-else class="text-grey">Nenhum (Nó de Topo / Raiz)</span>
            </template>
          </v-list-item>
        </v-list>
      </v-col>
    </v-row>

    <!-- Caminho de Rede & Topologia (Diagnóstico de Rota) -->
    <v-card variant="outlined" class="rounded-lg pa-4 mt-6">
      <div class="d-flex align-center justify-space-between mb-3">
        <div class="text-subtitle-1 font-weight-bold d-flex align-center ga-2">
          <v-icon color="primary">mdi-transit-connection-variant</v-icon>
          Caminho de Rede & Diagnóstico Hierárquico
        </div>
        <div class="d-flex align-center ga-2">
          <v-chip v-if="deviceNetwork" size="x-small" color="info" variant="tonal">
            {{ deviceNetwork.name }} ({{ deviceNetwork.cidr }})
          </v-chip>
          <v-chip size="x-small" color="primary" variant="tonal">Topologia & RCA</v-chip>
        </div>
      </div>

      <div class="d-flex align-center flex-wrap ga-3 pa-3 bg-surface-variant-subtle rounded-lg">
        <!-- Hop 1: NetMonitor Server -->
        <div class="d-flex align-center ga-2 pa-2 rounded bg-surface border">
          <v-avatar color="indigo" size="32">
            <v-icon color="white" size="18">mdi-server-security</v-icon>
          </v-avatar>
          <div>
            <div class="text-caption font-weight-bold">NetMonitor</div>
            <div class="text-caption text-grey">Servidor Central</div>
          </div>
          <v-icon color="success" size="16" class="ml-1">mdi-check-circle</v-icon>
        </div>

        <v-icon color="grey-darken-1" size="20">mdi-arrow-right-bold</v-icon>

        <!-- Hop 2: Parent Device (if configured) -->
        <template v-if="detailStore.device?.parent">
          <router-link
            :to="'/devices/' + detailStore.device.parent.id"
            class="text-decoration-none"
          >
            <div class="d-flex align-center ga-2 pa-2 rounded bg-surface border">
              <v-avatar color="primary" size="32">
                <v-icon color="white" size="18">mdi-router-network</v-icon>
              </v-avatar>
              <div>
                <div class="text-caption font-weight-bold text-primary">
                  {{ detailStore.device.parent.name }}
                </div>
                <div class="text-caption text-grey">Dispositivo Pai (Uplink)</div>
              </div>
              <v-icon color="primary" size="16" class="ml-1">mdi-open-in-new</v-icon>
            </div>
          </router-link>

          <v-icon color="grey-darken-1" size="20">mdi-arrow-right-bold</v-icon>
        </template>

        <!-- Hop 3: Current Device -->
        <div class="d-flex align-center ga-2 pa-2 rounded bg-surface border">
          <v-avatar :color="getStatusColor(detailStore.device?.status)" size="32">
            <v-icon color="white" size="18">
              {{ isDeviceNetworkGateway ? 'mdi-router-network' : 'mdi-lan' }}
            </v-icon>
          </v-avatar>
          <div>
            <div class="d-flex align-center ga-1">
              <span class="text-caption font-weight-bold">{{ detailStore.device?.name }}</span>
              <v-chip
                v-if="isDeviceNetworkGateway"
                size="x-small"
                color="primary"
                variant="flat"
                class="text-xxs"
              >
                GATEWAY
              </v-chip>
            </div>
            <div class="text-caption text-grey font-mono">
              {{ detailStore.device?.ipAddress || 'Host' }}
            </div>
          </div>
          <v-chip
            :color="getStatusColor(detailStore.device?.status)"
            size="x-small"
            variant="tonal"
            class="ml-1"
          >
            {{ (detailStore.device?.status || 'UNKNOWN').toUpperCase() }}
          </v-chip>
        </div>
      </div>

      <div class="text-caption text-medium-emphasis mt-3 d-flex align-center ga-1">
        <v-icon size="14" color="info">mdi-information-outline</v-icon>
        <span>
          A hierarquia de topologia inibe tempestades de alarmes: se o dispositivo pai cair, as
          notificações deste equipamento serão suprimidas para alertar apenas a causa raiz.
        </span>
      </div>
    </v-card>

    <!-- Saúde do equipamento -->
    <template v-if="canHealth">
      <v-divider class="my-6" />
      <DeviceHealthSummary :metrics="detailStore.metrics" />
    </template>

    <!-- Resumo de tráfego por interface monitorada -->
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
                  @click="emit('openInterfaceChart', item.source, 'combined')"
                >
                  <v-icon size="16">mdi-chart-line</v-icon>
                  <v-tooltip activator="parent" location="top"> Ver Gráfico Combinado </v-tooltip>
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
                  @click="emit('openInterfaceChart', item.source, 'inBps')"
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
                  @click="emit('openInterfaceChart', item.source, 'outBps')"
                >
                  <v-icon size="16">mdi-chart-areaspline</v-icon>
                  <v-tooltip activator="parent" location="top"> Gráfico de Upload (OUT) </v-tooltip>
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
                  @click="emit('openInterfaceChart', item.source, 'inOctets')"
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
                  @click="emit('openInterfaceChart', item.source, 'outOctets')"
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

    <!-- Estabilidade dos monitores -->
    <template v-if="detailStore.monitors.length > 0">
      <v-divider class="my-6" />
      <div class="text-subtitle-1 font-weight-bold mb-3 d-flex align-center ga-2">
        <v-icon color="primary">mdi-chart-line</v-icon>
        Estabilidade dos Monitores (24h)
      </div>
      <v-row>
        <v-col v-for="monitor in monitorsWithUptime" :key="monitor.id" cols="12" sm="6" lg="4">
          <v-card variant="outlined" class="rounded-lg pa-4">
            <div class="d-flex align-center justify-space-between mb-2">
              <div class="font-weight-medium text-truncate" :title="monitor.name">
                {{ monitor.name }}
              </div>
              <v-chip
                :color="getUptimeColor(monitor.uptime?.uptimePercentage)"
                size="small"
                variant="tonal"
              >
                {{ formatUptime(monitor.uptime?.uptimePercentage) }}%
              </v-chip>
            </div>
            <div class="text-caption text-grey">
              {{ monitor.uptime?.totalChecks ?? 0 }} checagens ·
              {{ monitor.uptime?.upChecks ?? 0 }} up · {{ monitor.uptime?.downChecks ?? 0 }} down
            </div>
            <v-progress-linear
              :model-value="monitor.uptime?.uptimePercentage ?? 100"
              :color="getUptimeColor(monitor.uptime?.uptimePercentage)"
              height="6"
              rounded
              class="mt-2"
            ></v-progress-linear>
          </v-card>
        </v-col>
      </v-row>
    </template>

    <!-- Alerta de SNMP configurado sem resposta -->
    <v-alert
      v-if="canSnmpConfigured && !canSnmpConnected"
      type="info"
      variant="tonal"
      density="comfortable"
      class="rounded-lg mt-4"
    >
      <div class="font-weight-medium">SNMP configurado, mas ainda sem resposta</div>
      <div class="text-caption">
        O inventário de interfaces e o tráfego aparecem depois da primeira comunicação bem-sucedida.
        Verifique a comunidade e o alcance de rede e execute uma varredura.
      </div>
      <template #append>
        <v-btn
          size="small"
          variant="tonal"
          color="primary"
          :loading="detailStore.scanningSnmp"
          @click="emit('openScanModal')"
        >
          Varrer agora
        </v-btn>
      </template>
    </v-alert>

    <!-- Tabela do Histórico Bruto de Registros Recentes -->
    <v-card elevation="2" class="rounded-lg pa-4 border mt-4">
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
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue'
import {
  useDeviceDetailStore,
  type DeviceInterface,
  type DeviceMetric,
} from '@/stores/deviceDetail'
import { useMonitorsStore } from '@/stores/monitors'
import { useNetworksStore } from '@/stores/networks'
import DeviceHealthSummary from '@/components/devices/DeviceHealthSummary.vue'
import { formatBps, formatBytes, formatMeasuredValue } from '@/utils/formatters'
import { getStatusColor } from '@/utils/monitorPresentation'
import { useInfiniteList } from '@/composables/useInfiniteList'
import type { MonitorUptimeResponse } from '@/bindings/MonitorUptimeResponse'

const props = defineProps<{
  deviceId: number
  canHealth: boolean
  canSnmpConfigured: boolean
  canSnmpConnected: boolean
}>()

const emit = defineEmits<{
  (e: 'openScanModal'): void
  (
    e: 'openInterfaceChart',
    intf: DeviceInterface,
    metricType?: 'inBps' | 'outBps' | 'inOctets' | 'outOctets' | 'combined'
  ): void
}>()

const detailStore = useDeviceDetailStore()
const monitorsStore = useMonitorsStore()
const networksStore = useNetworksStore()

const deviceNetwork = computed(() => {
  const dev = detailStore.device
  if (!dev) return null
  if (dev.networkId) {
    return networksStore.networks.find((n) => n.id === dev.networkId) || null
  }
  const ip = dev.ipAddress?.trim()
  if (!ip || !ip.includes('.')) return null
  const parts = ip.split('.')
  if (parts.length !== 4) return null
  const prefix = `${parts[0]}.${parts[1]}.${parts[2]}.`
  return networksStore.networks.find((n) => n.cidr?.startsWith(prefix)) || null
})

const isDeviceNetworkGateway = computed(() => {
  if (!deviceNetwork.value?.gateway || !detailStore.device?.ipAddress) return false
  return deviceNetwork.value.gateway.trim() === detailStore.device.ipAddress.trim()
})

onMounted(() => {
  if (networksStore.networks.length === 0) {
    void networksStore.fetchNetworks()
  }
})

const uptimeByMonitor = ref<Record<number, MonitorUptimeResponse>>({})
const uptimeLoading = ref(false)

const showMetricsHistory = ref(false)
const metricsHistory = useInfiniteList<DeviceMetric>(() => `/devices/${props.deviceId}/metrics`, {
  label: 'histórico de métricas',
})

const monitorsWithUptime = computed(() =>
  detailStore.monitors.map((monitor) => ({
    ...monitor,
    uptime: uptimeByMonitor.value[monitor.id] ?? null,
  }))
)

function getUptimeColor(value?: number | null): string {
  if (value == null) return 'grey'
  if (value >= 99.0) return 'success'
  if (value >= 95.0) return 'warning'
  return 'error'
}

function formatUptime(value?: number | null): string {
  if (value == null) return '0.0'
  return value.toFixed(1)
}

async function loadUptime() {
  const monitors = detailStore.monitors.filter((m) => m.isEnabled !== false)
  if (monitors.length === 0) return
  uptimeLoading.value = true
  try {
    const results = await Promise.all(
      monitors.map(async (monitor) => {
        const uptime = await monitorsStore.fetchUptime(monitor.id, 24)
        return { id: monitor.id, uptime }
      })
    )
    for (const { id, uptime } of results) {
      if (uptime) {
        uptimeByMonitor.value[id] = uptime
      }
    }
  } finally {
    uptimeLoading.value = false
  }
}

onMounted(loadUptime)
watch(() => detailStore.monitors.map((m) => m.id).join(','), loadUptime, { immediate: true })

function toggleShowMetricsHistory() {
  showMetricsHistory.value = !showMetricsHistory.value
  if (showMetricsHistory.value) metricsHistory.reset()
}

function interfaceLabel(intf: DeviceInterface): string {
  return intf.ifName || intf.name || `if-${intf.id}`
}

function formatMetricValue(metric: DeviceMetric): string {
  return formatMeasuredValue(metric.metricValue, metric.unit)
}

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
</script>
