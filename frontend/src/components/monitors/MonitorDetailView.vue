<template>
  <div>
    <!--
      O atalho de saída só existe na navegação por rota. Dentro de um diálogo
      quem fecha é o "X" do cabeçalho, e um segundo botão de voltar no corpo
      da janela seria uma ação duplicada.
    -->
    <v-btn
      v-if="!embedded"
      variant="text"
      prepend-icon="mdi-arrow-left"
      class="mb-4"
      to="/monitors"
    >
      Voltar para Monitores
    </v-btn>

    <!-- Loading State -->
    <v-card
      v-if="monitorsStore.loading && !monitorsStore.currentMonitor"
      elevation="2"
      class="pa-8 text-center rounded-lg"
    >
      <v-progress-circular indeterminate color="primary" size="48"></v-progress-circular>
      <div class="mt-4 text-subtitle-1 text-grey">
        Carregando métricas e histórico do monitor...
      </div>
    </v-card>

    <div v-else-if="monitorsStore.currentMonitor">
      <!-- Header do Monitor -->
      <MonitorDetailHeader
        :monitor="monitor"
        :header-chip="headerChip"
        :type-icon="typeIcon"
        :type-text="typeText"
        :formatted-target="formattedTarget"
        :running="monitorsStore.runningId === monitor.id"
        @test="monitorsStore.runMonitor(monitor.id)"
        @edit="editDialog = true"
        @toggle-enabled="monitorsStore.toggleMonitorEnabled(monitor.id, !monitor.isEnabled)"
        @delete="confirmDelete"
      />

      <!-- Instabilidade histórica do alvo: "oscilou 12x nas últimas 24h" -->
      <InstabilityIndicator :scope-key="`monitor:${monitor.id}`" />

      <!-- Cards de Métricas KPI -->
      <MonitorKpiCards
        :is-traffic-monitor="isTrafficMonitor"
        :is-gauge-monitor="isGaugeMonitor"
        :is-interface-monitor="isInterfaceMonitor"
        :traffic-in-text="trafficInText"
        :traffic-out-text="trafficOutText"
        :interface-speed-text="interfaceSpeedText"
        :interface-oper-text="interfaceOperText"
        :header-chip="headerChip"
        :gauge-color-value="gaugeColorValue"
        :gauge-current-text="gaugeCurrentText"
        :gauge-avg-text="gaugeAvgText"
        :gauge-min-text="gaugeMinText"
        :gauge-max-text="gaugeMaxText"
        :stats="stats"
        :interface-flap-count="interfaceFlapCount"
        :latency-kpi-titles="latencyKpiTitles"
        :last-latency-text="lastLatencyText"
        :avg-latency-text="avgLatencyText"
        :min-latency-text="minLatencyText"
        :max-latency-text="maxLatencyText"
      />

      <!-- Linha de Base Estatística & Detecção de Anomalias (§2.3.3) -->
      <MonitorBaselineCard
        v-if="!isGaugeMonitor && !isInterfaceMonitor && !isTrafficMonitor"
        :baseline-data="baselineData"
        @create-anomaly-rule="showRuleCatalog = true"
      />

      <!-- Seção de Gráficos e Timeline -->
      <MonitorChartsSection
        :is-traffic-monitor="isTrafficMonitor"
        :is-gauge-monitor="isGaugeMonitor"
        :is-interface-monitor="isInterfaceMonitor"
        :traffic-tab="trafficTab"
        :traffic-series="trafficSeries"
        :recent-results="monitor.recentResults || []"
        :status-breakdown="statusBreakdown"
        :total-checks="stats.totalChecks"
        :gauge-type="gaugeTypeLabel(monitor)"
        :gauge-avg="gaugeStats.avg"
        :gauge-avg-text="gaugeAvgText"
        :gauge-unit-type="gaugeUnitType"
        :gauge-series="gaugeSeries"
        :avg-latency="stats.avgLatency"
        :latency-series="latencySeries"
        :link-interface-label="deviceLinkInterfaceName"
        :link-traffic-tab="linkTrafficTab"
        :link-traffic-series="linkTrafficSeries"
        :latest-link-in-bps="latestLinkInBps"
        :latest-link-out-bps="latestLinkOutBps"
        @update:traffic-tab="trafficTab = $event"
        @update:link-traffic-tab="linkTrafficTab = $event"
      />

      <!-- Mapa de Calor de Latência por Hora do Dia (§2.2.2) -->
      <div v-if="!isGaugeMonitor && !isTrafficMonitor && !isInterfaceMonitor" class="mb-6">
        <SaasLatencyHeatmapWidget :monitor-id="monitor.id" />
      </div>

      <!-- Tabela com Histórico de Verificações Recentes -->
      <MonitorHistoryTable
        :show="showHistory"
        :history="history"
        :loading="monitorsStore.loading"
        @toggle="toggleShowHistory"
        @refresh="refreshData"
      />

      <!-- Tabela com Histórico de Alertas -->
      <MonitorAlertHistoryTable
        :show="showAlerts"
        :alert-history="alertHistory"
        :loading="monitorsStore.loading"
        :alerts-store-loading="alertsStore.loading"
        :silence-durations="silenceDurations"
        @toggle="toggleShowAlerts"
        @refresh="refreshData"
        @acknowledge="acknowledgeAlertItem"
        @silence="silenceAlertItem"
      />
    </div>

    <!-- State de Erro / Não Encontrado -->
    <v-card v-else elevation="2" class="pa-8 text-center rounded-lg">
      <v-icon size="64" color="error" class="mb-4">mdi-alert-circle-outline</v-icon>
      <div class="text-h6 text-error">Monitor não encontrado</div>
      <div class="text-body-2 text-grey mt-1">O monitor solicitado não existe ou foi removido.</div>
      <v-btn v-if="!embedded" color="primary" class="mt-4" to="/monitors">
        Voltar para Monitores
      </v-btn>
    </v-card>

    <!-- Dialog de Edição do Monitor -->
    <MonitorFormDialog
      v-model="editDialog"
      :monitor="monitorsStore.currentMonitor"
      @saved="refreshData"
    ></MonitorFormDialog>

    <!-- Dialog do Catálogo de Regras para Criação de Regra de Anomalia -->
    <AlertRuleCatalogDialog
      v-model="showRuleCatalog"
      :fixed-device-id="monitor?.deviceId"
      :fixed-device-name="monitor?.device?.name"
      @applied="refreshData"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useMonitorsStore, type Monitor, type MonitorResult } from '@/stores/monitors'
