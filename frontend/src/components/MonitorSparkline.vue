<template>
  <div
    class="monitor-sparkline-wrapper d-inline-flex align-center"
    :style="{ position: 'relative', width: cssWidth, height: `${height}px` }"
  >
    <svg
      class="monitor-sparkline-svg"
      :viewBox="`0 0 ${viewBoxWidth} ${height}`"
      preserveAspectRatio="none"
      style="display: block; width: 100%; height: 100%"
    >
      <!-- Modo Dual (ex: Entrada & Saída de Tráfego de Rede) -->
      <template v-if="isDual">
        <defs>
          <linearGradient :id="inGradientId" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" :stop-color="inColor" stop-opacity="0.32" />
            <stop offset="100%" :stop-color="inColor" stop-opacity="0.0" />
          </linearGradient>
          <linearGradient :id="outGradientId" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" :stop-color="outColor" stop-opacity="0.28" />
            <stop offset="100%" :stop-color="outColor" stop-opacity="0.0" />
          </linearGradient>
        </defs>

        <!-- Entrada (Download) -->
        <polygon
          v-if="inPoints.length > 1"
          :points="inAreaPoints"
          :fill="`url(#${inGradientId})`"
        />
        <polyline
          v-if="inPoints.length > 1"
          :points="inLinePoints"
          fill="none"
          :stroke="inColor"
          stroke-width="1.75"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
        <line
          v-else-if="inPoints.length === 1"
          x1="0"
          :y1="inPoints[0].y"
          :x2="viewBoxWidth"
          :y2="inPoints[0].y"
          :stroke="inColor"
          stroke-width="1.5"
          stroke-dasharray="2,2"
          opacity="0.6"
        />

        <!-- Saída (Upload) -->
        <polygon
          v-if="outPoints.length > 1"
          :points="outAreaPoints"
          :fill="`url(#${outGradientId})`"
        />
        <polyline
          v-if="outPoints.length > 1"
          :points="outLinePoints"
          fill="none"
          :stroke="outColor"
          stroke-width="1.75"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
        <line
          v-else-if="outPoints.length === 1"
          x1="0"
          :y1="outPoints[0].y"
          :x2="viewBoxWidth"
          :y2="outPoints[0].y"
          :stroke="outColor"
          stroke-width="1.5"
          stroke-dasharray="2,2"
          opacity="0.6"
        />
      </template>

      <!-- Modo Único (CPU, Memória, Sensores) -->
      <template v-else-if="points.length > 1">
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
          :x2="viewBoxWidth"
          :y2="points[0].y"
          :stroke="color"
          stroke-width="1.5"
          stroke-dasharray="2,2"
          opacity="0.6"
        />
      </template>

      <!-- Linha vertical guia no hover -->
      <line
        v-if="activeHoverIndex !== null && currentGuideX !== null"
        :x1="currentGuideX"
        y1="0"
        :x2="currentGuideX"
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
            <!-- Tooltip Modo Dual -->
            <template v-if="isDual">
              <div class="d-flex align-center ga-2 mb-1">
                <span class="status-indicator-dot" :style="{ backgroundColor: inColor }"></span>
                <span style="font-size: 12px; color: #4ade80" class="font-weight-medium"
                  >↓ Entrada:</span
                >
                <span style="font-size: 12px; color: #ffffff" class="font-weight-bold">
                  {{ formatPointValue(col.inVal) }}
                </span>
              </div>
              <div class="d-flex align-center ga-2 mb-1">
                <span class="status-indicator-dot" :style="{ backgroundColor: outColor }"></span>
                <span style="font-size: 12px; color: #38bdf8" class="font-weight-medium"
                  >↑ Saída:</span
                >
                <span style="font-size: 12px; color: #ffffff" class="font-weight-bold">
                  {{ formatPointValue(col.outVal) }}
                </span>
              </div>
              <div
                v-if="col.recordedAt"
                style="font-size: 11px; color: #cbd5e1"
                class="d-flex align-center ga-1"
              >
                <v-icon size="12" color="#94a3b8">mdi-clock-outline</v-icon>
                <span>Data e Hora: {{ formatShortDateTime(col.recordedAt) }}</span>
              </div>
              <div style="font-size: 10px; color: #64748b" class="mt-1">
                Amostra {{ col.idx + 1 }} de {{ columns.length }}
              </div>
            </template>

            <!-- Tooltip Modo Único -->
            <template v-else>
              <div class="d-flex align-center ga-2 mb-1">
                <span class="status-indicator-dot" :style="{ backgroundColor: color }"></span>
                <span style="font-size: 13px; color: #38bdf8" class="font-weight-bold">Valor:</span>
                <span style="font-size: 13px; color: #ffffff" class="font-weight-bold">
                  {{ formatPointValue(col.pt?.value) }}
                </span>
              </div>
              <div
                v-if="col.pt?.recordedAt"
                style="font-size: 11px; color: #cbd5e1"
                class="d-flex align-center ga-1"
              >
                <v-icon size="12" color="#94a3b8">mdi-clock-outline</v-icon>
                <span>Data e Hora: {{ formatShortDateTime(col.pt.recordedAt) }}</span>
              </div>
              <div style="font-size: 10px; color: #64748b" class="mt-1">
                Amostra {{ col.idx + 1 }} de {{ points.length }}
              </div>
            </template>
          </div>
        </v-tooltip>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { formatBps, formatBytes, formatLatency, formatShortDateTime } from '@/utils/formatters'

