<template>
  <DashboardCard
    :title="widget.title || 'Consumo de Banda de Ether'"
    icon="mdi-swap-horizontal-bold"
    :subtitle="targetDescription"
  >
    <template #actions>
      <v-select
        v-model="selectedDeviceId"
        :items="deviceOptions"
        item-title="name"
        item-value="id"
        density="compact"
        variant="outlined"
        hide-details
        aria-label="Equipamento"
        class="ether-widget__select"
        placeholder="Equipamento"
        @update:model-value="onDeviceChange"
      ></v-select>

      <v-select
        v-model="selectedInterfaceId"
        :items="interfaceOptions"
        item-title="name"
        item-value="id"
        density="compact"
        variant="outlined"
        hide-details
        aria-label="Interface"
        class="ether-widget__select"
        placeholder="Interface"
      ></v-select>

      <TimeframeToggle v-model="timeframe" />
    </template>

    <BaseMetricChart
      :series="chartSeries"
      unit-type="bandwidth"
      :height="220"
      empty-text="Sem dados de tráfego para a interface. Selecione um equipamento com SNMP ativo e interface monitorada."
      aria-label="Tráfego de entrada e saída da interface"
    />

    <template #footer>
      <div class="d-flex align-center justify-space-between flex-wrap ga-2 text-caption">
        <span>
          Pico Rx <strong>{{ formatBps(peakRx) }}</strong> · Pico Tx
          <strong>{{ formatBps(peakTx) }}</strong>
        </span>
        <span>
          Total acumulado <strong>{{ totalVolumeFormatted }}</strong>
        </span>
      </div>
    </template>
  </DashboardCard>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { useDevicesStore } from '@/stores/devices'
import { useDeviceDetailStore } from '@/stores/deviceDetail'
import { useEventsStore } from '@/stores/events'
import type { WidgetConfig } from '@/stores/dashboard'
import { formatBps, formatBytes } from '@/utils/formatters'
import { paletteColor } from '@/utils/charts/scale'
import BaseMetricChart, { type ChartSeriesInput } from '@/components/BaseMetricChart.vue'
import DashboardCard from '@/components/dashboard/DashboardCard.vue'
import TimeframeToggle from '@/components/dashboard/TimeframeToggle.vue'
import { useWidgetTimeframe } from '@/composables/useWidgetTimeframe'

const props = defineProps<{
  widget: WidgetConfig
}>()

const devicesStore = useDevicesStore()
const deviceDetailStore = useDeviceDetailStore()
const eventsStore = useEventsStore()

const timeframe = useWidgetTimeframe(() => props.widget, '15m')
const selectedDeviceId = ref<number | 'all'>((props.widget.config?.deviceId as any) || 'all')
const selectedInterfaceId = ref<number | 'all'>((props.widget.config?.interfaceId as any) || 'all')

interface TrafficSample {
  time: string
  inBps: number
  outBps: number
  timestamp: number
}

const localSamples = ref<TrafficSample[]>([])

const deviceOptions = computed(() => {
  const options: Array<{ id: number | 'all'; name: string }> = [
    { id: 'all', name: 'Todos os Equipamentos' },
  ]
  for (const dev of devicesStore.devices) {
    options.push({ id: dev.id, name: dev.name || dev.ipAddress || `Device #${dev.id}` })
  }
  return options
})

const interfaceOptions = computed(() => {
  const options: Array<{ id: number | 'all'; name: string }> = [
    { id: 'all', name: 'Todas as Interfaces (Média)' },
  ]
  if (selectedDeviceId.value !== 'all') {
    for (const iface of deviceDetailStore.interfaces) {
      options.push({
        id: iface.id,
        name: iface.name || iface.ifName || `if-${iface.snmpIndex || iface.id}`,
      })
    }
  }
  return options
})

const targetDescription = computed(() => {
  const dev = devicesStore.devices.find((d) => d.id === selectedDeviceId.value)
  const devName = dev ? dev.name : 'Média Global'

  if (selectedInterfaceId.value !== 'all') {
    const iface = deviceDetailStore.interfaces.find((i) => i.id === selectedInterfaceId.value)
    const ifName = iface ? iface.name || iface.ifName : `Interface #${selectedInterfaceId.value}`
    return `${devName} — ${ifName}`
  }
  return `${devName} — Interfaces de Tráfego`
})

let unbindEvent: (() => void) | null = null

