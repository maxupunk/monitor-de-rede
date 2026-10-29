<template>
  <DashboardCard
    :title="widget.title || 'Consumo de Banda vs Latência'"
    icon="mdi-chart-multiaxis"
    icon-color="deep-purple"
    :subtitle="`Tráfego × latência de ping · Alvo: ${selectedPingTargetLabel}`"
    :loading="loading"
  >
    <template #actions>
      <v-btn
        v-if="!hasDnsPingMonitors && dnsServersStore.servers.length > 0"
        size="small"
        variant="tonal"
        color="deep-purple"
        prepend-icon="mdi-dns"
        :loading="provisioningDnsPing"
        @click="provisionDnsPing"
      >
        Adicionar Ping para DNS
        <v-tooltip activator="parent" location="top">
          Provisiona monitores de Ping ICMP para os servidores DNS compararem com a banda
        </v-tooltip>
      </v-btn>

      <v-combobox
        v-model="selectedPingTarget"
        :items="pingTargetOptions"
        item-title="title"
        item-value="value"
        :return-object="false"
        density="compact"
        variant="outlined"
        hide-details
        aria-label="Alvo de ping"
        class="bvl-widget__target"
        placeholder="Alvo de Ping / IP / DNS"
      >
        <template #item="{ props: itemProps, item }">
          <v-list-item
            v-bind="itemProps"
            :title="itemField(item, 'title')"
            :subtitle="itemField(item, 'subtitle')"
            :prepend-icon="itemField(item, 'icon')"
          ></v-list-item>
        </template>
      </v-combobox>

      <v-select
        v-model="selectedDeviceId"
        :items="deviceOptions"
        item-title="name"
        item-value="id"
        density="compact"
        variant="outlined"
        hide-details
        aria-label="Equipamento"
        class="bvl-widget__device"
        placeholder="Equipamento"
      ></v-select>

      <TimeframeToggle v-model="timeframe" />
    </template>

    <v-alert
      v-if="hasSaturationCorrelation"
      type="warning"
      variant="tonal"
      density="compact"
      class="mb-3 rounded-lg"
      prepend-icon="mdi-alert-decagram"
    >
      <span class="font-weight-bold">Alerta de Saturação de Banda:</span>
      Picos de latência estão se correlacionando com alto tráfego na interface.
    </v-alert>

    <BaseMetricChart
      :series="chartSeries"
      unit-type="bandwidth"
      right-unit-type="latency"
      :height="240"
      empty-text="Sem dados correlacionados no período."
      aria-label="Tráfego de banda comparado à latência de ping"
    />

    <template #footer>
      <div class="d-flex align-center justify-space-between flex-wrap ga-2 text-caption">
        <span>
          Banda <strong>{{ formatBps(currentBw) }}</strong> (pico {{ formatBps(peakBw) }}) ·
          Latência <strong>{{ formatLatency(currentLatency) }}</strong> (média
          {{ formatLatency(avgLatency) }})
        </span>
        <span>
          Índice de correlação <strong>{{ correlationScore }}%</strong>
        </span>
      </div>
    </template>
  </DashboardCard>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import { useMonitorsStore } from '@/stores/monitors'
import {
  useDevicesStore,
  type BandwidthLatencyPoint,
  type BandwidthLatencyResponse,
} from '@/stores/devices'
import { useDnsServersStore } from '@/stores/dnsServers'
import { useDnsPerformanceStore } from '@/stores/dnsPerformance'
import { useEventsStore } from '@/stores/events'
import type { WidgetConfig } from '@/stores/dashboard'
import { formatLatency, formatBps } from '@/utils/formatters'
import { paletteColor } from '@/utils/charts/scale'
import BaseMetricChart, { type ChartSeriesInput } from '@/components/BaseMetricChart.vue'
import DashboardCard from '@/components/dashboard/DashboardCard.vue'
import TimeframeToggle from '@/components/dashboard/TimeframeToggle.vue'
import { useWidgetTimeframe } from '@/composables/useWidgetTimeframe'

const props = defineProps<{
  widget: WidgetConfig
}>()

const monitorsStore = useMonitorsStore()
const devicesStore = useDevicesStore()
const dnsServersStore = useDnsServersStore()
const dnsPerformanceStore = useDnsPerformanceStore()
const eventsStore = useEventsStore()

const timeframe = useWidgetTimeframe(() => props.widget, '15m')
const initialPingTarget =
  props.widget.config?.targetHost || (props.widget.config?.monitorId as any) || 'all'
const selectedPingTarget = ref<number | 'all' | string>(initialPingTarget)
const selectedDeviceId = ref<number | 'all'>((props.widget.config?.deviceId as any) || 'all')

const loading = ref(false)
const provisioningDnsPing = ref(false)
const serverResponse = ref<BandwidthLatencyResponse | null>(null)
const localSamples = ref<BandwidthLatencyPoint[]>([])

