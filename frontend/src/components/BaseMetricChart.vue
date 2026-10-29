<template>
  <div class="metric-chart">
    <div
      ref="plotRef"
      class="metric-chart__plot"
      :class="{ 'metric-chart__plot--clickable': clickable }"
      :style="{ height: `${height}px` }"
      role="img"
      :aria-label="ariaLabel"
      @mousemove="onPointer"
      @mouseleave="hoverIndex = null"
      @touchmove.passive="onTouch"
      @touchend="hoverIndex = null"
      @click="onClick"
    >
      <svg
        v-if="hasData && width > 0"
        class="metric-chart__svg"
        :width="width"
        :height="height"
        :viewBox="`0 0 ${width} ${height}`"
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
            <stop offset="0%" :stop-color="s.color" stop-opacity="0.22" />
            <stop offset="100%" :stop-color="s.color" stop-opacity="0" />
          </linearGradient>
        </defs>

        <!-- Grade horizontal e rótulos do eixo -->
        <g v-for="tick in leftTicks" :key="`l-${tick.y}`">
          <line
            :x1="area.left"
            :x2="area.left + area.width"
            :y1="tick.y"
            :y2="tick.y"
            class="metric-chart__grid"
            :class="{ 'metric-chart__grid--base': tick.base }"
          />
          <text :x="area.left - 8" :y="tick.y" class="metric-chart__label" text-anchor="end">
            {{ tick.label }}
          </text>
        </g>
        <text
          v-for="tick in rightTicks"
          :key="`r-${tick.y}`"
          :x="area.left + area.width + 8"
          :y="tick.y"
          class="metric-chart__label"
          text-anchor="start"
        >
          {{ tick.label }}
        </text>

        <!-- Eixo de tempo -->
        <text
          v-for="tick in timeTicks"
          :key="`t-${tick.x}`"
          :x="tick.x"
          :y="height - 4"
          class="metric-chart__label"
          :text-anchor="tick.anchor"
        >
          {{ tick.label }}
        </text>

        <!-- Limiares (aviso/crítico) e média -->
        <g v-for="line in referenceLines" :key="`ref-${line.label}`">
          <line
            :x1="area.left"
            :x2="area.left + area.width"
            :y1="line.y"
            :y2="line.y"
            :stroke="line.color"
            class="metric-chart__reference"
          />
        </g>

        <!-- Áreas, depois linhas: a linha nunca fica sob o preenchimento de outra série -->
        <path
          v-for="s in drawn"
          v-show="s.fillArea"
          :key="`a-${s.id}`"
          :d="s.area"
          :fill="`url(#${s.gradientId})`"
        />
        <path
          v-for="s in drawn"
          :key="`l-${s.id}`"
          :d="s.line"
          :stroke="s.color"
          class="metric-chart__line"
          :class="{ 'metric-chart__line--dashed': s.dashed }"
        />

        <!-- Amostras destacadas pela própria série (ex.: perda de pacote) -->
        <template v-for="s in drawn" :key="`m-${s.id}`">
          <circle
            v-for="marker in s.markers"
            :key="marker.index"
            :cx="marker.x"
            :cy="marker.y"
            r="3"
            :fill="marker.color"
            class="metric-chart__marker"
          />
        </template>

        <!-- Cursor -->
        <template v-if="hover">
          <line
            :x1="hover.x"
            :x2="hover.x"
            :y1="area.top"
            :y2="area.top + area.height"
            class="metric-chart__crosshair"
          />
          <circle
            v-for="dot in hover.dots"
            :key="dot.id"
            :cx="hover.x"
            :cy="dot.y"
            r="4"
            :fill="dot.color"
            class="metric-chart__marker"
          />
        </template>
      </svg>

      <div v-else-if="!hasData" class="metric-chart__empty">
        <v-icon size="32" color="primary">mdi-chart-line-variant</v-icon>
        <div class="text-body-2 mt-1">{{ emptyText }}</div>
      </div>

      <ChartTooltipCard
        v-if="hover && tooltipStyle"
        :rows="hover.rows"
        :footer="hover.footer"
        :position-style="tooltipStyle"
      />
    </div>

    <!-- Legenda: a identidade da série nunca depende só da cor -->
    <div v-if="legendVisible" class="metric-chart__legend">
      <span v-for="s in drawn" :key="`lg-${s.id}`" class="metric-chart__legend-item">
        <span
          class="metric-chart__legend-swatch"
          :class="{ 'metric-chart__legend-swatch--dashed': s.dashed }"
          :style="{ borderColor: s.color }"
        ></span>
        {{ s.label }}
        <strong v-if="s.latestLabel">{{ s.latestLabel }}</strong>
      </span>
      <span
        v-for="line in referenceLines"
        :key="`lgr-${line.label}`"
        class="metric-chart__legend-item"
      >
        <span
          class="metric-chart__legend-swatch metric-chart__legend-swatch--dashed"
          :style="{ borderColor: line.color }"
        ></span>
        {{ line.label }}
      </span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import ChartTooltipCard, { type ChartTooltipRow } from '@/components/charts/ChartTooltipCard.vue'