export interface SparklinePoint {
  value: number
  recordedAt: string
}

const props = withDefaults(
  defineProps<{
    data?: SparklinePoint[]
    inData?: SparklinePoint[]
    outData?: SparklinePoint[]
    color?: string
    inColor?: string
    outColor?: string
    width?: number | string
    height?: number
    unit?: string
    formatValue?: (val: number) => string
  }>(),
  {
    data: () => [],
    inData: () => [],
    outData: () => [],
    color: '#2196F3',
    inColor: '#4ade80',
    outColor: '#38bdf8',
    width: 90,
    height: 28,
    unit: '%',
    formatValue: undefined,
  }
)

const activeHoverIndex = ref<number | null>(null)

const gradientId = `sparkline-grad-${Math.random().toString(36).slice(2)}`
const inGradientId = `sparkline-in-grad-${Math.random().toString(36).slice(2)}`
const outGradientId = `sparkline-out-grad-${Math.random().toString(36).slice(2)}`

const isDual = computed(() => {
  const hasIn = Boolean(props.inData && props.inData.length > 0)
  const hasOut = Boolean(props.outData && props.outData.length > 0)
  return hasIn || hasOut
})

const viewBoxWidth = computed(() => {
  if (typeof props.width === 'number') return props.width
  const parsed = parseFloat(String(props.width))
  return isNaN(parsed) || parsed <= 0 ? 100 : parsed
})

const cssWidth = computed(() => {
  if (typeof props.width === 'number') return `${props.width}px`
  return props.width || '100%'
})

