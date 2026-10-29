<template>
  <DashboardCard
    :title="widget.title || config.defaultTitle"
    :icon="config.icon"
    :icon-color="statusColor"
    :subtitle="`Origem: ${targetDescription}`"
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
        class="resource-widget__select"
        placeholder="Equipamento"
        @update:model-value="onDeviceChange"
      ></v-select>
      <TimeframeToggle v-model="timeframe" />
    </template>

    <!-- Destaques: atual, pico, média e o quarto número de cada recurso -->
    <v-row dense class="mb-2">
      <v-col cols="6" sm="3">
        <div class="resource-widget__kpi">
          <div class="resource-widget__kpi-label">Atual</div>
          <div class="text-h6 font-weight-bold" :class="`text-${statusColor}`">
            {{ formatUsageValue(currentUsage) }}
          </div>
          <div
            v-if="resourceType === 'ram' && currentUsagePercent !== null"
            class="resource-widget__kpi-label"
          >
            {{ formatPercent(currentUsagePercent) }} do total
          </div>
        </div>
      </v-col>
      <v-col cols="6" sm="3">
        <div class="resource-widget__kpi">
          <div class="resource-widget__kpi-label">Pico</div>
          <div class="text-h6 font-weight-bold text-error">{{ formatUsageValue(peakUsage) }}</div>
        </div>
      </v-col>
      <v-col cols="6" sm="3">
        <div class="resource-widget__kpi">
          <div class="resource-widget__kpi-label">Média</div>
          <div class="text-h6 font-weight-bold text-info">{{ formatUsageValue(avgUsage) }}</div>
        </div>
      </v-col>
      <v-col cols="6" sm="3">
        <div class="resource-widget__kpi">
          <div class="resource-widget__kpi-label">{{ config.fourthCardLabel }}</div>
          <div v-if="resourceType === 'cpu'" class="text-h6 font-weight-bold text-secondary">
            {{ cpuLoad === null ? '—' : cpuLoad.toFixed(2) }}
          </div>
          <div v-else class="text-subtitle-1 font-weight-bold text-primary">
            {{ memoryAllocation }}
          </div>
        </div>
      </v-col>
    </v-row>

    <BaseMetricChart
      :series="chartSeries"
      :unit-type="resourceType === 'ram' ? 'bytes' : 'percentage'"
      :format-value="formatUsageValue"
      :thresholds="chartThresholds"
      :min-ceiling="chartCeiling"
      :height="200"
      :empty-text="config.emptyText"
      :aria-label="config.label"
    />
  </DashboardCard>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { useDevicesStore } from '@/stores/devices'
import { useDeviceDetailStore } from '@/stores/deviceDetail'
import { useEventsStore } from '@/stores/events'
import type { WidgetConfig } from '@/stores/dashboard'
import { formatBinaryBytes, formatPercent } from '@/utils/formatters'
import { CHART_STATUS_COLORS, paletteColor } from '@/utils/charts/scale'
import BaseMetricChart, {
  type ChartReferenceLine,
  type ChartSeriesInput,
} from '@/components/BaseMetricChart.vue'
import DashboardCard from '@/components/dashboard/DashboardCard.vue'
import TimeframeToggle from '@/components/dashboard/TimeframeToggle.vue'
import { useWidgetTimeframe } from '@/composables/useWidgetTimeframe'
import { latestMetricValue, resourceMetricWindow, RESOURCE_SERIES } from '@/utils/resourceMetrics'

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
  },
  ram: {
    icon: 'mdi-memory',
    emptyIcon: 'mdi-chip',
    defaultTitle: 'Uso de RAM',
    label: 'Uso de RAM',
    emptyText: 'Sem dados de uso de RAM no período',
    fourthCardLabel: 'Alocação',
    metricNames: RESOURCE_SERIES.ram,
  },
} as const

const config = computed(() => CONFIGS[resourceType.value])

const devicesStore = useDevicesStore()
const deviceDetailStore = useDeviceDetailStore()
const eventsStore = useEventsStore()

const timeframe = useWidgetTimeframe(() => props.widget, '15m')
const configuredDeviceId = Number(props.widget.config?.deviceId)
const selectedDeviceId = ref<number | null>(
  Number.isInteger(configuredDeviceId) && configuredDeviceId > 0 ? configuredDeviceId : null
)

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
    : formatPercent(value)
}

/** Limiares em % (o layout pode trocar os padrões 70/85). */
const warningPercent = computed(() => {
  const value = Number(props.widget.config?.warningThreshold)
  return Number.isFinite(value) && value > 0 ? value : 70
})
const criticalPercent = computed(() => {
  const value = Number(props.widget.config?.criticalThreshold)
  return Number.isFinite(value) && value > 0 ? value : 85
})

const statusColor = computed(() => {
  const usage = currentUsagePercent.value ?? 0
  if (usage >= criticalPercent.value) return 'error'
  if (usage >= warningPercent.value) return 'warning'
  return 'success'
})

/** Converte % no valor do eixo: RAM é desenhada em bytes, CPU em %. */
function percentToAxis(percent: number): number | null {
  if (resourceType.value === 'cpu') return percent
  if (totalRamBytes.value === null || totalRamBytes.value <= 0) return null
  return (percent / 100) * totalRamBytes.value
}

const chartThresholds = computed<ChartReferenceLine[]>(() => {
  const warning = percentToAxis(warningPercent.value)
  const critical = percentToAxis(criticalPercent.value)
  if (warning === null || critical === null) return []
  return [
    {
      value: warning,
      label: `Alerta (${formatPercent(warningPercent.value, 0)})`,
      color: CHART_STATUS_COLORS.warning,
    },
    {
      value: critical,
      label: `Crítico (${formatPercent(criticalPercent.value, 0)})`,
      color: CHART_STATUS_COLORS.critical,
    },
  ]
})

/** RAM com total conhecido: o eixo vai até a capacidade, não até o pico. */
const chartCeiling = computed(() =>
  resourceType.value === 'ram' && totalRamBytes.value ? totalRamBytes.value : 0
)

const chartSeries = computed<ChartSeriesInput[]>(() => [
  {
    id: resourceType.value,
    label: config.value.label,
    color: paletteColor(0),
    fillArea: true,
    data: samples.value.map((s) => ({
      time: new Date(s.timestamp).toISOString(),
      value: s.value,
    })),
  },
])
</script>

<style scoped>
.resource-widget__select {
  min-width: 180px;
  max-width: 220px;
}

.resource-widget__kpi {
  padding: 8px;
  border-radius: 8px;
  text-align: center;
  background: rgba(var(--v-theme-primary), 0.06);
}

.resource-widget__kpi-label {
  font-size: 0.75rem;
  color: rgba(var(--v-theme-on-surface), 0.8);
}
</style>