import { useEventsStore } from '@/stores/events'
import { useAlertsStore } from '@/stores/alerts'
import { apiService } from '@/services/apiService'
import { useInfiniteList } from '@/composables/useInfiniteList'
import { confirm } from '@/composables/useConfirm'
import { useDevicesStore, type Device } from '@/stores/devices'
import type { DeviceMetric } from '@/stores/deviceDetail'
import type { ChartSeriesInput } from '@/components/BaseMetricChart.vue'
import MonitorFormDialog from '@/components/MonitorFormDialog.vue'
import AlertRuleCatalogDialog from '@/components/AlertRuleCatalogDialog.vue'
import InstabilityIndicator from '@/components/InstabilityIndicator.vue'
import MonitorDetailHeader from './detail/MonitorDetailHeader.vue'
import MonitorKpiCards from './detail/MonitorKpiCards.vue'
import MonitorBaselineCard, { type MonitorBaselinePayload } from './detail/MonitorBaselineCard.vue'
import MonitorChartsSection from './detail/MonitorChartsSection.vue'
import SaasLatencyHeatmapWidget from '@/components/widgets/SaasLatencyHeatmapWidget.vue'
import MonitorHistoryTable from './detail/MonitorHistoryTable.vue'
import MonitorAlertHistoryTable from './detail/MonitorAlertHistoryTable.vue'
import {
  isGaugeMonitor as isGaugeMonitorFn,
  isTrafficMonitor as isTrafficMonitorFn,
  gaugeMetricName,
  gaugeTypeLabel,
  gaugeUsagePercent,
  formatGaugeValue,
  gaugeColor as gaugeColorFn,
  isInterfaceMonitor as isInterfaceMonitorFn,
  interfaceStatusInfo,
  latestResultData,
  getStatusColor,
} from '@/utils/monitorPresentation'
import { formatBinaryBytes, formatDateTime, formatLatency, formatBps } from '@/utils/formatters'
import type { AlertEvent } from '@/stores/alerts'