onUnmounted(() => {
  unbindEvent?.()
  unbindEvent = null
})

onMounted(async () => {
  if (devicesStore.devices.length === 0) {
    await devicesStore.fetchDevices()
  }
  if (selectedDeviceId.value !== 'all' && typeof selectedDeviceId.value === 'number') {
    await deviceDetailStore.loadDeviceDetails(selectedDeviceId.value)
  }
  buildSamples()

  unbindEvent = eventsStore.onEvent('metric:recorded', (data: any) => {
    if (data && data.metrics && Array.isArray(data.metrics)) {
      const devId = Number(data.deviceId)
      if (selectedDeviceId.value === 'all' || selectedDeviceId.value === devId) {
        let inBps = 0
        let outBps = 0
        let matched = false

        for (const m of data.metrics) {
          if (selectedInterfaceId.value === 'all' || m.interfaceId === selectedInterfaceId.value) {
            if (m.name === 'inBps') {
              inBps += Number(m.value) || 0
              matched = true
            }
            if (m.name === 'outBps') {
              outBps += Number(m.value) || 0
              matched = true
            }
          }
        }

        if (matched) {
          const now = new Date()
          localSamples.value.push({
            time: now.toLocaleTimeString([], {
              hour: '2-digit',
              minute: '2-digit',
              second: '2-digit',
            }),
            inBps,
            outBps,
            timestamp: now.getTime(),
          })
          if (localSamples.value.length > 50) {
            localSamples.value.shift()
          }
        }
      }
    }
  })
})

async function onDeviceChange(val: number | 'all') {
  selectedInterfaceId.value = 'all'
  if (typeof val === 'number') {
    await deviceDetailStore.loadDeviceDetails(val)
  }
  buildSamples()
}

watch([selectedInterfaceId, timeframe], () => {
  buildSamples()
})

function buildSamples() {
  const list: TrafficSample[] = []
  const metricsSource = deviceDetailStore.metrics

  if (metricsSource && metricsSource.length > 0) {
    const sorted = [...metricsSource].sort(
      (a, b) => new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime()
    )

    const map = new Map<string, { inBps: number; outBps: number; ts: number }>()

    for (const m of sorted) {
      if (selectedInterfaceId.value !== 'all' && m.interfaceId !== selectedInterfaceId.value) {
        continue
      }

      if (m.metricName === 'inBps' || m.metricName === 'outBps') {
        const d = new Date(m.createdAt)
        const key = d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
        if (!map.has(key)) {
          map.set(key, { inBps: 0, outBps: 0, ts: d.getTime() })
        }
        const entry = map.get(key)!
        if (m.metricName === 'inBps')
          entry.inBps = Math.max(entry.inBps, Number(m.metricValue) || 0)
        if (m.metricName === 'outBps')
          entry.outBps = Math.max(entry.outBps, Number(m.metricValue) || 0)
      }
    }

    map.forEach((val, key) => {
      list.push({
        time: key,
        inBps: val.inBps,
        outBps: val.outBps,
        timestamp: val.ts,
      })
    })
  }

  localSamples.value = list.slice(-30)
}

const samples = computed(() => localSamples.value)

const peakRx = computed(() =>
  samples.value.length ? Math.max(...samples.value.map((s) => s.inBps)) : 0
)
const peakTx = computed(() =>
  samples.value.length ? Math.max(...samples.value.map((s) => s.outBps)) : 0
)

const totalVolumeFormatted = computed(() => {
  if (samples.value.length === 0) return '0 B'
  const totalBits = samples.value.reduce((acc, s) => acc + (s.inBps + s.outBps) * 60, 0)
  const totalBytes = totalBits / 8
  return formatBytes(totalBytes)
})

const chartSeries = computed<ChartSeriesInput[]>(() => [
  {
    id: 'rx',
    label: 'Download (Rx)',
    color: paletteColor(0),
    data: samples.value.map((s) => ({
      time: new Date(s.timestamp).toISOString(),
      value: s.inBps,
    })),
  },
  {
    id: 'tx',
    label: 'Upload (Tx)',
    color: paletteColor(1),
    data: samples.value.map((s) => ({
      time: new Date(s.timestamp).toISOString(),
      value: s.outBps,
    })),
  },
])
</script>

<style scoped>
.ether-widget__select {
  min-width: 160px;
  max-width: 200px;
}
</style>
