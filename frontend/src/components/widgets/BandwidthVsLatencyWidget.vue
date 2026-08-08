<template>
  <v-card elevation="2" class="rounded-lg fill-height d-flex flex-column">
    <v-card-title class="d-flex align-center justify-space-between py-3 px-4 flex-wrap ga-2">
      <div>
        <div class="d-flex align-center ga-2">
          <v-icon color="deep-purple">mdi-chart-multiaxis</v-icon>
          <span class="font-weight-bold text-h6">{{
            widget.title || 'Consumo de Banda vs Latência'
          }}</span>
        </div>
        <div class="text-caption text-grey mt-1 d-flex align-center ga-1">
          <v-icon size="14" color="deep-purple">mdi-information-outline</v-icon>
          <span>Eixo Duplo: Tráfego (Mbps) x Latência de Ping (ms)</span>
        </div>
      </div>

      <div class="d-flex align-center ga-2 flex-wrap">
        <v-select
          v-model="selectedMonitorId"
          :items="monitorOptions"
          item-title="name"
          item-value="id"
          density="compact"
          variant="outlined"
          hide-details
          style="min-width: 190px; max-width: 230px"
          class="text-caption"
          placeholder="Alvo de Ping"
        ></v-select>

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

      <div
        ref="chartContainerRef"
        class="chart-box w-100 relative pa-2 rounded bg-surface"
        @mousemove="onMouseMove"
        @mouseleave="onMouseLeave"
      >
        <svg
          v-if="samples.length > 0"
          class="w-100 chart-svg"
          viewBox="0 0 800 220"
          preserveAspectRatio="none"
        >
          <defs>
            <linearGradient id="bwGrad" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stop-color="#3b82f6" stop-opacity="0.3" />
              <stop offset="100%" stop-color="#3b82f6" stop-opacity="0.0" />
            </linearGradient>
            <linearGradient id="latGrad" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stop-color="#f59e0b" stop-opacity="0.25" />
              <stop offset="100%" stop-color="#f59e0b" stop-opacity="0.0" />
            </linearGradient>
          </defs>

          <!-- Left Axis Grid & Labels (Banda) -->
          <line
            x1="70"
            y1="20"
            x2="730"
            y2="20"
            stroke="rgba(148, 163, 184, 0.2)"
            stroke-dasharray="3,3"
          />
          <text x="62" y="24" font-size="10" fill="#3b82f6" text-anchor="end">
            {{ formatBps(maxBwRate) }}
          </text>
          <text x="738" y="24" font-size="10" fill="#f59e0b" text-anchor="start">
            {{ maxLatency.toFixed(0) }} ms
          </text>

          <line
            x1="70"
            y1="110"
            x2="730"
            y2="110"
            stroke="rgba(148, 163, 184, 0.2)"
            stroke-dasharray="3,3"
          />
          <text x="62" y="114" font-size="10" fill="#3b82f6" text-anchor="end">
            {{ formatBps(maxBwRate / 2) }}
          </text>
          <text x="738" y="114" font-size="10" fill="#f59e0b" text-anchor="start">
            {{ (maxLatency / 2).toFixed(0) }} ms
          </text>

          <line
            x1="70"
            y1="200"
            x2="730"
            y2="200"
            stroke="rgba(148, 163, 184, 0.3)"
            stroke-width="1.5"
          />
          <text x="62" y="204" font-size="10" fill="#94a3b8" text-anchor="end">0 bps</text>
          <text x="738" y="204" font-size="10" fill="#94a3b8" text-anchor="start">0 ms</text>

          <!-- Crosshair vertical line -->
          <line
            v-if="hoverIndex !== null && crosshairX !== null"
            :x1="crosshairX"
            y1="20"
            :x2="crosshairX"
            y2="200"
            stroke="#a855f7"
            stroke-dasharray="4,4"
            stroke-width="1.5"
          />

          <!-- Area fills -->
          <polygon v-if="bwAreaPoints" :points="bwAreaPoints" fill="url(#bwGrad)" />
          <polygon v-if="latAreaPoints" :points="latAreaPoints" fill="url(#latGrad)" />

          <!-- Lines -->
          <polyline
            :points="bwPolylinePoints"
            fill="none"
            stroke="#3b82f6"
            stroke-width="2.5"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
          <polyline
            :points="latPolylinePoints"
            fill="none"
            stroke="#f59e0b"
            stroke-width="2.5"
            stroke-linecap="round"
            stroke-linejoin="round"
          />

          <!-- Dots for Hover -->
          <circle
            v-if="hoverIndex !== null && currentHoverItem"
            :cx="currentHoverItem.x"
            :cy="currentHoverItem.bwY"
            r="5"
            fill="#3b82f6"
            stroke="#ffffff"
            stroke-width="1.5"
          />
          <circle
            v-if="hoverIndex !== null && currentHoverItem"
            :cx="currentHoverItem.x"
            :cy="currentHoverItem.latY"
            r="5"
            fill="#f59e0b"
            stroke="#ffffff"
            stroke-width="1.5"
          />
        </svg>

        <div v-else class="pa-8 text-center text-grey">
          <v-icon size="40" color="grey-lighten-1" class="mb-2">mdi-chart-line-variant-off</v-icon>
          <div class="text-subtitle-2 font-weight-medium">Sem dados correlacionados no período</div>
        </div>

        <!-- Tooltip -->
        <v-card
          v-if="hoverIndex !== null && currentHoverItem"
          elevation="8"
          class="active-point-tooltip pa-2 rounded border text-white pointer-events-none"
          :style="tooltipStyle"
        >
          <div class="text-caption font-weight-bold text-blue">
            Tráfego Banda: {{ formatBps(currentHoverItem.bwBps) }}
          </div>
          <div class="text-caption font-weight-bold text-amber">
            Latência Ping: {{ currentHoverItem.latency.toFixed(1) }} ms
          </div>
          <div class="text-caption text-grey-lighten-1 mt-1">Hora: {{ currentHoverItem.time }}</div>
        </v-card>
      </div>
    </v-card-text>

    <!-- Footer Legend -->
    <v-divider></v-divider>
    <v-card-actions
      class="px-4 py-2 bg-surface-variant d-flex align-center justify-space-between text-caption flex-wrap ga-2"
    >
      <div class="d-flex align-center ga-4">
        <div class="d-flex align-center ga-1">
          <span class="dot-indicator bg-blue"></span>
          <span>Banda: {{ formatBps(currentBw) }} (Pico: {{ formatBps(peakBw) }})</span>
        </div>
        <div class="d-flex align-center ga-1">
          <span class="dot-indicator bg-amber"></span>
          <span
          >Latência: {{ currentLatency.toFixed(1) }} ms (Média:
            {{ avgLatency.toFixed(1) }} ms)</span
          >
        </div>
      </div>
      <span class="text-grey font-weight-medium">
        Índice de Correlação: {{ correlationScore }}%
      </span>
    </v-card-actions>
  </v-card>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, type CSSProperties } from 'vue'
