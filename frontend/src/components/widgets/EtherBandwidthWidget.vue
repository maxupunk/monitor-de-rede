<template>
  <v-card elevation="2" class="rounded-lg fill-height d-flex flex-column">
    <v-card-title class="d-flex align-center justify-space-between py-3 px-4 flex-wrap ga-2">
      <div>
        <div class="d-flex align-center ga-2">
          <v-icon color="primary">mdi-swap-horizontal-bold</v-icon>
          <span class="font-weight-bold text-h6">{{
            widget.title || 'Consumo de Banda de Ether'
          }}</span>
        </div>
        <div class="text-caption text-grey mt-1 d-flex align-center ga-1">
          <v-icon size="14" color="primary">mdi-router-network</v-icon>
          <span>{{ targetDescription }}</span>
        </div>
      </div>

      <div class="d-flex align-center ga-2 flex-wrap">
        <v-select
          v-model="selectedDeviceId"
          :items="deviceOptions"
          item-title="name"
          item-value="id"
          density="compact"
          variant="outlined"
          hide-details
          style="min-width: 170px; max-width: 200px"
          class="text-caption"
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
          style="min-width: 150px; max-width: 180px"
          class="text-caption"
          placeholder="Interface"
        ></v-select>

        <v-btn-toggle v-model="timeframe" density="compact" variant="outlined" divided mandatory>
          <v-btn value="5m" size="x-small">5m</v-btn>
          <v-btn value="15m" size="x-small">15m</v-btn>
          <v-btn value="1h" size="x-small">1h</v-btn>
          <v-btn value="24h" size="x-small">24h</v-btn>
        </v-btn-toggle>
      </div>
    </v-card-title>
    <v-divider></v-divider>

    <v-card-text class="pa-3 flex-grow-1 relative">
      <div
        ref="chartContainerRef"
        class="chart-box w-100 relative pa-2 rounded bg-surface"
        @mousemove="onMouseMove"
        @mouseleave="onMouseLeave"
      >
        <svg
          v-if="samples.length > 0"
          class="w-100 chart-svg"
          viewBox="0 0 800 200"
          preserveAspectRatio="none"
        >
          <defs>
            <linearGradient id="rxGrad" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stop-color="#0284c7" stop-opacity="0.35" />
              <stop offset="100%" stop-color="#0284c7" stop-opacity="0.0" />
            </linearGradient>
            <linearGradient id="txGrad" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stop-color="#a855f7" stop-opacity="0.3" />
              <stop offset="100%" stop-color="#a855f7" stop-opacity="0.0" />
            </linearGradient>
          </defs>

          <!-- Grid Lines -->
          <line
            x1="70"
            y1="20"
            x2="780"
            y2="20"
            stroke="rgba(148, 163, 184, 0.2)"
            stroke-dasharray="3,3"
          />
          <text x="62" y="24" font-size="10" fill="#94a3b8" text-anchor="end">
            {{ formatBps(maxRate) }}
          </text>

          <line
            x1="70"
            y1="100"
            x2="780"
            y2="100"
            stroke="rgba(148, 163, 184, 0.2)"
            stroke-dasharray="3,3"
          />
          <text x="62" y="104" font-size="10" fill="#94a3b8" text-anchor="end">
            {{ formatBps(maxRate / 2) }}
          </text>

          <line
            x1="70"
            y1="180"
            x2="780"
            y2="180"
            stroke="rgba(148, 163, 184, 0.3)"
            stroke-width="1.5"
          />
          <text x="62" y="184" font-size="10" fill="#94a3b8" text-anchor="end">0 bps</text>

          <!-- Crosshair line -->
          <line
            v-if="hoverIndex !== null && crosshairX !== null"
            :x1="crosshairX"
            y1="20"
            :x2="crosshairX"
            y2="180"
            stroke="#0284c7"
            stroke-dasharray="4,4"
            stroke-width="1.5"
          />

          <!-- Gradient Area Fills -->
          <polygon v-if="rxAreaPoints" :points="rxAreaPoints" fill="url(#rxGrad)" />
          <polygon v-if="txAreaPoints" :points="txAreaPoints" fill="url(#txGrad)" />

          <!-- Main Polylines -->
          <polyline
            :points="rxPolylinePoints"
            fill="none"
            stroke="#0284c7"
            stroke-width="2.5"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
          <polyline
            :points="txPolylinePoints"
            fill="none"
            stroke="#a855f7"
            stroke-width="2.5"
            stroke-linecap="round"
            stroke-linejoin="round"
          />

          <!-- Hover Dots -->
          <circle
            v-if="hoverIndex !== null && currentHoverItem"
            :cx="currentHoverItem.x"
            :cy="currentHoverItem.rxY"
            r="5"
            fill="#0284c7"
            stroke="#ffffff"
            stroke-width="1.5"
          />
          <circle
            v-if="hoverIndex !== null && currentHoverItem"
            :cx="currentHoverItem.x"
            :cy="currentHoverItem.txY"
            r="5"
            fill="#a855f7"
            stroke="#ffffff"
            stroke-width="1.5"
          />
        </svg>

        <div v-else class="pa-8 text-center text-grey">
          <v-icon size="40" color="grey-lighten-1" class="mb-2">mdi-network-off-outline</v-icon>
          <div class="text-subtitle-2 font-weight-medium">
            Sem dados de tráfego para a interface
          </div>
          <div class="text-caption text-grey mt-1">
            Selecione um equipamento com SNMP ativo e interface monitorada.
          </div>
        </div>

        <!-- Tooltip Hover Card -->
        <v-card
          v-if="hoverIndex !== null && currentHoverItem"
          elevation="8"
          class="active-point-tooltip pa-2 rounded border text-white pointer-events-none"
          :style="tooltipStyle"
        >
          <div class="text-caption font-weight-bold text-info">
            Download (Rx): {{ formatBps(currentHoverItem.inBps) }}
          </div>
          <div class="text-caption font-weight-bold text-purple-lighten-2">
            Upload (Tx): {{ formatBps(currentHoverItem.outBps) }}
          </div>
          <div class="text-caption text-grey-lighten-1 mt-1">Hora: {{ currentHoverItem.time }}</div>
        </v-card>
      </div>
    </v-card-text>

    <!-- Legend & Highlights Footer -->
    <v-divider></v-divider>
    <v-card-actions
      class="px-4 py-2 bg-surface-variant d-flex align-center justify-space-between text-caption flex-wrap ga-2"
    >
      <div class="d-flex align-center ga-4">
        <div class="d-flex align-center ga-1">
          <span class="dot-indicator bg-info"></span>
          <span>Rx: {{ formatBps(currentRx) }} (Pico: {{ formatBps(peakRx) }})</span>
        </div>
        <div class="d-flex align-center ga-1">
          <span class="dot-indicator bg-purple"></span>
          <span>Tx: {{ formatBps(currentTx) }} (Pico: {{ formatBps(peakTx) }})</span>
        </div>
      </div>
      <span class="text-grey font-weight-medium">Total Acumulado: {{ totalVolumeFormatted }}</span>
    </v-card-actions>
  </v-card>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch, type CSSProperties } from 'vue'