const hasDnsPingMonitors = computed(() =>
  monitorsStore.monitors.some(
    (m) =>
      m.type === 'ping' &&
      (m.name.toLowerCase().includes('dns') ||
        dnsServersStore.servers.some((s) => s.address === m.target))
  )
)

async function provisionDnsPing() {
  provisioningDnsPing.value = true
  try {
    const res = await dnsPerformanceStore.provisionPingMonitors()
    if (res) {
      await monitorsStore.fetchMonitors()
      await loadData()
    }
  } finally {
    provisioningDnsPing.value = false
  }
}

let unbindMetricEvent: (() => void) | null = null
let unbindMonitorEvent: (() => void) | null = null

async function loadData() {
  loading.value = true
  try {
    const res = await devicesStore.fetchBandwidthLatencySeries({
      deviceId: selectedDeviceId.value,
      pingTarget: selectedPingTarget.value,
      timeframe: timeframe.value,
    })
    serverResponse.value = res
    if (res && Array.isArray(res.samples)) {
      localSamples.value = res.samples
    } else {
      localSamples.value = []
    }
  } catch {
    localSamples.value = []
  } finally {
    loading.value = false
  }
}

watch([selectedDeviceId, selectedPingTarget, timeframe], () => {
  loadData()
})

onMounted(async () => {
  if (monitorsStore.monitors.length === 0) await monitorsStore.fetchMonitors()
  if (devicesStore.devices.length === 0) await devicesStore.fetchDevices()
  await dnsServersStore.fetchServers()
  await loadData()

  // Escuta métricas SNMP de tráfego em tempo real
  unbindMetricEvent = eventsStore.onEvent('metric:recorded', (data) => {
    if (!data || !Array.isArray(data.metrics)) return
    const devId = Number(data.deviceId)
    if (selectedDeviceId.value !== 'all' && selectedDeviceId.value !== devId) return

    let totalBps = 0
    let matched = false
    for (const m of data.metrics) {
      if (m.name === 'inBps' || m.name === 'outBps' || m.name === 'traffic') {
        totalBps += Number(m.value) || 0
        matched = true
      }
    }

    if (matched && localSamples.value.length > 0) {
      const lastPoint = localSamples.value[localSamples.value.length - 1]
      lastPoint.bwBps = totalBps
    }
  })

  // Escuta resultados de ping/dns em tempo real
  unbindMonitorEvent = eventsStore.onEvent('monitor:result', (data) => {
    const monId = Number(data.monitorId ?? data.id)
    if (!monId) return

    const mon = monitorsStore.monitors.find((m) => m.id === monId)
    if (!mon || (mon.type !== 'ping' && mon.type !== 'dns')) return

    const dnsServer = (mon.configuration as any)?.dnsServer || (mon.configuration as any)?.dohUrl
    const isMatch =
      selectedPingTarget.value === 'all' ||
      selectedPingTarget.value === monId ||
      String(selectedPingTarget.value) === mon.target ||
      (dnsServer && String(selectedPingTarget.value) === String(dnsServer))

    if (!isMatch) return

    const lat = typeof data.latencyMs === 'number' ? data.latencyMs : null
    if (lat !== null && localSamples.value.length > 0) {
      const lastPoint = localSamples.value[localSamples.value.length - 1]
      lastPoint.latency = lat
    }
  })
})

onUnmounted(() => {
  if (unbindMetricEvent) unbindMetricEvent()
  if (unbindMonitorEvent) unbindMonitorEvent()
})

export interface PingTargetOption {
  value: number | 'all' | string
  title: string
  subtitle?: string
  icon?: string
}

const pingTargetOptions = computed<PingTargetOption[]>(() => {
  const options: PingTargetOption[] = [
    {
      value: 'all',
      title: 'Todos os Monitores Ping (Média Latência)',
      subtitle: 'Calcula a média de todos os alvos de ping',
      icon: 'mdi-chart-timeline-variant',
    },
  ]

  // 1. Monitores de Ping cadastrados (ativos)
  for (const m of monitorsStore.monitors.filter((m) => m.type === 'ping')) {
    const isDns =
      m.name.toLowerCase().includes('dns') ||
      dnsServersStore.servers.some((s) => s.address === m.target)
    const device = m.deviceId ? devicesStore.devices.find((d) => d.id === m.deviceId) : null

    let icon = 'mdi-pulse'
    let subtitle = `Monitor de Ping #${m.id} · Alvo: ${m.target}`

    if (isDns) {
      icon = 'mdi-dns'
      subtitle = `Ping para Servidor DNS · Alvo: ${m.target}`
    } else if (device) {
      icon = 'mdi-devices'
      subtitle = `Ping de ${device.name || 'Dispositivo #' + device.id} · Alvo: ${m.target}`
    }

    options.push({
      value: m.id,
      title: `${m.name} (${m.target})`,
      subtitle,
      icon,
    })
  }

  // 2. Monitores DNS cadastrados (Resolução)
  for (const m of monitorsStore.monitors.filter((m) => m.type === 'dns')) {
    const cfg = (m.configuration || {}) as Record<string, unknown>
    const dnsServer = (cfg.dnsServer || cfg.dohUrl || m.target) as string
    options.push({
      value: m.id,
      title: `${m.name} (${dnsServer})`,
      subtitle: `Resolução DNS · Alvo: ${m.target} · Monitor #${m.id}`,
      icon: 'mdi-dns-outline',
    })
  }

  // 3. Se houver alvo personalizado informado
  if (
    typeof selectedPingTarget.value === 'string' &&
    selectedPingTarget.value !== 'all' &&
    !options.some((o) => String(o.value) === String(selectedPingTarget.value))
  ) {
    options.push({
      value: selectedPingTarget.value,
      title: `Alvo Customizado: ${selectedPingTarget.value}`,
      subtitle: 'IP fixo ou hostname DNS informado',
      icon: 'mdi-target',
    })
  }

  return options
})

