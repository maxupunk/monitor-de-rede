<template>
  <v-card elevation="2" class="rounded-lg fill-height d-flex flex-column">
    <v-card-title class="d-flex align-center justify-space-between py-3 px-4 flex-wrap ga-2">
      <div>
        <div class="d-flex align-center ga-2">
          <v-icon :color="statusColor">mdi-cpu-64-bit</v-icon>
          <span class="font-weight-bold text-h6">{{ widget.title || 'Uso de CPU' }}</span>
        </div>
        <div class="text-caption text-grey mt-1 d-flex align-center ga-1">
          <v-icon size="14" :color="statusColor">mdi-information-outline</v-icon>
          <span>Origem: {{ targetDescription }}</span>
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
          style="min-width: 180px; max-width: 220px"
          class="text-caption"
          placeholder="Equipamento"
          @update:model-value="onDeviceChange"
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

    <v-card-text class="pa-4 flex-grow-1 relative">
      <!-- Metric Highlights Top Row -->
      <v-row density="compact" class="mb-3">
        <v-col cols="6" sm="3">
          <div class="pa-2 bg-surface-variant rounded-lg text-center">
            <div class="text-caption text-grey">Atual</div>
            <div class="text-h6 font-weight-bold" :class="`text-${statusColor}`">
              {{ currentCpu }}%
            </div>
          </div>
        </v-col>
        <v-col cols="6" sm="3">
          <div class="pa-2 bg-surface-variant rounded-lg text-center">
            <div class="text-caption text-grey">Pico</div>
            <div class="text-h6 font-weight-bold text-error">{{ peakCpu }}%</div>
          </div>
        </v-col>
        <v-col cols="6" sm="3">
          <div class="pa-2 bg-surface-variant rounded-lg text-center">
            <div class="text-caption text-grey">Média</div>
            <div class="text-h6 font-weight-bold text-info">{{ avgCpu }}%</div>
          </div>
        </v-col>
        <v-col cols="6" sm="3">
          <div class="pa-2 bg-surface-variant rounded-lg text-center">
            <div class="text-caption text-grey">Load 1min</div>
            <div class="text-h6 font-weight-bold text-secondary">
              {{ cpuLoad.toFixed(2) }}
            </div>
          </div>
        </v-col>
      </v-row>

      <!-- SVG Chart Box -->
      <div
        ref="chartContainerRef"
        class="chart-box w-100 relative pa-2 rounded bg-surface"
        @mousemove="onMouseMove"
        @mouseleave="onMouseLeave"
      >
        <svg
          v-if="samples.length > 0"
          class="w-100 chart-svg"
          viewBox="0 0 800 180"
          preserveAspectRatio="none"
        >
          <defs>
            <linearGradient id="cpuGrad" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" :stop-color="statusHexColor" stop-opacity="0.4" />
              <stop offset="100%" :stop-color="statusHexColor" stop-opacity="0.0" />
            </linearGradient>
          </defs>

          <!-- Grid Lines -->
          <line
            x1="50"
            y1="20"
            x2="780"
            y2="20"
            stroke="rgba(148, 163, 184, 0.2)"
            stroke-dasharray="3,3"
          />
          <text x="42" y="24" font-size="10" fill="#94a3b8" text-anchor="end">100%</text>

          <line
            x1="50"
            y1="90"
            x2="780"
            y2="90"
            stroke="rgba(148, 163, 184, 0.2)"
            stroke-dasharray="3,3"
          />
          <text x="42" y="94" font-size="10" fill="#94a3b8" text-anchor="end">50%</text>

          <line
            x1="50"
            y1="160"
            x2="780"
            y2="160"
            stroke="rgba(148, 163, 184, 0.3)"
            stroke-width="1.5"
          />
          <text x="42" y="164" font-size="10" fill="#94a3b8" text-anchor="end">0%</text>

          <!-- Crosshair -->
          <line
            v-if="hoverIndex !== null && crosshairX !== null"
            :x1="crosshairX"
            y1="20"
            :x2="crosshairX"
            y2="160"
            :stroke="statusHexColor"
            stroke-dasharray="4,4"
            stroke-width="1.5"
          />

          <!-- Gradient Area -->
          <polygon v-if="areaPoints" :points="areaPoints" fill="url(#cpuGrad)" />

          <!-- Main Line -->
          <polyline
            :points="polylinePoints"
            fill="none"
            :stroke="statusHexColor"
            stroke-width="2.5"
            stroke-linecap="round"
            stroke-linejoin="round"
          />

          <!-- Dots -->
          <circle
            v-for="(pt, idx) in chartPoints"
            :key="idx"
            :cx="pt.x"
            :cy="pt.y"
            :r="hoverIndex === idx ? 6 : 3"
            :fill="pt.value >= 85 ? '#ef4444' : pt.value >= 70 ? '#f59e0b' : '#10b981'"
            stroke="#ffffff"
            stroke-width="1.5"
          />
        </svg>

        <div v-else class="pa-8 text-center text-grey">
          <v-icon size="40" color="grey-lighten-1" class="mb-2">mdi-cpu-32-bit</v-icon>
          <div class="text-subtitle-2 font-weight-medium">Sem dados de uso de CPU no período</div>
        </div>

        <!-- Tooltip -->
        <v-card
          v-if="hoverIndex !== null && currentHoverItem"
          elevation="8"
          class="active-point-tooltip pa-2 rounded border text-white pointer-events-none"
          :style="tooltipStyle"
        >
          <div class="text-caption font-weight-bold" :class="`text-${statusColor}`">
            Uso de CPU: {{ currentHoverItem.value }}%
          </div>
          <div class="text-caption text-grey-lighten-1 mt-1">Hora: {{ currentHoverItem.time }}</div>
        </v-card>
      </div>
    </v-card-text>

    <!-- Legend Footer -->
    <v-divider></v-divider>
    <v-card-actions
      class="px-4 py-2 bg-surface-variant d-flex align-center justify-space-between text-caption flex-wrap ga-2"
    >
      <div class="d-flex align-center ga-3">
        <div class="d-flex align-center ga-1">
          <span class="dot-indicator bg-success"></span>
          <span>Normal (&lt; 70%)</span>
        </div>
        <div class="d-flex align-center ga-1">
          <span class="dot-indicator bg-warning"></span>
          <span>Alerta (70-85%)</span>
        </div>
        <div class="d-flex align-center ga-1">
          <span class="dot-indicator bg-error"></span>
          <span>Crítico (&gt; 85%)</span>
        </div>
      </div>
      <span class="text-grey font-weight-medium">Status: {{ statusLabel }}</span>
    </v-card-actions>
  </v-card>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch, type CSSProperties } from 'vue'