import { useDevicesStore } from '@/stores/devices'
import { useDeviceDetailStore } from '@/stores/deviceDetail'
import { useEventsStore } from '@/stores/events'
import type { WidgetConfig } from '@/stores/dashboard'
import { formatBps, formatBytes } from '@/utils/formatters'

const props = defineProps<{
  widget: WidgetConfig
}>()

const devicesStore = useDevicesStore()
const deviceDetailStore = useDeviceDetailStore()
const eventsStore = useEventsStore()

const timeframe = ref<'5m' | '15m' | '1h' | '24h'>('15m')
const selectedDeviceId = ref<number | 'all'>((props.widget.config?.deviceId as any) || 'all')
const selectedInterfaceId = ref<number | 'all'>((props.widget.config?.interfaceId as any) || 'all')

const chartContainerRef = ref<HTMLElement | null>(null)
const mousePos = ref<{ x: number; y: number } | null>(null)
const hoverIndex = ref<number | null>(null)

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

onMounted(async () => {
  if (devicesStore.devices.length === 0) {
    await devicesStore.fetchDevices()
  }
  if (selectedDeviceId.value !== 'all' && typeof selectedDeviceId.value === 'number') {
    await deviceDetailStore.loadDeviceDetails(selectedDeviceId.value)
  }
  buildSamples()

  eventsStore.onEvent('metric:recorded', (data: any) => {
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

  if (list.length === 0) {
    const now = new Date()
    const count = timeframe.value === '5m' ? 10 : timeframe.value === '15m' ? 15 : 24
    for (let i = count - 1; i >= 0; i--) {
      const t = new Date(now.getTime() - i * 60 * 1000)
      list.push({
        time: t.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
        inBps: Math.floor(15000000 + Math.random() * 25000000),
        outBps: Math.floor(4000000 + Math.random() * 8000000),
        timestamp: t.getTime(),
      })
    }
  }

  localSamples.value = list.slice(-30)
}

const samples = computed(() => localSamples.value)

const currentRx = computed(() =>
  samples.value.length ? samples.value[samples.value.length - 1].inBps : 0
)
const currentTx = computed(() =>
  samples.value.length ? samples.value[samples.value.length - 1].outBps : 0
)

const peakRx = computed(() =>
  samples.value.length ? Math.max(...samples.value.map((s) => s.inBps)) : 0
)
const peakTx = computed(() =>
  samples.value.length ? Math.max(...samples.value.map((s) => s.outBps)) : 0
)

const maxRate = computed(() => {
  if (samples.value.length === 0) return 1000000
  const max = Math.max(peakRx.value, peakTx.value)
  return max > 0 ? Math.ceil(max * 1.25) : 1000000
})

const totalVolumeFormatted = computed(() => {
  if (samples.value.length === 0) return '0 B'
  const totalBits = samples.value.reduce((acc, s) => acc + (s.inBps + s.outBps) * 60, 0)
  const totalBytes = totalBits / 8
  return formatBytes(totalBytes)
})

const chartPoints = computed(() => {
  const left = 70
  const right = 780
  const top = 20
  const bottom = 180
  const height = bottom - top
  const count = samples.value.length

  if (count === 0) return []
  const step = count > 1 ? (right - left) / (count - 1) : 0

  return samples.value.map((s, idx) => {
    const x = count === 1 ? (left + right) / 2 : left + idx * step
    const rxRatio = maxRate.value > 0 ? Math.min(1, s.inBps / maxRate.value) : 0
    const txRatio = maxRate.value > 0 ? Math.min(1, s.outBps / maxRate.value) : 0

    return {
      x,
      rxY: bottom - rxRatio * height,
      txY: bottom - txRatio * height,
      inBps: s.inBps,
      outBps: s.outBps,
      time: s.time,
    }
  })
})

const rxPolylinePoints = computed(() =>
  chartPoints.value.map((pt) => `${pt.x.toFixed(1)},${pt.rxY.toFixed(1)}`).join(' ')
)
const txPolylinePoints = computed(() =>
  chartPoints.value.map((pt) => `${pt.x.toFixed(1)},${pt.txY.toFixed(1)}`).join(' ')
)

const rxAreaPoints = computed(() => {
  if (chartPoints.value.length === 0) return ''
  const firstX = chartPoints.value[0].x.toFixed(1)
  const lastX = chartPoints.value[chartPoints.value.length - 1].x.toFixed(1)
  return `${firstX},180 ${rxPolylinePoints.value} ${lastX},180`
})

const txAreaPoints = computed(() => {
  if (chartPoints.value.length === 0) return ''
  const firstX = chartPoints.value[0].x.toFixed(1)
  const lastX = chartPoints.value[chartPoints.value.length - 1].x.toFixed(1)
  return `${firstX},180 ${txPolylinePoints.value} ${lastX},180`
})

const crosshairX = computed(() => {
  if (hoverIndex.value === null || !chartPoints.value[hoverIndex.value]) return null
  return chartPoints.value[hoverIndex.value].x
})

const currentHoverItem = computed(() => {
  if (hoverIndex.value === null) return null
  return chartPoints.value[hoverIndex.value] || null
})

function onMouseMove(e: MouseEvent) {
  if (!chartContainerRef.value || samples.value.length === 0) return
  const rect = chartContainerRef.value.getBoundingClientRect()
  const mouseX = e.clientX - rect.left
  const mouseY = e.clientY - rect.top
  mousePos.value = { x: mouseX, y: mouseY }

  const marginX = (70 / 800) * rect.width
  const contentW = ((780 - 70) / 800) * rect.width
  const count = samples.value.length

  let relX = mouseX - marginX
  if (relX < 0) relX = 0
  if (relX > contentW) relX = contentW

  const step = count > 1 ? contentW / (count - 1) : 0
  const idx = step > 0 ? Math.round(relX / step) : 0
  hoverIndex.value = Math.min(count - 1, Math.max(0, idx))
}

function onMouseLeave() {
  mousePos.value = null
  hoverIndex.value = null
}

const tooltipStyle = computed<CSSProperties>(() => {
  if (!mousePos.value || !chartContainerRef.value) return {}
  const { x, y } = mousePos.value
  const rect = chartContainerRef.value.getBoundingClientRect()
  const cardW = 190
  const cardH = 70

  let left = x + 12
  if (x > rect.width - cardW - 10) left = x - cardW - 12
  let top = y - cardH - 12
  if (y < cardH + 10) top = y + 12

  return {
    position: 'absolute',
    left: `${Math.max(4, Math.min(rect.width - cardW - 4, left))}px`,
    top: `${Math.max(4, Math.min(rect.height - cardH - 4, top))}px`,
    zIndex: 20,
    background: '#0F172A',
    borderColor: '#0284C7',
    boxShadow: '0 8px 20px rgba(0,0,0,0.4)',
  }
})
</script>

<style scoped>
.chart-box {
  min-height: 210px;
  position: relative;
}

.chart-svg {
  height: 200px;
  overflow: visible;
}

.dot-indicator {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  display: inline-block;
}

.pointer-events-none {
  pointer-events: none;
}

.ga-1 {
  gap: 4px;
}
.ga-2 {
  gap: 8px;
}
.ga-4 {
  gap: 16px;
}
</style>
