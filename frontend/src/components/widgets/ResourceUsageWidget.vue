<template>
  <v-card elevation="2" class="rounded-lg fill-height d-flex flex-column">
    <v-card-title class="d-flex align-center justify-space-between py-3 px-4 flex-wrap ga-2">
      <div>
        <div class="d-flex align-center ga-2">
          <v-icon :color="statusColor">{{ config.icon }}</v-icon>
          <span class="font-weight-bold text-h6">{{ widget.title || config.defaultTitle }}</span>
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
              {{ formatUsageValue(currentUsage) }}
            </div>
            <div
              v-if="resourceType === 'ram' && currentUsagePercent !== null"
              class="text-caption text-grey"
            >
              {{ currentUsagePercent.toFixed(1) }}% do total
            </div>
          </div>
        </v-col>
        <v-col cols="6" sm="3">
          <div class="pa-2 bg-surface-variant rounded-lg text-center">
            <div class="text-caption text-grey">Pico</div>
            <div class="text-h6 font-weight-bold text-error">{{ formatUsageValue(peakUsage) }}</div>
          </div>
        </v-col>
        <v-col cols="6" sm="3">
          <div class="pa-2 bg-surface-variant rounded-lg text-center">
            <div class="text-caption text-grey">Média</div>
            <div class="text-h6 font-weight-bold text-info">{{ formatUsageValue(avgUsage) }}</div>
          </div>
        </v-col>
        <v-col cols="6" sm="3">
          <div class="pa-2 bg-surface-variant rounded-lg text-center">
            <div class="text-caption text-grey">{{ config.fourthCardLabel }}</div>
            <div v-if="resourceType === 'cpu'" class="text-h6 font-weight-bold text-secondary">
              {{ cpuLoad === null ? '—' : cpuLoad.toFixed(2) }}
            </div>
            <div v-else class="text-subtitle-1 font-weight-bold text-primary">
              {{ memoryAllocation }}
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
            <linearGradient :id="`grad-${resourceType}`" x1="0" y1="0" x2="0" y2="1">
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
          <text x="42" y="24" font-size="10" fill="#94a3b8" text-anchor="end">
            {{ chartMaxLabel }}
          </text>

          <line
            x1="50"
            y1="90"
            x2="780"
            y2="90"
            stroke="rgba(148, 163, 184, 0.2)"
            stroke-dasharray="3,3"
          />
          <text x="42" y="94" font-size="10" fill="#94a3b8" text-anchor="end">
            {{ chartMidLabel }}
          </text>

          <line
            x1="50"
            y1="160"
            x2="780"
            y2="160"
            stroke="rgba(148, 163, 184, 0.3)"
            stroke-width="1.5"
          />
          <text x="42" y="164" font-size="10" fill="#94a3b8" text-anchor="end">
            {{ chartMinLabel }}
          </text>

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
          <polygon v-if="areaPoints" :points="areaPoints" :fill="`url(#grad-${resourceType})`" />

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
            :fill="
              pt.usagePercent >= 85 ? '#ef4444' : pt.usagePercent >= 70 ? '#f59e0b' : '#10b981'
            "
            stroke="#ffffff"
            stroke-width="1.5"
          />
        </svg>

        <div v-else class="pa-8 text-center text-grey">
          <v-icon size="40" color="grey-lighten-1" class="mb-2">{{ config.emptyIcon }}</v-icon>
          <div class="text-subtitle-2 font-weight-medium">{{ config.emptyText }}</div>
        </div>

        <!-- Tooltip -->
        <v-card
          v-if="hoverIndex !== null && currentHoverItem"
          elevation="8"
          class="active-point-tooltip pa-2 rounded border text-white pointer-events-none"
          :style="tooltipStyle"
        >
          <div class="text-caption font-weight-bold" :class="`text-${statusColor}`">
            {{ config.label }}: {{ formatUsageValue(currentHoverItem.value) }}
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
          <span>{{ config.legendLabels.normal }}</span>
        </div>
        <div class="d-flex align-center ga-1">
          <span class="dot-indicator bg-warning"></span>
          <span>{{ config.legendLabels.warning }}</span>
        </div>
        <div class="d-flex align-center ga-1">
          <span class="dot-indicator bg-error"></span>
          <span>{{ config.legendLabels.danger }}</span>
        </div>
      </div>
      <span class="text-grey">Janela exibida: {{ timeframe }}</span>
    </v-card-actions>
  </v-card>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { useDevicesStore } from '@/stores/devices'
import { useDeviceDetailStore } from '@/stores/deviceDetail'
import { useEventsStore } from '@/stores/events'
import type { WidgetConfig } from '@/stores/dashboard'
import { formatBinaryBytes } from '@/utils/formatters'
import { chartTooltipStyle } from '@/utils/chartTooltip'
import {
  latestMetricValue,
  resourceMetricWindow,
  RESOURCE_SERIES,
  type ResourceTimeframe,
} from '@/utils/resourceMetrics'