// --- Lógica do Modo Único ---
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
  const vw = viewBoxWidth.value

  return props.data.map((d, idx) => {
    const x = count === 1 ? vw / 2 : (idx / (count - 1)) * vw
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

const singleColumns = computed(() => {
  const count = points.value.length
  if (count === 0) return []
  const vw = viewBoxWidth.value

  return points.value.map((pt, idx) => {
    let leftX = 0
    let rightX = vw

    if (count > 1) {
      if (idx === 0) {
        leftX = 0
        rightX = (pt.x + points.value[1].x) / 2
      } else if (idx === count - 1) {
        leftX = (pt.x + points.value[idx - 1].x) / 2
        rightX = vw
      } else {
        leftX = (pt.x + points.value[idx - 1].x) / 2
        rightX = (pt.x + points.value[idx + 1].x) / 2
      }
    }

    const colWidth = Math.max(0.001, rightX - leftX)
    return {
      pt,
      idx,
      x: pt.x,
      flexRatio: colWidth / vw,
      inVal: undefined,
      outVal: undefined,
      recordedAt: pt.recordedAt,
    }
  })
})

// --- Lógica do Modo Dual (Tráfego de Rede) ---
const dualMaxVal = computed(() => {
  const inVals = (props.inData || []).map((d) => d.value).filter((v) => !isNaN(v))
  const outVals = (props.outData || []).map((d) => d.value).filter((v) => !isNaN(v))
  const all = [...inVals, ...outVals]
  return all.length > 0 ? Math.max(...all, 0) : 0
})

function buildPoints(series: SparklinePoint[], max: number, min = 0) {
  const count = series.length
  if (count === 0) return []
  const padding = 3
  const top = padding
  const bottom = props.height - padding
  const range = max - min
  const vw = viewBoxWidth.value

  return series.map((d, idx) => {
    const x = count === 1 ? vw / 2 : (idx / (count - 1)) * vw
    const ratio = range > 0 ? (d.value - min) / range : 0
    const y = bottom - ratio * (bottom - top)
    return { x, y, value: d.value, recordedAt: d.recordedAt }
  })
}

const inPoints = computed(() =>
  isDual.value ? buildPoints(props.inData || [], dualMaxVal.value, 0) : []
)

const outPoints = computed(() =>
  isDual.value ? buildPoints(props.outData || [], dualMaxVal.value, 0) : []
)

function toLinePoints(pts: Array<{ x: number; y: number }>) {
  return pts.map((p) => `${p.x.toFixed(1)},${p.y.toFixed(1)}`).join(' ')
}

function toAreaPoints(pts: Array<{ x: number; y: number }>, lineStr: string, h: number) {
  if (pts.length === 0) return ''
  const first = pts[0]
  const last = pts[pts.length - 1]
  return `${first.x.toFixed(1)},${h} ${lineStr} ${last.x.toFixed(1)},${h}`
}

const inLinePoints = computed(() => toLinePoints(inPoints.value))
const inAreaPoints = computed(() => toAreaPoints(inPoints.value, inLinePoints.value, props.height))

const outLinePoints = computed(() => toLinePoints(outPoints.value))
const outAreaPoints = computed(() =>
  toAreaPoints(outPoints.value, outLinePoints.value, props.height)
)

const dualColumns = computed(() => {
  const inArr = props.inData || []
  const outArr = props.outData || []
  const count = Math.max(inArr.length, outArr.length)
  if (count === 0) return []
  const vw = viewBoxWidth.value

  return Array.from({ length: count }, (_, idx) => {
    const x = count === 1 ? vw / 2 : (idx / (count - 1)) * vw
    let leftX = 0
    let rightX = vw
    if (count > 1) {
      const step = vw / (count - 1)
      leftX = Math.max(0, x - step / 2)
      rightX = Math.min(vw, x + step / 2)
      if (idx === 0) leftX = 0
      if (idx === count - 1) rightX = vw
    }
    const colWidth = Math.max(0.001, rightX - leftX)
    const inPt = inArr[idx]
    const outPt = outArr[idx]
    const recordedAt = inPt?.recordedAt || outPt?.recordedAt

    return {
      pt: inPt || outPt,
      idx,
      x,
      flexRatio: colWidth / vw,
      inVal: inPt?.value,
      outVal: outPt?.value,
      recordedAt,
    }
  })
})

const columns = computed(() => (isDual.value ? dualColumns.value : singleColumns.value))

const currentGuideX = computed(() => {
  if (activeHoverIndex.value === null) return null
  if (isDual.value) {
    return dualColumns.value[activeHoverIndex.value]?.x ?? null
  }
  return points.value[activeHoverIndex.value]?.x ?? null
})

function formatPointValue(val?: number): string {
  if (val === undefined || val === null || isNaN(val)) return '—'
  if (props.formatValue) {
    return props.formatValue(val)
  }
  const u = (props.unit || '%').trim()
  if (u.toLowerCase() === 'bps' || u.toLowerCase() === 'bit/s' || u.toLowerCase() === 'bits/s') {
    return formatBps(val)
  }
  if (u.toLowerCase() === 'bytes' || u.toLowerCase() === 'b') {
    return formatBytes(val)
  }
  if (u.toLowerCase() === 'ms') {
    return formatLatency(val)
  }
  const numeric = Number(val.toFixed(1))
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