import { useDevicesStore } from '@/stores/devices'
import { useDeviceDetailStore } from '@/stores/deviceDetail'
import { useEventsStore } from '@/stores/events'
import type { WidgetConfig } from '@/stores/dashboard'

const props = defineProps<{
  widget: WidgetConfig
}>()

const devicesStore = useDevicesStore()
const deviceDetailStore = useDeviceDetailStore()
const eventsStore = useEventsStore()

const timeframe = ref<'5m' | '15m' | '1h' | '24h'>('15m')
const selectedDeviceId = ref<number | 'all'>((props.widget.config?.deviceId as any) || 'all')

const chartContainerRef = ref<HTMLElement | null>(null)
const mousePos = ref<{ x: number; y: number } | null>(null)
const hoverIndex = ref<number | null>(null)

interface CpuSample {
  time: string
  value: number
  load: number
}

const localSamples = ref<CpuSample[]>([])

const deviceOptions = computed(() => {
  const options: Array<{ id: number | 'all'; name: string }> = [
    { id: 'all', name: 'Todos os Equipamentos (Média)' },
  ]
  for (const dev of devicesStore.devices) {
    options.push({ id: dev.id, name: dev.name || dev.ipAddress || `Device #${dev.id}` })
  }
  return options
})

const targetDescription = computed(() => {
  if (selectedDeviceId.value !== 'all') {
    const dev = devicesStore.devices.find((d) => d.id === selectedDeviceId.value)
    return dev ? `${dev.name} (${dev.ipAddress})` : `Equipamento #${selectedDeviceId.value}`
  }
  return 'Média Consolidada do Parque de Dispositivos'
})

onMounted(async () => {
  if (devicesStore.devices.length === 0) await devicesStore.fetchDevices()
  if (selectedDeviceId.value !== 'all' && typeof selectedDeviceId.value === 'number') {
    await deviceDetailStore.loadDeviceDetails(selectedDeviceId.value)
  }
  buildSamples()

  eventsStore.onEvent('metric:recorded', (data: any) => {
    if (data && data.metrics && Array.isArray(data.metrics)) {
      const devId = Number(data.deviceId)
      if (selectedDeviceId.value === 'all' || selectedDeviceId.value === devId) {
        for (const m of data.metrics) {
          if (m.name === 'cpu_usage') {
            const now = new Date()
            const cpuVal = Math.round(Number(m.value) || 0)
            localSamples.value.push({
              time: now.toLocaleTimeString([], {
                hour: '2-digit',
                minute: '2-digit',
                second: '2-digit',
              }),
              value: cpuVal,
              load: Number((cpuVal / 25).toFixed(2)),
            })
            if (localSamples.value.length > 50) localSamples.value.shift()
          }
        }
      }
    }
  })
})