export type ResourceType = 'cpu' | 'ram'

const props = withDefaults(
  defineProps<{
    widget: WidgetConfig
    type?: ResourceType
  }>(),
  {
    type: 'cpu',
  }
)

const resourceType = computed<ResourceType>(() => {
  if (props.type) return props.type
  if (props.widget.type === 'ram-usage') return 'ram'
  return 'cpu'
})

const CONFIGS = {
  cpu: {
    icon: 'mdi-cpu-64-bit',
    emptyIcon: 'mdi-cpu-32-bit',
    defaultTitle: 'Uso de CPU',
    label: 'Uso de CPU',
    emptyText: 'Sem dados de uso de CPU no período',
    fourthCardLabel: 'Load 1min',
    metricNames: RESOURCE_SERIES.cpu,
    legendLabels: {
      normal: 'Normal (< 70%)',
      warning: 'Alerta (70-85%)',
      danger: 'Crítico (> 85%)',
    },
  },
  ram: {
    icon: 'mdi-memory',
    emptyIcon: 'mdi-chip',
    defaultTitle: 'Uso de RAM',
    label: 'Uso de RAM',
    emptyText: 'Sem dados de uso de RAM no período',
    fourthCardLabel: 'Alocação',
    metricNames: RESOURCE_SERIES.ram,
    legendLabels: {
      normal: 'Suficiente (< 70%)',
      warning: 'Atenção (70-85%)',
      danger: 'Esgotando (> 85%)',
    },
  },
} as const

const config = computed(() => CONFIGS[resourceType.value])

const devicesStore = useDevicesStore()
const deviceDetailStore = useDeviceDetailStore()
const eventsStore = useEventsStore()

const timeframe = ref<ResourceTimeframe>('15m')
const configuredDeviceId = Number(props.widget.config?.deviceId)
const selectedDeviceId = ref<number | null>(
  Number.isInteger(configuredDeviceId) && configuredDeviceId > 0 ? configuredDeviceId : null
)

const chartContainerRef = ref<HTMLElement | null>(null)
const mousePos = ref<{ x: number; y: number } | null>(null)
const hoverIndex = ref<number | null>(null)

interface SamplePoint {
  time: string
  value: number
  timestamp: number
}

const localSamples = ref<SamplePoint[]>([])

const deviceOptions = computed(() => {
  const options: Array<{ id: number; name: string }> = []
  for (const dev of devicesStore.devices) {
    options.push({ id: dev.id, name: dev.name || dev.ipAddress || `Device #${dev.id}` })
  }
  return options
})

const targetDescription = computed(() => {
  const dev = devicesStore.devices.find((d) => d.id === selectedDeviceId.value)
  if (dev) return `${dev.name} (${dev.ipAddress || 'SNMP'})`
  return selectedDeviceId.value
    ? `Dispositivo #${selectedDeviceId.value}`
    : 'Selecione um equipamento'
})

let stopMetricListener: (() => void) | null = null

onMounted(async () => {
  if (devicesStore.devices.length === 0) {
    await devicesStore.fetchDevices()
  }
  if (selectedDeviceId.value === null) {
    selectedDeviceId.value = devicesStore.devices.find((device) => device.isSystem)?.id ?? null
  }
  if (selectedDeviceId.value !== null) {
    await deviceDetailStore.loadDeviceDetails(selectedDeviceId.value)
  }
  buildSamples()

  stopMetricListener = eventsStore.onEvent('metric:recorded', (data: any) => {
    if (data && data.metrics && Array.isArray(data.metrics)) {
      const devId = Number(data.deviceId)
      if (selectedDeviceId.value === devId) {
        for (const m of data.metrics) {
          if ((config.value.metricNames as readonly string[]).includes(String(m.name))) {
            const val = Number(m.value) || 0
            const recordedAt = String(m.recordedAt ?? data.recordedAt ?? new Date().toISOString())
            const recordedDate = new Date(recordedAt)
            localSamples.value.push({
              time: recordedDate.toLocaleTimeString([], {
                hour: '2-digit',
                minute: '2-digit',
                second: '2-digit',
              }),
              value: Math.max(0, val),
              timestamp: recordedDate.getTime(),
            })
            if (localSamples.value.length > 120) {
              localSamples.value.shift()
            }
            break
          }
        }
      }
    }
  })
})

onUnmounted(() => stopMetricListener?.())

async function onDeviceChange(val: number | null) {
  if (val !== null) await deviceDetailStore.loadDeviceDetails(val)
  buildSamples()
}

watch(timeframe, () => {
  buildSamples()
})

