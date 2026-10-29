<template>
  <div
    ref="wrapperRef"
    class="sparkline"
    :style="{ width: cssWidth, height: `${height}px` }"
    @mousemove="onPointer"
    @mouseleave="hoverIndex = null"
  >
    <svg
      v-if="pixelWidth > 0 && count > 0"
      class="sparkline__svg"
      :width="pixelWidth"
      :height="height"
      :viewBox="`0 0 ${pixelWidth} ${height}`"
    >
      <defs>
        <linearGradient
          v-for="s in drawn"
          :id="s.gradientId"
          :key="s.gradientId"
          x1="0"
          y1="0"
          x2="0"
          y2="1"
        >
          <stop offset="0%" :stop-color="s.color" stop-opacity="0.25" />
          <stop offset="100%" :stop-color="s.color" stop-opacity="0" />
        </linearGradient>
      </defs>
      <template v-for="s in drawn" :key="s.id">
        <path v-if="s.area" :d="s.area" :fill="`url(#${s.gradientId})`" />
        <path :d="s.line" :stroke="s.color" class="sparkline__line" />
      </template>
      <template v-if="hover">
        <line :x1="hover.x" :x2="hover.x" y1="0" :y2="height" class="sparkline__crosshair" />
        <circle
          v-for="dot in hover.dots"
          :key="dot.id"
          :cx="hover.x"
          :cy="dot.y"
          r="2.5"
          :fill="dot.color"
          class="sparkline__dot"
        />
      </template>
    </svg>

    <!-- Um tooltip só, teleportado: escapa de tabelas e cards com overflow -->
    <v-tooltip
      :model-value="hover !== null"
      :activator="wrapperRef ?? undefined"
      location="top"
      :open-on-hover="false"
      content-class="sparkline__tooltip"
    >
      <template v-if="hover">
        <div v-for="row in hover.rows" :key="row.label" class="sparkline__row">
          <span class="sparkline__swatch" :style="{ backgroundColor: row.color }"></span>
          <span>{{ row.label }}</span>
          <strong>{{ row.value }}</strong>
        </div>
        <div v-if="hover.time" class="sparkline__time">
          {{ formatShortDateTime(hover.time) }} · amostra {{ hover.index + 1 }} de {{ count }}
        </div>
      </template>
    </v-tooltip>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useElementWidth } from '@/composables/useElementWidth'
import { areaPath, linePath, nearestIndex, xAt, yAt, type PlotArea } from '@/utils/charts/scale'
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
    /** Nome das séries no modo duplo (padrão: tráfego de interface). */
    inLabel?: string
    outLabel?: string
  }>(),
  {
    data: () => [],
    inData: () => [],
    outData: () => [],
    color: '#3987e5',
    inColor: '#199e70',
    outColor: '#3987e5',
    width: 90,
    height: 28,
    unit: '%',
    formatValue: undefined,
    inLabel: 'Entrada',
    outLabel: 'Saída',
  }
)

const wrapperRef = ref<HTMLElement | null>(null)
const measured = useElementWidth(wrapperRef)
const hoverIndex = ref<number | null>(null)
const uid = Math.random().toString(36).slice(2, 8)

const cssWidth = computed(() =>
  typeof props.width === 'number' ? `${props.width}px` : props.width || '100%'
)
const pixelWidth = computed(() =>
  measured.value > 0 ? measured.value : typeof props.width === 'number' ? props.width : 0
)

const isDual = computed(() => props.inData.length > 0 || props.outData.length > 0)

/** Séries a desenhar: uma no modo único; entrada e saída no modo duplo. */
const seriesList = computed(() =>
  isDual.value
    ? [
        { id: 'in', label: `↓ ${props.inLabel}`, color: props.inColor, data: props.inData },
        { id: 'out', label: `↑ ${props.outLabel}`, color: props.outColor, data: props.outData },
      ]
    : [{ id: 'value', label: 'Valor', color: props.color, data: props.data }]
)

const count = computed(() => Math.max(0, ...seriesList.value.map((s) => s.data.length)))

const area = computed<PlotArea>(() => ({
  left: 1,
  top: 3,
  width: Math.max(1, pixelWidth.value - 2),
  height: Math.max(1, props.height - 6),
}))