const props = defineProps<{
  /** Id do monitor. Ausente = veio da rota `/monitors/:id`. */
  monitorId?: number
  /** Renderizado dentro de um diálogo: some o "voltar", o fechar é do host. */
  embedded?: boolean
}>()

const emit = defineEmits<{ (e: 'closed'): void }>()

const route = useRoute()
const router = useRouter()
const monitorsStore = useMonitorsStore()
const eventsStore = useEventsStore()
const alertsStore = useAlertsStore()
const devicesStore = useDevicesStore()
const associatedDevice = ref<Device | null>(null)

const deviceLinkInterfaceId = computed(() => associatedDevice.value?.linkInterfaceId || null)
const deviceLinkInterfaceName = computed(() => {
  if (associatedDevice.value?.linkInterfaceName) return associatedDevice.value.linkInterfaceName
  if (!deviceLinkInterfaceId.value) return null
  const fromHistory = gaugeHistory.value.find(
    (m) => m.interfaceId === deviceLinkInterfaceId.value
  )?.interfaceName
  return fromHistory || `Interface #${deviceLinkInterfaceId.value}`
})

const monitorId = computed(() => props.monitorId ?? Number(route.params.id))
const editDialog = ref(false)

const emptyMonitor: Monitor = {
  id: 0,
  deviceId: 0,
  name: '',
  type: 'ping',
  target: '',
  port: undefined,
  configuration: {},
  intervalSeconds: 60,
  timeoutSeconds: 5,
  status: 'unknown',
  isEnabled: true,
  device: undefined,
  recentResults: [],
  stats: undefined,
  gaugeMetric: null,
}

const monitor = computed<Monitor>(() => monitorsStore.currentMonitor || emptyMonitor)

const stats = computed(
  () =>
    monitor.value.stats || {
      avgLatency: null,
      minLatency: null,
      maxLatency: null,
      lastLatency: null,
      uptimePercentage: 100,
      totalChecks: 0,
      upChecks: 0,
    }
)

const showHistory = ref(false)
const history = useInfiniteList<MonitorResult>(() => `/monitors/${monitorId.value}/results`, {
  label: 'histórico de verificações',
})

const showAlerts = ref(false)
const alertHistory = useInfiniteList<AlertEvent>(() => `/monitors/${monitorId.value}/alerts`, {
  label: 'histórico de alertas',
})

const silenceDurations = [
  { minutes: 30, label: '30 minutos' },
  { minutes: 60, label: '1 hora' },
  { minutes: 240, label: '4 horas' },
  { minutes: 1440, label: '24 horas' },
]

function toggleShowHistory() {
  showHistory.value = !showHistory.value
  // Reabrir o card recomeça da primeira página: o histórico pode ter crescido
  // enquanto ele estava recolhido.
  if (showHistory.value) history.reset()
}

function toggleShowAlerts() {
  showAlerts.value = !showAlerts.value
  if (showAlerts.value) alertHistory.reset()
}

const formattedTarget = computed(() => {
  if (monitor.value.port) {
    return `${monitor.value.target}:${monitor.value.port}`
  }
  return monitor.value.target
})

const statusText = computed(() => (monitor.value.status || 'UNKNOWN').toUpperCase())