import { useElementWidth } from '@/composables/useElementWidth'
import { chartTooltipStyle } from '@/utils/chartTooltip'
import {
  CHART_STATUS_COLORS,
  areaPath,
  axisTicks,
  linePath,
  nearestIndex,
  xAt,
  yAt,
  yDomain,
  type PlotArea,
} from '@/utils/charts/scale'
import {
  formatAxisTime,
  formatBps,
  formatBytes,
  formatLatency,
  formatPercent,
  formatShortDateTime,
} from '@/utils/formatters'

export interface ChartDataPoint {
  time: string
  /** `null` marca falta de amostra: a linha quebra ali. */
  value: number | null
  formattedValue?: string
  status?: string
  /** Cor própria destaca a amostra com um marcador (ex.: perda de pacote). */
  color?: string
}

export interface ChartSeriesInput {
  id: string
  label: string
  color: string
  fillArea?: boolean
  dashed?: boolean
  /** Série no eixo da direita (gráficos de duas unidades). */
  axis?: 'left' | 'right'
  data: ChartDataPoint[]
}

export interface ChartReferenceLine {
  value: number
  label: string
  color?: string
}

type UnitType = 'bandwidth' | 'bytes' | 'latency' | 'percentage' | 'generic' | 'boolean' | 'state'

const props = withDefaults(
  defineProps<{
    series: ChartSeriesInput[]
    unitType?: UnitType
    customUnit?: string
    formatValue?: (val: number) => string
    /** Unidade e formato do eixo da direita. */
    rightUnitType?: UnitType
    rightFormatValue?: (val: number) => string
    showAvgLine?: boolean
    avgValue?: number
    thresholds?: ChartReferenceLine[]
    height?: number
    curve?: 'linear' | 'step'
    /** Teto mínimo do eixo esquerdo. */
    minCeiling?: number
    showLegend?: boolean
    clickable?: boolean
    emptyText?: string
    ariaLabel?: string
  }>(),
  {
    unitType: 'generic',
    customUnit: '',
    formatValue: undefined,
    rightUnitType: 'generic',
    rightFormatValue: undefined,
    showAvgLine: false,
    avgValue: undefined,
    thresholds: () => [],
    height: 220,
    curve: undefined,
    minCeiling: 0,
    showLegend: undefined,
    clickable: false,
    emptyText: 'Nenhum dado disponível para o gráfico.',
    ariaLabel: 'Gráfico de série temporal',
  }
)

const emit = defineEmits<{ (e: 'point-click', index: number): void }>()

const plotRef = ref<HTMLElement | null>(null)
const width = useElementWidth(plotRef)
const hoverIndex = ref<number | null>(null)
const pointer = ref<{ x: number; y: number } | null>(null)
const uid = Math.random().toString(36).slice(2, 8)

const leftSeries = computed(() => props.series.filter((s) => s.axis !== 'right'))
const rightSeries = computed(() => props.series.filter((s) => s.axis === 'right'))
const count = computed(() => Math.max(0, ...props.series.map((s) => s.data.length)))
const hasData = computed(() => count.value > 0)

const resolvedCurve = computed(
  () =>
    props.curve ?? (props.unitType === 'boolean' || props.unitType === 'state' ? 'step' : 'linear')
)

function formatterFor(unit: UnitType, custom?: (v: number) => string) {
  return (val: number): string => {
    if (custom) return custom(val)
    if (unit === 'bandwidth') return formatBps(val)
    if (unit === 'bytes') return formatBytes(val)
    if (unit === 'latency') return formatLatency(val)
    if (unit === 'percentage') return formatPercent(val, val >= 10 ? 0 : 1)
    if (unit === 'boolean') return val >= 0.5 ? 'Ligado' : 'Desligado'
    const rounded = Number.isInteger(val) ? String(val) : val.toFixed(1)
    return props.customUnit ? `${rounded} ${props.customUnit}` : rounded
  }
}

const formatLeft = computed(() => formatterFor(props.unitType, props.formatValue))
const formatRight = computed(() => formatterFor(props.rightUnitType, props.rightFormatValue))