/**
 * Modo único: a escala acompanha a variação (min→max), que é o que o olho
 * procura numa miniatura. Modo duplo: zero na base, para entrada e saída
 * serem comparáveis.
 */
const domain = computed(() => {
  const values = seriesList.value.flatMap((s) => s.data.map((d) => d.value)).filter(Number.isFinite)
  if (values.length === 0) return { min: 0, max: 1 }
  const max = Math.max(...values)
  const min = isDual.value ? 0 : Math.min(...values)
  if (max === min) return { min: min - 1, max: max + 1 }
  return { min, max }
})

const drawn = computed(() =>
  seriesList.value.map((s) => {
    const points = s.data.map((d, i) => ({
      x: xAt(i, count.value, area.value),
      y: yAt(d.value, domain.value, area.value),
    }))
    return {
      id: s.id,
      color: s.color,
      gradientId: `sp-${uid}-${s.id}`,
      points,
      line:
        points.length === 1
          ? `M${area.value.left} ${points[0].y}H${area.value.left + area.value.width}`
          : linePath(points),
      area: points.length > 1 ? areaPath(points, area.value.top + area.value.height) : '',
    }
  })
)

function formatPointValue(val: number): string {
  if (props.formatValue) return props.formatValue(val)
  const u = (props.unit || '%').trim().toLowerCase()
  if (u === 'bps' || u === 'bit/s' || u === 'bits/s') return formatBps(val)
  if (u === 'bytes' || u === 'b') return formatBytes(val)
  if (u === 'ms') return formatLatency(val)
  const numeric = Number(val.toFixed(1))
  return props.unit.startsWith('%') ? `${numeric}${props.unit}` : `${numeric} ${props.unit}`
}

const hover = computed(() => {
  const index = hoverIndex.value
  if (index === null || count.value === 0) return null
  const rows = seriesList.value.flatMap((s) => {
    const datum = s.data[index]
    return datum ? [{ label: s.label, color: s.color, value: formatPointValue(datum.value) }] : []
  })
  const dots = drawn.value.flatMap((s) =>
    s.points[index] ? [{ id: s.id, y: s.points[index].y, color: s.color }] : []
  )
  const time = seriesList.value.map((s) => s.data[index]?.recordedAt).find(Boolean) ?? ''
  return { index, x: xAt(index, count.value, area.value), rows, dots, time }
})

function onPointer(e: MouseEvent) {
  const el = wrapperRef.value
  if (!el || count.value === 0) return
  const rect = el.getBoundingClientRect()
  hoverIndex.value = nearestIndex(e.clientX - rect.left, count.value, area.value)
}
</script>

<style scoped>
.sparkline {
  position: relative;
  display: inline-block;
  user-select: none;
  cursor: crosshair;
}

.sparkline__svg {
  display: block;
  overflow: visible;
}

.sparkline__line {
  fill: none;
  stroke-width: 1.25;
  stroke-linecap: round;
  stroke-linejoin: round;
  vector-effect: non-scaling-stroke;
}

.sparkline__crosshair {
  stroke: rgba(var(--v-theme-on-surface), 0.45);
  stroke-width: 1;
  stroke-dasharray: 2 2;
  shape-rendering: crispEdges;
}

.sparkline__dot {
  stroke: rgb(var(--v-theme-surface));
  stroke-width: 1;
}
</style>

<style>
/* Conteúdo teleportado do v-tooltip: fora do escopo do componente. */
.sparkline__tooltip {
  background: #0f172a !important;
  color: #f8fafc !important;
  font-size: 12px;
  padding: 6px 10px !important;
}

.sparkline__tooltip .sparkline__row {
  display: flex;
  align-items: center;
  gap: 8px;
  white-space: nowrap;
}

.sparkline__tooltip .sparkline__swatch {
  width: 8px;
  height: 8px;
  border-radius: 50%;
}

.sparkline__tooltip .sparkline__row strong {
  margin-left: auto;
  padding-left: 10px;
  font-variant-numeric: tabular-nums;
}

.sparkline__tooltip .sparkline__time {
  margin-top: 2px;
  color: #94a3b8;
  font-size: 11px;
}
</style>