const isTrafficMonitor = computed(() => isTrafficMonitorFn(monitor.value))
const isGaugeMonitor = computed(() => isGaugeMonitorFn(monitor.value) && !isTrafficMonitor.value)
const isInterfaceMonitor = computed(() => isInterfaceMonitorFn(monitor.value))
const isMemoryGauge = computed(() => gaugeMetricName(monitor.value) === 'memory_usage')

const typeText = computed(() => {
  if (isTrafficMonitor.value) return 'TRÁFEGO'
  if (isGaugeMonitor.value) return gaugeTypeLabel(monitor.value)
  if (isInterfaceMonitor.value) return 'INTERFACE'
  return (monitor.value.type || 'PING').toUpperCase()
})

const gaugeColorValue = computed(() =>
  gaugeColorFn(gaugeUsagePercent(monitor.value), gaugeMetricName(monitor.value))
)

// Unifica a apresentação do status no header: tráfego mostra taxa, memória mostra
// quantidade, CPU mostra percentual,
// interface mostra o estado real e monitor tradicional mostra status textual.
const headerChip = computed(() => {
  if (isTrafficMonitor.value) {
    const info = interfaceStatusInfo(
      monitor.value.status,
      latestResultData(monitor.value.recentResults)
    )
    return {
      label: trafficInText.value !== 'N/D' ? `↓ ${trafficInText.value}` : info.label.toUpperCase(),
      color: info.color,
      icon: 'mdi-swap-vertical-bold',
    }
  }
  if (isGaugeMonitor.value) {
    return {
      label: formatGaugeValue(monitor.value, true).toUpperCase(),
      color: gaugeColorValue.value,
      icon: 'mdi-gauge',
    }
  }
  if (isInterfaceMonitor.value) {
    const info = interfaceStatusInfo(
      monitor.value.status,
      latestResultData(monitor.value.recentResults)
    )
    return { label: info.label.toUpperCase(), color: info.color, icon: info.icon }
  }
  return {
    label: statusText.value,
    color: getStatusColor(monitor.value.status),
    icon: 'mdi-circle',
  }
})

const typeIcon = computed(() => {
  if (isTrafficMonitor.value) return 'mdi-swap-vertical-bold'
  if (isGaugeMonitor.value) {
    return gaugeMetricName(monitor.value) === 'memory_usage' ? 'mdi-memory' : 'mdi-chip'
  }
  if (isInterfaceMonitor.value) return headerChip.value.icon
  switch (monitor.value.type) {
    case 'http':
    case 'https':
      return 'mdi-web'
    case 'tcp':
      return 'mdi-ethernet-cable'
    case 'dns':
      return 'mdi-dns'
    default:
      return 'mdi-ping'
  }
})

const lastLatencyText = computed(() => formatLatency(stats.value.lastLatency))
const avgLatencyText = computed(() => formatLatency(stats.value.avgLatency))
const minLatencyText = computed(() => formatLatency(stats.value.minLatency))
const maxLatencyText = computed(() => {
  return stats.value.maxLatency !== null ? `${stats.value.maxLatency}ms` : 'N/A'
})

const latencyKpiTitles = computed(() => {
  if (monitor.value?.type === 'ping') {
    return {
      current: 'Ping Atual',
      avg: 'Ping Médio',
      minMax: 'Ping Mín / Máx',
      currentCaption: 'Última resposta registrada',
      avgCaption: 'Média das verificações recentes',
      minMaxCaption: 'Mínima e máxima de latência',
    }
  }
  if (monitor.value?.type === 'dns') {
    return {
      current: 'Tempo de Consulta Atual',
      avg: 'Tempo de Consulta Médio',
      minMax: 'Consulta Mín / Máx',
      currentCaption: 'Última consulta DNS registrada',
      avgCaption: 'Média dos tempos de resolução',
      minMaxCaption: 'Mínimo e máximo tempo de consulta',
    }
  }
  return {
    current: 'Tempo de Resposta Atual',
    avg: 'Tempo de Resposta Médio',
    minMax: 'Resposta Mín / Máx',
    currentCaption: 'Última resposta registrada',
    avgCaption: 'Média das verificações recentes',
    minMaxCaption: 'Mínimo e máximo tempo de resposta',
  }
})