function valuesOf(list: ChartSeriesInput[]): number[] {
  return list.flatMap((s) => s.data.map((d) => d.value).filter((v): v is number => v !== null))
}

function domainFor(list: ChartSeriesInput[], unit: UnitType, extra: number[] = []) {
  if (unit === 'boolean') return { min: 0, max: 1 }
  return yDomain([...valuesOf(list), ...extra], {
    minCeiling: unit === 'percentage' ? 100 : props.minCeiling,
    base: unit === 'bytes' ? 1024 : 10,
  })
}

const referenceLines = computed(() => {
  const lines = props.thresholds.map((t, i) => ({
    ...t,
    color: t.color ?? (i === 0 ? CHART_STATUS_COLORS.warning : CHART_STATUS_COLORS.critical),
  }))
  if (props.showAvgLine && props.avgValue !== undefined && props.avgValue !== null) {
    lines.push({ value: props.avgValue, label: 'Média', color: CHART_STATUS_COLORS.warning })
  }
  return lines.map((line) => ({ ...line, y: yAt(line.value, leftDomain.value, area.value) }))
})

const leftDomain = computed(() =>
  domainFor(
    leftSeries.value,
    props.unitType,
    props.thresholds.map((t) => t.value)
  )
)
const rightDomain = computed(() => domainFor(rightSeries.value, props.rightUnitType))

/** Margens fixas em pixels: espaço para os rótulos, nunca proporcional ao tamanho. */
const area = computed<PlotArea>(() => {
  const left = 64
  const right = rightSeries.value.length > 0 ? 64 : 12
  const top = 10
  const bottom = 24
  return {
    left,
    top,
    width: Math.max(10, width.value - left - right),
    height: Math.max(10, props.height - top - bottom),
  }
})

const leftTicks = computed(() => {
  if (props.unitType === 'boolean') {
    return [
      { y: yAt(0, leftDomain.value, area.value), label: 'Desligado', base: true },
      { y: yAt(1, leftDomain.value, area.value), label: 'Ligado', base: false },
    ]
  }
  return axisTicks(leftDomain.value.min, leftDomain.value.max, 4).map((value) => ({
    y: yAt(value, leftDomain.value, area.value),
    label: formatLeft.value(value),
    base: value === leftDomain.value.min,
  }))
})

const rightTicks = computed(() =>
  rightSeries.value.length === 0
    ? []
    : axisTicks(rightDomain.value.min, rightDomain.value.max, 4).map((value) => ({
        y: yAt(value, rightDomain.value, area.value),
        label: formatRight.value(value),
      }))
)

/** Série de referência para o eixo de tempo: a mais longa. */
const timeline = computed(
  () =>
    props.series.reduce<ChartSeriesInput | null>(
      (longest, s) => (!longest || s.data.length > longest.data.length ? s : longest),
      null
    )?.data ?? []
)

const timeTicks = computed(() => {
  const data = timeline.value
  if (data.length === 0) return []
  const first = Date.parse(data[0].time)
  const last = Date.parse(data[data.length - 1].time)
  const span = Number.isFinite(first) && Number.isFinite(last) ? Math.abs(last - first) : 0
  // Um rótulo a cada ~110 px, no mínimo o primeiro e o último.
  const slots = Math.max(2, Math.min(data.length, Math.floor(area.value.width / 110) + 1))
  const indexes = Array.from(
    new Set(
      Array.from({ length: slots }, (_, i) =>
        Math.round((i * (data.length - 1)) / (slots - 1 || 1))
      )
    )
  )
  return indexes.map((index, i) => ({
    x: xAt(index, count.value, area.value),
    label: formatAxisTime(data[index].time, span),
    anchor: (i === 0 && indexes.length > 1
      ? 'start'
      : i === indexes.length - 1 && indexes.length > 1
        ? 'end'
        : 'middle') as 'start' | 'middle' | 'end',
  }))
})

const drawn = computed(() =>
  props.series.map((s) => {
    const domain = s.axis === 'right' ? rightDomain.value : leftDomain.value
    const format = s.axis === 'right' ? formatRight.value : formatLeft.value
    const points = s.data.map((d, index) => ({
      x: xAt(index, count.value, area.value),
      y: d.value === null ? null : yAt(d.value, domain, area.value),
    }))
    const baseY = yAt(Math.max(domain.min, 0), domain, area.value)
    const latest = [...s.data].reverse().find((d) => d.value !== null)
    return {
      id: s.id,
      label: s.label,
      color: s.color,
      dashed: s.dashed === true,
      fillArea: s.fillArea !== false && props.series.length <= 2,
      gradientId: `mc-${uid}-${s.id}`,
      line: linePath(points, resolvedCurve.value),
      area: areaPath(points, baseY, resolvedCurve.value),
      latestLabel:
        latest && latest.value !== null ? (latest.formattedValue ?? format(latest.value)) : '',
      markers: s.data.flatMap((d, index) => {
        const y = points[index].y
        return d.color && d.color !== s.color && y !== null
          ? [{ index, x: points[index].x, y, color: d.color }]
          : []
      }),
      points,
      format,
    }
  })
)