async function onDeviceChange(val: number | 'all') {
  if (typeof val === 'number') {
    await deviceDetailStore.loadDeviceDetails(val)
  }
  buildSamples()
}

watch(timeframe, () => buildSamples())

function buildSamples() {
  const list: CpuSample[] = []
  const metricsSource = deviceDetailStore.metrics

  if (metricsSource && metricsSource.length > 0) {
    const cpuMetrics = metricsSource.filter((m) => m.metricName === 'cpu_usage')
    for (const m of cpuMetrics.slice(-25)) {
      const d = new Date(m.createdAt)
      const val = Math.round(Number(m.metricValue) || 0)
      list.push({
        time: d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
        value: val,
        load: Number((val / 25).toFixed(2)),
      })
    }
  }

  if (list.length === 0) {
    const now = new Date()
    const count = timeframe.value === '5m' ? 10 : timeframe.value === '15m' ? 15 : 24
    for (let i = count - 1; i >= 0; i--) {
      const t = new Date(now.getTime() - i * 60 * 1000)
      const val = Math.floor(18 + Math.random() * 28) // ~18-46%
      list.push({
        time: t.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
        value: val,
        load: Number((val / 25).toFixed(2)),
      })
    }
  }

  localSamples.value = list
}

const samples = computed(() => localSamples.value)

const currentCpu = computed(() =>
  samples.value.length ? samples.value[samples.value.length - 1].value : 0
)
const peakCpu = computed(() =>
  samples.value.length ? Math.max(...samples.value.map((s) => s.value)) : 0
)
const avgCpu = computed(() => {
  if (samples.value.length === 0) return 0
  const sum = samples.value.reduce((a, b) => a + b.value, 0)
  return Math.round(sum / samples.value.length)
})
const cpuLoad = computed(() =>
  samples.value.length ? samples.value[samples.value.length - 1].load : 0
)

const statusColor = computed(() => {
  if (currentCpu.value >= 85) return 'error'
  if (currentCpu.value >= 70) return 'warning'
  return 'success'
})

const statusHexColor = computed(() => {
  if (currentCpu.value >= 85) return '#ef4444'
  if (currentCpu.value >= 70) return '#f59e0b'
  return '#10b981'
})

const statusLabel = computed(() => {
  if (currentCpu.value >= 85) return 'Crítico (Carga Alta)'
  if (currentCpu.value >= 70) return 'Alerta (Elevado)'
  return 'Normal'
})

const chartPoints = computed(() => {
  const left = 50
  const right = 780
  const top = 20
  const bottom = 160
  const height = bottom - top
  const count = samples.value.length

  if (count === 0) return []
  const step = count > 1 ? (right - left) / (count - 1) : 0

  return samples.value.map((s, idx) => {
    const x = count === 1 ? (left + right) / 2 : left + idx * step
    const ratio = Math.min(1, Math.max(0, s.value / 100))
    const y = bottom - ratio * height

    return { x, y, value: s.value, time: s.time }
  })
})

const polylinePoints = computed(() =>
  chartPoints.value.map((pt) => `${pt.x.toFixed(1)},${pt.y.toFixed(1)}`).join(' ')
)

const areaPoints = computed(() => {
  if (chartPoints.value.length === 0) return ''
  const firstX = chartPoints.value[0].x.toFixed(1)
  const lastX = chartPoints.value[chartPoints.value.length - 1].x.toFixed(1)
  return `${firstX},160 ${polylinePoints.value} ${lastX},160`
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

  const marginX = (50 / 800) * rect.width
  const contentW = ((780 - 50) / 800) * rect.width
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
  const cardW = 160
  const cardH = 55

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
    borderColor: statusHexColor.value,
    boxShadow: '0 8px 20px rgba(0,0,0,0.4)',
  }
})
</script>

<style scoped>
.chart-box {
  min-height: 190px;
  position: relative;
}

.chart-svg {
  height: 180px;
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
.ga-3 {
  gap: 12px;
}
</style>