// --- Monitores de Interface / Tráfego ---
const interfaceLatestData = computed(() => latestResultData(monitor.value.recentResults))

const interfaceSpeedText = computed(() => {
  const data = interfaceLatestData.value
  return (data?.speedFormatted as string | undefined) || headerChip.value.label
})

const interfaceOperText = computed(() => {
  const data = interfaceLatestData.value
  return (data?.operStatusText as string | undefined) || headerChip.value.label
})

const interfaceFlapCount = computed(() => {
  const results = monitor.value.recentResults || []
  let flaps = 0
  for (let i = 1; i < results.length; i++) {
    if (results[i].status !== results[i - 1].status) flaps++
  }
  return flaps
})

const statusBreakdown = computed(() => {
  const results = monitor.value.recentResults || []
  const counts = { up: 0, down: 0, warning: 0, disabled: 0, unknown: 0 }
  for (const r of results) {
    const key = (r.status && r.status in counts ? r.status : 'unknown') as keyof typeof counts
    counts[key]++
  }
  return counts
})

// --- Histórico de Métricas (CPU/Memória/Tráfego de Interface) ---
const gaugeHistory = ref<DeviceMetric[]>([])

const trafficTab = ref<'inBps' | 'outBps' | 'combined'>('combined')

const trafficMetrics = computed(() => {
  const ifName = monitor.value.configuration?.ifName as string | undefined
  return gaugeHistory.value
    .filter((m) => {
      if (m.metricName !== 'inBps' && m.metricName !== 'outBps') return false
      if (ifName && m.interfaceName) {
        return m.interfaceName.toLowerCase() === ifName.toLowerCase()
      }
      return true
    })
    .slice()
    .sort((a, b) => new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime())
})

const latestInBps = computed(() => {
  const list = trafficMetrics.value.filter((m) => m.metricName === 'inBps')
  if (list.length === 0) return null
  return Number(list[list.length - 1].metricValue)
})

const latestOutBps = computed(() => {
  const list = trafficMetrics.value.filter((m) => m.metricName === 'outBps')
  if (list.length === 0) return null
  return Number(list[list.length - 1].metricValue)
})

const trafficInText = computed(() => {
  if (latestInBps.value !== null && Number.isFinite(latestInBps.value)) {
    return formatBps(latestInBps.value)
  }
  if (monitor.value.gaugeMetric?.value) {
    return formatBps(monitor.value.gaugeMetric.value)
  }
  return 'N/D'
})

const trafficOutText = computed(() => {
  if (latestOutBps.value !== null && Number.isFinite(latestOutBps.value)) {
    return formatBps(latestOutBps.value)
  }
  return 'N/D'
})

const trafficSeries = computed<ChartSeriesInput[]>(() => {
  const inList = trafficMetrics.value.filter((m) => m.metricName === 'inBps')
  const outList = trafficMetrics.value.filter((m) => m.metricName === 'outBps')

  if (trafficTab.value === 'combined') {
    const series: ChartSeriesInput[] = []
    if (inList.length > 0) {
      series.push({
        id: 'inBps',
        label: 'Download (IN)',
        color: '#4CAF50',
        fillArea: true,
        data: inList.map((m) => {
          const val = Number(m.metricValue) || 0
          return {
            time: formatDateTime(m.createdAt, '-'),
            value: val,
            formattedValue: formatBps(val),
          }
        }),
      })
    }
    if (outList.length > 0) {
      series.push({
        id: 'outBps',
        label: 'Upload (OUT)',
        color: '#2196F3',
        fillArea: false,
        data: outList.map((m) => {
          const val = Number(m.metricValue) || 0
          return {
            time: formatDateTime(m.createdAt, '-'),
            value: val,
            formattedValue: formatBps(val),
          }
        }),
      })
    }
    return series
  }

  const isDownload = trafficTab.value === 'inBps'
  const targetList = isDownload ? inList : outList
  if (targetList.length === 0) return []

  return [
    {
      id: trafficTab.value,
      label: isDownload ? 'Download (IN)' : 'Upload (OUT)',
      color: isDownload ? '#4CAF50' : '#2196F3',
      fillArea: true,
      data: targetList.map((m) => {
        const val = Number(m.metricValue) || 0
        return {
          time: formatDateTime(m.createdAt, '-'),
          value: val,
          formattedValue: formatBps(val),
        }
      }),
    },
  ]
})

