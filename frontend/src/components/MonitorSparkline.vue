<template>
  <v-tooltip location="top" :disabled="points.length === 0" color="#0F172A">
    <template #activator="{ props: tooltipProps }">
      <svg
        v-bind="tooltipProps"
        class="monitor-sparkline"
        :width="width"
        :height="height"
        :viewBox="`0 0 ${width} ${height}`"
        preserveAspectRatio="none"
      >
        <template v-if="points.length > 1">
          <defs>
            <linearGradient :id="gradientId" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" :stop-color="color" stop-opacity="0.35" />
              <stop offset="100%" :stop-color="color" stop-opacity="0" />
            </linearGradient>
          </defs>
          <polygon :points="areaPoints" :fill="`url(#${gradientId})`" />
          <polyline
            :points="linePoints"
            fill="none"
            :stroke="color"
            stroke-width="1.75"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
          <circle :cx="lastPoint!.x" :cy="lastPoint!.y" r="2.25" :fill="color" />
        </template>
        <template v-else-if="points.length === 1">
          <line
            x1="0"
            :y1="lastPoint!.y"
            :x2="width"
            :y2="lastPoint!.y"
            :stroke="color"
            stroke-width="1.5"
            stroke-dasharray="2,2"
            opacity="0.6"
          />
          <circle :cx="lastPoint!.x" :cy="lastPoint!.y" r="2.25" :fill="color" />
        </template>
      </svg>
    </template>
    <div v-if="points.length > 0" class="pa-1" style="font-size: 11px; color: #ffffff">
      Atual: <strong>{{ formattedCurrent }}</strong>
      <span style="color: #94a3b8"> · {{ data.length }} amostra(s)</span>
    </div>
  </v-tooltip>
</template>

<script setup lang="ts">
import { computed } from 'vue'

export interface SparklinePoint {
  value: number
  recordedAt: string
}

const props = withDefaults(
  defineProps<{
    data?: SparklinePoint[]
    color?: string
    width?: number
    height?: number
    unit?: string
  }>(),
  {
    data: () => [],
    color: '#2196F3',
    width: 90,
    height: 28,
    unit: '%',
  }
)

const gradientId = `sparkline-grad-${Math.random().toString(36).slice(2)}`

const values = computed(() => props.data.map((d) => d.value).filter((v) => !isNaN(v)))

const maxVal = computed(() => (values.value.length > 0 ? Math.max(...values.value) : 0))
const minVal = computed(() => (values.value.length > 0 ? Math.min(...values.value) : 0))

const points = computed(() => {
  const count = props.data.length
  if (count === 0) return []

  const padding = 3
  const top = padding
  const bottom = props.height - padding
  const range = maxVal.value - minVal.value

  return props.data.map((d, idx) => {
    const x = count === 1 ? props.width / 2 : (idx / (count - 1)) * props.width
    const ratio = range > 0 ? (d.value - minVal.value) / range : 0.5
    const y = bottom - ratio * (bottom - top)
    return { x, y }
  })
})

const lastPoint = computed(() =>
  points.value.length > 0 ? points.value[points.value.length - 1] : null
)

const linePoints = computed(() =>
  points.value.map((p) => `${p.x.toFixed(1)},${p.y.toFixed(1)}`).join(' ')
)

const areaPoints = computed(() => {
  if (points.value.length === 0) return ''
  const first = points.value[0]
  const last = points.value[points.value.length - 1]
  return `${first.x.toFixed(1)},${props.height} ${linePoints.value} ${last.x.toFixed(1)},${props.height}`
})

const formattedCurrent = computed(() => {
  const current = props.data[props.data.length - 1]?.value
  return current !== undefined ? `${current.toFixed(1)}${props.unit}` : '—'
})
</script>

<style scoped>
.monitor-sparkline {
  display: block;
  overflow: visible;
}
</style>