function buildSamples() {
  const history = resourceMetricWindow(
    deviceDetailStore.metrics,
    config.value.metricNames,
    timeframe.value
  )
  localSamples.value = history.map((metric) => {
    const date = new Date(metric.createdAt)
    return {
      time: date.toLocaleTimeString([], {
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit',
      }),
      value: Math.max(0, Number(metric.metricValue)),
      timestamp: date.getTime(),
    }
  })
}

const samples = computed(() => localSamples.value)

const currentUsage = computed(() =>
  samples.value.length ? samples.value[samples.value.length - 1].value : 0
)
const peakUsage = computed(() =>
  samples.value.length ? Math.max(...samples.value.map((s) => s.value)) : 0
)
const avgUsage = computed(() => {
  if (samples.value.length === 0) return 0
  const sum = samples.value.reduce((acc, s) => acc + s.value, 0)
  return sum / samples.value.length
})

const cpuLoad = computed(() =>
  latestMetricValue(deviceDetailStore.metrics, RESOURCE_SERIES.loadAverage)
)
const totalRamBytes = computed(() =>
  latestMetricValue(deviceDetailStore.metrics, RESOURCE_SERIES.memoryTotalBytes)
)
const usedRamBytes = computed(() =>
  latestMetricValue(deviceDetailStore.metrics, RESOURCE_SERIES.memoryUsedBytes)
)
const memoryAllocation = computed(() => {
  if (totalRamBytes.value === null) return 'N/D'
  const used = usedRamBytes.value ?? currentUsage.value
  return `${formatBinaryBytes(used)} / ${formatBinaryBytes(totalRamBytes.value)}`
})

const currentUsagePercent = computed(() => {
  if (resourceType.value === 'cpu') return currentUsage.value
  if (totalRamBytes.value === null || totalRamBytes.value <= 0) return null
  return (currentUsage.value / totalRamBytes.value) * 100
})

function formatUsageValue(value: number): string {
  return resourceType.value === 'ram'
    ? formatBinaryBytes(value, { fractionDigits: 1 })
    : `${value.toFixed(1)}%`
}

const statusColor = computed(() => {
  const usage = currentUsagePercent.value ?? 0
  if (usage >= 85) return 'error'
  if (usage >= 70) return 'warning'
  return 'success'
})

const statusHexColor = computed(() => {
  const usage = currentUsagePercent.value ?? 0
  if (usage >= 85) return '#ef4444'
  if (usage >= 70) return '#f59e0b'
  return '#10b981'
})

const chartMaxValue = computed(() => {
  if (resourceType.value === 'cpu') return 100
  if (totalRamBytes.value !== null && totalRamBytes.value > 0) return totalRamBytes.value
  const peak = peakUsage.value
  return peak > 0 ? peak * 1.15 : 1
})
const chartMaxLabel = computed(() => formatUsageValue(chartMaxValue.value))
const chartMidLabel = computed(() => formatUsageValue(chartMaxValue.value / 2))
const chartMinLabel = computed(() => formatUsageValue(0))

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
    const ratio = Math.min(1, Math.max(0, s.value / chartMaxValue.value))
    const y = bottom - ratio * height
    const usagePercent =
      resourceType.value === 'ram' && totalRamBytes.value
        ? (s.value / totalRamBytes.value) * 100
        : s.value
    return { x, y, value: s.value, usagePercent, time: s.time }
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
  if (hoverIndex.value === null || !samples.value[hoverIndex.value]) return null
  return samples.value[hoverIndex.value]
})

function onMouseMove(e: MouseEvent) {
  if (!chartContainerRef.value) return
  const rect = chartContainerRef.value.getBoundingClientRect()
  const mouseX = e.clientX - rect.left
  const mouseY = e.clientY - rect.top

  mousePos.value = { x: mouseX, y: mouseY }

  const count = samples.value.length
  if (count === 0) return

  const marginX = (50 / 800) * rect.width
  const contentWidth = ((780 - 50) / 800) * rect.width
  let relX = mouseX - marginX
  if (relX < 0) relX = 0
  if (relX > contentWidth) relX = contentWidth

  const step = count > 1 ? contentWidth / (count - 1) : 0
  const idx = step > 0 ? Math.round(relX / step) : 0
  hoverIndex.value = Math.min(count - 1, Math.max(0, idx))
}

function onMouseLeave() {
  mousePos.value = null
  hoverIndex.value = null
}

const tooltipStyle = computed(() => {
  if (!mousePos.value || !chartContainerRef.value) return {}
  const { x, y } = mousePos.value
  const rect = chartContainerRef.value.getBoundingClientRect()

  return {
    ...chartTooltipStyle({
      x,
      y,
      containerWidth: rect.width,
      containerHeight: rect.height,
      maxWidth: 280,
      estimatedHeight: 65,
      offset: 12,
      padding: 4,
    }),
    zIndex: 20,
    background: '#0f172a',
    borderColor: '#334155',
  }
})
</script>

<style scoped>
.chart-box {
  position: relative;
  min-height: 180px;
  user-select: none;
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
</style>