const selectedPingTargetLabel = computed(() => {
  if (selectedPingTarget.value === 'all') return 'Todos os Monitores Ping'
  const matched = pingTargetOptions.value.find(
    (opt) => String(opt.value) === String(selectedPingTarget.value)
  )
  return matched ? matched.title : String(selectedPingTarget.value)
})

const deviceOptions = computed(() => {
  const options: Array<{ id: number | 'all'; name: string }> = [
    { id: 'all', name: 'Todos os Equipamentos (Média Banda)' },
  ]
  for (const dev of devicesStore.devices) {
    options.push({ id: dev.id, name: dev.name || dev.ipAddress || `Device #${dev.id}` })
  }
  return options
})

const samples = computed(() => localSamples.value)

const currentBw = computed(() => {
  if (serverResponse.value && serverResponse.value.currentBw > 0) {
    return serverResponse.value.currentBw
  }
  return samples.value.length ? samples.value[samples.value.length - 1].bwBps : 0
})

const peakBw = computed(() => {
  if (serverResponse.value && serverResponse.value.peakBw > 0) {
    return serverResponse.value.peakBw
  }
  return samples.value.length ? Math.max(...samples.value.map((s) => s.bwBps)) : 0
})

const currentLatency = computed(() => {
  if (serverResponse.value && serverResponse.value.currentLatency > 0) {
    return serverResponse.value.currentLatency
  }
  return samples.value.length ? samples.value[samples.value.length - 1].latency : 0
})

const avgLatency = computed(() => {
  if (serverResponse.value && serverResponse.value.avgLatency > 0) {
    return serverResponse.value.avgLatency
  }
  const valid = samples.value.filter((s) => s.latency > 0)
  if (valid.length === 0) return 0
  return valid.reduce((a, b) => a + b.latency, 0) / valid.length
})

const hasSaturationCorrelation = computed(() => {
  if (serverResponse.value) {
    return serverResponse.value.hasSaturationCorrelation
  }
  if (samples.value.length < 5) return false
  const peakBwVal = peakBw.value
  const avgLatVal = avgLatency.value

  const correlatedPoints = samples.value.filter(
    (s) => s.bwBps > peakBwVal * 0.75 && s.latency > avgLatVal * 1.5
  )
  return correlatedPoints.length >= 2
})

const correlationScore = computed(() => {
  if (serverResponse.value) {
    return serverResponse.value.correlationScore
  }
  if (samples.value.length === 0) return 0
  return hasSaturationCorrelation.value ? 82 : 15
})

/** Banda no eixo esquerdo, latência no direito: cada um na sua unidade. */
const chartSeries = computed<ChartSeriesInput[]>(() => [
  {
    id: 'bandwidth',
    label: 'Tráfego de banda',
    color: paletteColor(0),
    data: samples.value.map((s) => ({
      time: new Date(s.timestamp).toISOString(),
      value: s.bwBps,
    })),
  },
  {
    id: 'latency',
    label: 'Latência de ping',
    color: paletteColor(1),
    axis: 'right',
    data: samples.value.map((s) => ({
      time: new Date(s.timestamp).toISOString(),
      value: s.latency,
    })),
  },
])

function itemField(item: unknown, field: string): string | undefined {
  if (!item || typeof item !== 'object') return undefined
  const raw = (item as { raw?: Record<string, unknown> }).raw
  if (raw && typeof raw === 'object' && field in raw) {
    const val = raw[field]
    return typeof val === 'string' ? val : undefined
  }
  if (field in item) {
    const val = (item as Record<string, unknown>)[field]
    return typeof val === 'string' ? val : undefined
  }
  return undefined
}
</script>

<style scoped>
.bvl-widget__target {
  min-width: 200px;
  max-width: 260px;
}

.bvl-widget__device {
  min-width: 170px;
  max-width: 200px;
}
</style>