// --- Métricas da Interface de Link (quando configurada no dispositivo) ---
const linkTrafficTab = ref<'inBps' | 'outBps' | 'combined'>('combined')

const linkTrafficMetrics = computed(() => {
  const linkId = deviceLinkInterfaceId.value
  const linkName = deviceLinkInterfaceName.value?.toLowerCase()
  if (!linkId && !linkName) return []
  return gaugeHistory.value
    .filter((m) => {
      if (m.metricName !== 'inBps' && m.metricName !== 'outBps') return false
      if (linkId && m.interfaceId === linkId) return true
      if (linkName && m.interfaceName && m.interfaceName.toLowerCase() === linkName) return true
      return false
    })
    .slice()
    .sort((a, b) => new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime())
})

const latestLinkInBps = computed(() => {
  const list = linkTrafficMetrics.value.filter((m) => m.metricName === 'inBps')
  if (list.length === 0) return null
  return Number(list[list.length - 1].metricValue)
})

const latestLinkOutBps = computed(() => {
  const list = linkTrafficMetrics.value.filter((m) => m.metricName === 'outBps')
  if (list.length === 0) return null
  return Number(list[list.length - 1].metricValue)
})

const linkTrafficSeries = computed<ChartSeriesInput[]>(() => {
  const inList = linkTrafficMetrics.value.filter((m) => m.metricName === 'inBps')
  const outList = linkTrafficMetrics.value.filter((m) => m.metricName === 'outBps')

  if (linkTrafficTab.value === 'combined') {
    const series: ChartSeriesInput[] = []
    if (inList.length > 0) {
      series.push({
        id: 'linkInBps',
        label: 'Download (IN)',
        color: '#4CAF50',
        fillArea: true,
        data: inList.map((m) => {
          const val = Number(m.metricValue) || 0
          return {
            time: formatDateTime(m.createdAt, '-'),
            value: val,
            formattedValue: formatBps(val),
          }
        }),
      })
    }
    if (outList.length > 0) {
      series.push({
        id: 'linkOutBps',
        label: 'Upload (OUT)',
        color: '#2196F3',
        fillArea: false,
        data: outList.map((m) => {
          const val = Number(m.metricValue) || 0
          return {
            time: formatDateTime(m.createdAt, '-'),
            value: val,
            formattedValue: formatBps(val),
          }
        }),
      })
    }
    return series
  }

  const isDownload = linkTrafficTab.value === 'inBps'
  const targetList = isDownload ? inList : outList
  if (targetList.length === 0) return []

  return [
    {
      id: linkTrafficTab.value,
      label: isDownload ? 'Download (IN)' : 'Upload (OUT)',
      color: isDownload ? '#4CAF50' : '#2196F3',
      fillArea: true,
      data: targetList.map((m) => {
        const val = Number(m.metricValue) || 0
        return {
          time: formatDateTime(m.createdAt, '-'),
          value: val,
          formattedValue: formatBps(val),
        }
      }),
    },
  ]
})

const gaugeHistoryFiltered = computed(() => {
  const name = isMemoryGauge.value ? 'memory_used_bytes' : gaugeMetricName(monitor.value)
  return gaugeHistory.value
    .filter((m) => m.metricName === name)
    .slice()
    .sort((a, b) => new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime())
})