const legendVisible = computed(
  () =>
    (props.showLegend ?? (props.series.length > 1 || referenceLines.value.length > 0)) &&
    hasData.value
)

const hover = computed(() => {
  const index = hoverIndex.value
  if (index === null || !hasData.value) return null
  const rows: ChartTooltipRow[] = []
  const dots: { id: string; y: number; color: string }[] = []
  let time = ''
  let status = ''
  drawn.value.forEach((s, i) => {
    const datum = props.series[i].data[index]
    const y = s.points[index]?.y
    if (!datum) return
    time ||= datum.time
    status ||= datum.status ?? ''
    rows.push({
      label: s.label,
      color: datum.color ?? s.color,
      value: datum.value === null ? 'sem amostra' : (datum.formattedValue ?? s.format(datum.value)),
    })
    if (y !== null && y !== undefined) dots.push({ id: s.id, y, color: s.color })
  })
  const footer = [formatShortDateTime(time), status ? status.toUpperCase() : '']
    .filter(Boolean)
    .join(' · ')
  return { x: xAt(index, count.value, area.value), rows, dots, footer }
})

const tooltipStyle = computed(() => {
  if (!pointer.value || !hover.value) return null
  return chartTooltipStyle({
    x: pointer.value.x,
    y: pointer.value.y,
    containerWidth: width.value,
    containerHeight: props.height,
    estimatedHeight: 30 + hover.value.rows.length * 18,
  })
})

function track(clientX: number, clientY: number) {
  const el = plotRef.value
  if (!el || !hasData.value) return
  const rect = el.getBoundingClientRect()
  const x = clientX - rect.left
  pointer.value = { x, y: clientY - rect.top }
  hoverIndex.value = nearestIndex(x, count.value, area.value)
}

function onPointer(e: MouseEvent) {
  track(e.clientX, e.clientY)
}

function onTouch(e: TouchEvent) {
  const touch = e.touches[0]
  if (touch) track(touch.clientX, touch.clientY)
}

function onClick() {
  if (props.clickable && hoverIndex.value !== null) emit('point-click', hoverIndex.value)
}
</script>

<style scoped>
.metric-chart {
  width: 100%;
  user-select: none;
}

.metric-chart__plot {
  position: relative;
  width: 100%;
}

.metric-chart__plot--clickable {
  cursor: pointer;
}

.metric-chart__svg {
  display: block;
  overflow: visible;
}

.metric-chart__grid {
  stroke: rgba(var(--v-theme-on-surface), 0.1);
  stroke-width: 1;
  shape-rendering: crispEdges;
}

.metric-chart__grid--base {
  stroke: rgba(var(--v-theme-on-surface), 0.28);
}

.metric-chart__label {
  fill: rgba(var(--v-theme-on-surface), 0.72);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  dominant-baseline: middle;
}

.metric-chart__line {
  fill: none;
  stroke-width: 1.5;
  stroke-linecap: round;
  stroke-linejoin: round;
  vector-effect: non-scaling-stroke;
}

.metric-chart__line--dashed {
  stroke-dasharray: 5 4;
}

.metric-chart__reference {
  stroke-width: 1;
  stroke-dasharray: 4 4;
  shape-rendering: crispEdges;
}

.metric-chart__crosshair {
  stroke: rgba(var(--v-theme-on-surface), 0.4);
  stroke-width: 1;
  stroke-dasharray: 3 3;
  shape-rendering: crispEdges;
}

.metric-chart__marker {
  stroke: rgb(var(--v-theme-surface));
  stroke-width: 1.5;
}

.metric-chart__empty {
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  text-align: center;
}

.metric-chart__legend {
  display: flex;
  flex-wrap: wrap;
  gap: 4px 16px;
  margin-top: 8px;
  padding-left: 64px;
  font-size: 12px;
}

.metric-chart__legend-item {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.metric-chart__legend-swatch {
  width: 14px;
  border-top: 2px solid;
}

.metric-chart__legend-swatch--dashed {
  border-top-style: dashed;
}
</style>
