<template>
  <div
    class="monitor-sparkline-wrapper d-inline-flex align-center"
    :style="{ position: 'relative', width: `${width}px`, height: `${height}px` }"
  >
    <svg
      class="monitor-sparkline-svg"
      :width="width"
      :height="height"
      :viewBox="`0 0 ${width} ${height}`"
      preserveAspectRatio="none"
      style="display: block; width: 100%; height: 100%"
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
      </template>
      <template v-else-if="points.length === 1">
        <line
          x1="0"
          :y1="points[0].y"
          :x2="width"
          :y2="points[0].y"
          :stroke="color"
          stroke-width="1.5"
          stroke-dasharray="2,2"
          opacity="0.6"
        />
      </template>

      <!-- Linha vertical guia no hover -->
      <line
        v-if="activeHoverIndex !== null && points[activeHoverIndex]"
        :x1="points[activeHoverIndex].x"
        y1="0"
        :x2="points[activeHoverIndex].x"
        :y2="height"
        stroke="#38BDF8"
        stroke-dasharray="2,2"
        stroke-width="1"
        opacity="0.75"
      />
    </svg>

    <!-- Overlay HTML com fatias por ponto de amostragem ativando o v-tooltip -->
    <div
      class="sparkline-hit-overlay"
      style="
        position: absolute;
        top: 0;
        left: 0;
        right: 0;
        bottom: 0;
        display: flex;
        width: 100%;
        height: 100%;
      "
    >
      <div
        v-for="col in columns"
        :key="col.idx"
        class="sparkline-hit-col"
        :style="{ flex: `${col.flexRatio}`, height: '100%' }"
      >
        <v-tooltip location="top" color="#0F172A" :open-delay="30" :close-delay="30" offset="6">
          <template #activator="{ props: tooltipProps }">
            <div
              v-bind="tooltipProps"
              style="width: 100%; height: 100%; cursor: pointer"
              @mouseenter="activeHoverIndex = col.idx"
              @mouseleave="activeHoverIndex = null"
            ></div>
          </template>
          <div class="custom-tooltip-content pa-2">
            <div class="d-flex align-center ga-2 mb-1">
              <span class="status-indicator-dot" :style="{ backgroundColor: color }"></span>
              <span style="font-size: 13px; color: #38bdf8" class="font-weight-bold">Valor:</span>
              <span style="font-size: 13px; color: #ffffff" class="font-weight-bold">
                {{ formatPointValue(col.pt.value) }}
              </span>
            </div>
            <div
              v-if="col.pt.recordedAt"
              style="font-size: 11px; color: #cbd5e1"
              class="d-flex align-center ga-1"
            >
              <v-icon size="12" color="#94a3b8">mdi-clock-outline</v-icon>
              <span>Data e Hora: {{ formatShortDateTime(col.pt.recordedAt) }}</span>
            </div>
            <div style="font-size: 10px; color: #64748b" class="mt-1">
              Amostra {{ col.idx + 1 }} de {{ points.length }}
            </div>
          </div>
        </v-tooltip>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { formatShortDateTime } from '@/utils/formatters'

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

const activeHoverIndex = ref<number | null>(null)

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
    return { x, y, value: d.value, recordedAt: d.recordedAt }
  })
})

const linePoints = computed(() =>
  points.value.map((p) => `${p.x.toFixed(1)},${p.y.toFixed(1)}`).join(' ')
)

const areaPoints = computed(() => {
  if (points.value.length === 0) return ''
  const first = points.value[0]
  const last = points.value[points.value.length - 1]
  return `${first.x.toFixed(1)},${props.height} ${linePoints.value} ${last.x.toFixed(1)},${props.height}`
})

const columns = computed(() => {
  const count = points.value.length
  if (count === 0) return []

  return points.value.map((pt, idx) => {
    let leftX = 0
    let rightX = props.width

    if (count > 1) {
      if (idx === 0) {
        leftX = 0
        rightX = (pt.x + points.value[1].x) / 2
      } else if (idx === count - 1) {
        leftX = (pt.x + points.value[idx - 1].x) / 2
        rightX = props.width
      } else {
        leftX = (pt.x + points.value[idx - 1].x) / 2
        rightX = (pt.x + points.value[idx + 1].x) / 2
      }
    }

    const colWidth = Math.max(0.001, rightX - leftX)

    return {
      pt,
      idx,
      flexRatio: colWidth / props.width,
    }
  })
})

function formatPointValue(val?: number): string {
  if (val === undefined || val === null || isNaN(val)) return '—'
  const numeric = Number(val.toFixed(1))
  const u = props.unit || '%'
  return u.startsWith('%') ? `${numeric}${u}` : `${numeric} ${u}`
}
</script>

<style scoped>
.monitor-sparkline-wrapper {
  user-select: none;
}

.status-indicator-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  display: inline-block;
}

.custom-tooltip-content {
  pointer-events: none;
  max-width: 260px;
}
</style>