const gaugeStats = computed(() => {
  const values = gaugeHistoryFiltered.value
    .map((m) => Number(m.metricValue))
    .filter((v) => !isNaN(v))
  const current =
    monitor.value.gaugeMetric?.value ?? (values.length > 0 ? values[values.length - 1] : null)
  if (values.length === 0) {
    return {
      current,
      avg: null as number | null,
      min: null as number | null,
      max: null as number | null,
    }
  }
  const avg = Number((values.reduce((a, b) => a + b, 0) / values.length).toFixed(1))
  return { current, avg, min: Math.min(...values), max: Math.max(...values) }
})

function formatGaugeStat(value: number | null | undefined): string {
  if (value === null || value === undefined) return 'N/A'
  return isMemoryGauge.value ? formatBinaryBytes(value) : `${Number(value).toFixed(1)}%`
}

const gaugeCurrentText = computed(() => formatGaugeStat(gaugeStats.value.current))
const gaugeAvgText = computed(() => formatGaugeStat(gaugeStats.value.avg))
const gaugeMinText = computed(() => formatGaugeStat(gaugeStats.value.min))
const gaugeMaxText = computed(() => formatGaugeStat(gaugeStats.value.max))
const gaugeUnitType = computed<'bytes' | 'percentage'>(() =>
  isMemoryGauge.value ? 'bytes' : 'percentage'
)

const gaugeSeries = computed<ChartSeriesInput[]>(() => {
  const list = gaugeHistoryFiltered.value
  if (list.length === 0) return []
  const name = gaugeMetricName(monitor.value)
  return [
    {
      id: name,
      label: name === 'memory_usage' ? 'Uso de Memória' : 'Uso de CPU',
      color: name === 'memory_usage' ? '#9C27B0' : '#2196F3',
      fillArea: true,
      data: list.map((m) => {
        const val = Number(m.metricValue) || 0
        return {
          time: formatDateTime(m.createdAt, '-'),
          value: val,
          formattedValue: formatGaugeStat(val),
        }
      }),
    },
  ]
})

const showRuleCatalog = ref(false)
const baselineData = ref<MonitorBaselinePayload | null>(null)

async function loadBaselineData() {
  if (!monitorId.value) return
  try {
    baselineData.value = await apiService.get<MonitorBaselinePayload>(
      `/monitors/${monitorId.value}/baseline`
    )
  } catch {
    baselineData.value = null
  }
}

async function loadAssociatedDevice() {
  if (!monitor.value.deviceId) {
    associatedDevice.value = null
    return
  }
  const cached = devicesStore.devices.find((d) => d.id === monitor.value.deviceId)
  if (cached) {
    associatedDevice.value = cached
  }
  try {
    const dev = await apiService.get<Device>(`/devices/${monitor.value.deviceId}`)
    if (dev) {
      associatedDevice.value = dev
    }
  } catch {
    // Silently continue
  }
}

async function loadGaugeHistory() {
  if (!monitor.value.deviceId) {
    gaugeHistory.value = []
    return
  }
  try {
    gaugeHistory.value = await apiService.get<DeviceMetric[]>(
      `/devices/${monitor.value.deviceId}/metrics`
    )
  } catch {
    gaugeHistory.value = []
  }
}

/**
 * O histórico de uso e tráfego é estado local desta tela, então ela mesma
 * assina o evento de coleta SNMP para acompanhar as novas amostras ao vivo.
 * Os demais dados (timeline, latência, KPIs) vêm patchados pela store.
 */
let stopMetricsListener: (() => void) | null = null