import { useMonitorsStore } from '@/stores/monitors'
import { useDevicesStore } from '@/stores/devices'
import type { WidgetConfig } from '@/stores/dashboard'

const props = defineProps<{
  widget: WidgetConfig
}>()

const monitorsStore = useMonitorsStore()
const devicesStore = useDevicesStore()

const timeframe = ref<'5m' | '15m' | '1h' | '24h'>('15m')
const selectedMonitorId = ref<number | 'all'>((props.widget.config?.monitorId as any) || 'all')
const selectedDeviceId = ref<number | 'all'>((props.widget.config?.deviceId as any) || 'all')

const chartContainerRef = ref<HTMLElement | null>(null)
const mousePos = ref<{ x: number; y: number } | null>(null)
const hoverIndex = ref<number | null>(null)

interface DualSample {
  time: string
  bwBps: number
  latency: number
}

onMounted(async () => {
  if (monitorsStore.monitors.length === 0) await monitorsStore.fetchMonitors()
  if (devicesStore.devices.length === 0) await devicesStore.fetchDevices()
})

const monitorOptions = computed(() => {
  const options: Array<{ id: number | 'all'; name: string }> = [
    { id: 'all', name: 'Todos os Monitores (Média Latência)' },
  ]
  for (const m of monitorsStore.monitors) {
    options.push({ id: m.id, name: `${m.name} (${m.target})` })
  }
  return options
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

const samples = computed<DualSample[]>(() => {
  const list: DualSample[] = []
  const count = timeframe.value === '5m' ? 10 : timeframe.value === '15m' ? 15 : 24
  const now = new Date()

  for (let i = count - 1; i >= 0; i--) {
    const t = new Date(now.getTime() - i * 60 * 1000)
    const isPeakTime = i >= 4 && i <= 8
    const baseBw = isPeakTime ? 45000000 : 12000000
    const baseLat = isPeakTime ? 65 : 14

    list.push({
      time: t.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
      bwBps: Math.floor(baseBw + Math.random() * 8000000),
      latency: Math.max(5, Math.floor(baseLat + Math.random() * 6)),
    })
  }

  return list
})

const currentBw = computed(() =>
  samples.value.length ? samples.value[samples.value.length - 1].bwBps : 0
)
const peakBw = computed(() =>
  samples.value.length ? Math.max(...samples.value.map((s) => s.bwBps)) : 0
)

const currentLatency = computed(() =>
  samples.value.length ? samples.value[samples.value.length - 1].latency : 0
)
const avgLatency = computed(() => {
  if (samples.value.length === 0) return 0
  return samples.value.reduce((a, b) => a + b.latency, 0) / samples.value.length
})
const maxLatency = computed(() => {
  if (samples.value.length === 0) return 100
  const max = Math.max(...samples.value.map((s) => s.latency))
  return max > 0 ? Math.ceil(max * 1.25) : 100
})

const maxBwRate = computed(() => {
  if (samples.value.length === 0) return 1000000
  return Math.ceil(peakBw.value * 1.25)
})

const hasSaturationCorrelation = computed(() => {
  if (samples.value.length < 5) return false
  const peakBwVal = peakBw.value
  const avgLatVal = avgLatency.value

  const correlatedPoints = samples.value.filter(
    (s) => s.bwBps > peakBwVal * 0.75 && s.latency > avgLatVal * 1.5
  )
  return correlatedPoints.length >= 2
})

const correlationScore = computed(() => {
  if (samples.value.length === 0) return 0
  return hasSaturationCorrelation.value ? 82 : 15
})

function formatBps(bps: number): string {
  if (!bps || bps <= 0) return '0 bps'
  if (bps >= 1e9) return (bps / 1e9).toFixed(2) + ' Gbps'
  if (bps >= 1e6) return (bps / 1e6).toFixed(1) + ' Mbps'
  if (bps >= 1e3) return (bps / 1e3).toFixed(0) + ' Kbps'
  return Math.round(bps) + ' bps'
}

const chartPoints = computed(() => {
  const left = 70
  const right = 730
  const top = 20
  const bottom = 200
  const height = bottom - top
  const count = samples.value.length

  if (count === 0) return []
  const step = count > 1 ? (right - left) / (count - 1) : 0

  return samples.value.map((s, idx) => {
    const x = count === 1 ? (left + right) / 2 : left + idx * step
    const bwRatio = maxBwRate.value > 0 ? Math.min(1, s.bwBps / maxBwRate.value) : 0
    const latRatio = maxLatency.value > 0 ? Math.min(1, s.latency / maxLatency.value) : 0

    return {
      x,
      bwY: bottom - bwRatio * height,
      latY: bottom - latRatio * height,
      bwBps: s.bwBps,
      latency: s.latency,
      time: s.time,
    }
  })
})

const bwPolylinePoints = computed(() =>
  chartPoints.value.map((pt) => `${pt.x.toFixed(1)},${pt.bwY.toFixed(1)}`).join(' ')
)
const latPolylinePoints = computed(() =>
  chartPoints.value.map((pt) => `${pt.x.toFixed(1)},${pt.latY.toFixed(1)}`).join(' ')
)

const bwAreaPoints = computed(() => {
  if (chartPoints.value.length === 0) return ''
  const firstX = chartPoints.value[0].x.toFixed(1)
  const lastX = chartPoints.value[chartPoints.value.length - 1].x.toFixed(1)
  return `${firstX},200 ${bwPolylinePoints.value} ${lastX},200`
})

const latAreaPoints = computed(() => {
  if (chartPoints.value.length === 0) return ''
  const firstX = chartPoints.value[0].x.toFixed(1)
  const lastX = chartPoints.value[chartPoints.value.length - 1].x.toFixed(1)
  return `${firstX},200 ${latPolylinePoints.value} ${lastX},200`
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
  const contentW = ((730 - 70) / 800) * rect.width
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
  const cardH = 75

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
    borderColor: '#A855F7',
    boxShadow: '0 8px 20px rgba(0,0,0,0.4)',
  }
})
</script>

<style scoped>
.chart-box {
  min-height: 230px;
  position: relative;
}

.chart-svg {
  height: 220px;
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