onMounted(async () => {
  if (!monitorId.value) return

  await monitorsStore.fetchMonitorById(monitorId.value)
  await loadAssociatedDevice()
  if (
    isGaugeMonitor.value ||
    isTrafficMonitor.value ||
    deviceLinkInterfaceId.value ||
    deviceLinkInterfaceName.value
  ) {
    await loadGaugeHistory()
  }
  if (!isGaugeMonitor.value && !isInterfaceMonitor.value && !isTrafficMonitor.value) {
    await loadBaselineData()
  }

  stopMetricsListener = eventsStore.onEvent('metric:recorded', (data) => {
    const hasLink = Boolean(deviceLinkInterfaceId.value || deviceLinkInterfaceName.value)
    if (!isGaugeMonitor.value && !isTrafficMonitor.value && !hasLink) return
    if (Number(data.deviceId) !== monitor.value.deviceId) return

    const samples = (data.metrics as Array<Record<string, unknown>>) || []

    for (const sample of samples) {
      gaugeHistory.value.push({
        id: Date.now() + gaugeHistory.value.length,
        deviceId: monitor.value.deviceId,
        interfaceId: sample.interfaceId as number | undefined,
        interfaceName: sample.interfaceName as string | undefined,
        metricName: String(sample.name),
        metricValue: Number(sample.value),
        unit: String(sample.unit ?? ''),
        createdAt: String(sample.recordedAt ?? new Date().toISOString()),
      })
    }
  })

  eventsStore.onEvent('monitor:result', (data) => {
    if (Number(data.monitorId) === monitorId.value) {
      loadBaselineData()
    }
  })

  eventsStore.onEvent(
    ['alert:triggered', 'alert:resolved', 'alert:acknowledged', 'alert:silenced'],
    (data) => {
      if (Number(data.monitorId) !== monitorId.value) return
      if (showAlerts.value) alertHistory.reset()
    }
  )
})

onUnmounted(() => {
  stopMetricsListener?.()
})

async function refreshData() {
  if (monitorId.value) {
    await monitorsStore.fetchMonitorById(monitorId.value)
    await loadAssociatedDevice()
    if (isGaugeMonitor.value || isTrafficMonitor.value || deviceLinkInterfaceName.value) {
      await loadGaugeHistory()
    }
    if (!isGaugeMonitor.value && !isInterfaceMonitor.value && !isTrafficMonitor.value) {
      await loadBaselineData()
    }
    if (showHistory.value) history.reset()
    if (showAlerts.value) alertHistory.reset()
  }
}

async function acknowledgeAlertItem(item: AlertEvent) {
  const success = await alertsStore.acknowledgeAlert(item.id)
  if (success && showAlerts.value) {
    alertHistory.reset()
  }
}

async function silenceAlertItem(item: AlertEvent, minutes: number) {
  const success = await alertsStore.silenceAlert(item.id, minutes)
  if (success && showAlerts.value) {
    alertHistory.reset()
  }
}

async function confirmDelete() {
  const ok = await confirm({
    title: 'Excluir monitor',
    message: 'Tem certeza de que deseja excluir este monitor e suas métricas históricas?',
    confirmText: 'Excluir',
    confirmColor: 'error',
    icon: 'mdi-delete-alert-outline',
  })
  if (!ok) return

  const success = await monitorsStore.deleteMonitor(monitorId.value)
  if (!success) return
  // Excluído: quem abriu decide o que fazer com a tela. Na rota, volta para
  // a lista; no diálogo, fecha e deixa o chamador recarregar.
  if (props.embedded) emit('closed')
  else router.push('/monitors')
}

// Estrutura unificada de dados para o componente BaseMetricChart
const latencySeries = computed<ChartSeriesInput[]>(() => {
  // `recentResults` já vem do mais antigo para o mais recente (ver stores/monitors.ts) —
  // não reverter aqui, ou o gráfico plota o tempo andando para trás.
  const results = monitor.value.recentResults || []
  if (results.length === 0) return []

  return [
    {
      id: 'latency',
      label: 'Tempo de Resposta',
      color: '#2196F3',
      fillArea: true,
      data: results.map((r) => {
        const val = r.latencyMs || 0
        const status = r.status || (val > 0 ? 'up' : 'down')
        return {
          time: formatDateTime(r.finishedAt, '-'),
          value: val,
          formattedValue: `${val} ms`,
          status,
          color: status === 'down' ? '#F44336' : '#2196F3',
        }
      }),
    },
  ]
})
</script>
