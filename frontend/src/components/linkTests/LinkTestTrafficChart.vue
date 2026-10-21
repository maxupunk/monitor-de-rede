<template>
  <figure class="traffic-chart" :aria-label="summary">
    <div ref="box" class="plot" :style="{ minHeight: `${height}px` }">
      <svg
        v-if="width > 0"
        :width="width"
        :height="height"
        role="img"
        :aria-label="summary"
        @pointerdown="hover"
        @pointermove="hover"
        @pointerleave="cursor = null"
      >
        <!-- Grade: vazão à esquerda, % à direita. -->
        <g class="grid">
          <line
            v-for="tick in leftTicks"
            :key="`l${tick.value}`"
            :x1="PAD.left"
            :x2="width - PAD.right"
            :y1="tick.y"
            :y2="tick.y"
          />
        </g>
        <g class="axis-text">
          <text
            v-for="tick in leftTicks"
            :key="`lt${tick.value}`"
            :x="PAD.left - 6"
            :y="tick.y + 4"
            text-anchor="end"
          >
            {{ tick.label }}
          </text>
          <template v-if="lines.length > 0">
            <text
              v-for="tick in rightTicks"
              :key="`rt${tick.value}`"
              :x="width - PAD.right + 6"
              :y="tick.y + 4"
            >
              {{ tick.label }}
            </text>
          </template>
          <text
            v-for="tick in timeTicks"
            :key="`tt${tick.value}`"
            :x="tick.x"
            :y="height - 4"
            text-anchor="middle"
          >
            {{ tick.label }}
          </text>
        </g>
        <line
          v-if="lines.length > 0"
          class="limit"
          :x1="PAD.left"
          :x2="width - PAD.right"
          :y1="yPercent(HEALTH_HIGH_PERCENT)"
          :y2="yPercent(HEALTH_HIGH_PERCENT)"
        />

        <!-- Vazão: área + linha. -->
        <template v-for="flow in flows" :key="flow.key">
          <path v-if="flow.area" :d="flow.area" :fill="flow.color" fill-opacity="0.16" />
          <path v-if="flow.line" :d="flow.line" :stroke="flow.color" class="flow-line" />
        </template>

        <!-- CPU (tracejado) e RAM (pontilhado) de cada equipamento. -->
        <template v-for="item in lines" :key="item.deviceId">
          <path v-if="item.memory" :d="item.memory" :stroke="item.color" class="memory-line" />
          <path v-if="item.cpu" :d="item.cpu" :stroke="item.color" class="cpu-line" />
        </template>

        <line
          v-if="cursor"
          class="cursor"
          :x1="cursor.x"
          :x2="cursor.x"
          :y1="PAD.top"
          :y2="height - PAD.bottom"
        />
      </svg>
      <div
        v-if="cursor && readout.length > 0"
        class="readout"
        :class="{ flip: cursor.x > width / 2, docked: narrow }"
        :style="narrow ? undefined : { left: `${cursor.x}px` }"
        role="status"
      >
        <div class="readout-time">{{ formatElapsedMs(cursor.t) }}</div>
        <div v-for="row in readout" :key="row.key" class="readout-row">
          <span class="swatch" :style="{ background: row.color }"></span>
          <span>{{ row.label }}</span>
          <strong>{{ row.value }}</strong>
        </div>
      </div>
    </div>

    <figcaption class="legend">
      <span v-if="forward.length > 0" class="legend-item">
        <span class="key key-flow" :style="{ borderColor: FORWARD_COLOR }"></span>Ida
      </span>
      <span v-if="reverse.length > 0" class="legend-item">
        <span class="key key-flow" :style="{ borderColor: REVERSE_COLOR }"></span>Volta
      </span>
      <span v-for="item in series" :key="item.deviceId" class="legend-item">
        <span class="key key-cpu" :style="{ borderColor: item.color }"></span>
        <strong>{{ item.name }}</strong>
        <span :class="{ high: (item.peakCpu ?? 0) >= HEALTH_HIGH_PERCENT }">
          {{ legendText(item) }}
        </span>
      </span>
      <span v-if="series.length > 0" class="legend-hint"> tracejado = CPU · pontilhado = RAM </span>
    </figcaption>
  </figure>
</template>

<script setup lang="ts">
/**
 * "Vazão × CPU" de uma perna do teste de velocidade: ida e volta (eixo da
 * esquerda) e o CPU/RAM de cada equipamento lido (eixo da direita, 0–100%),
 * no mesmo relógio. Passar o ponteiro mostra os valores daquele segundo.
 * Sem leitura de CPU, é só o gráfico de vazão.
 */
import { computed, ref } from 'vue'
import { useElementWidth } from '@/composables/useElementWidth'
import type { LinkTestHealthSample, LinkTestSample } from '@/types/linkTests'
import { formatElapsedMs, formatPercent } from '@/utils/formatters'
import {
  healthSeries,
  trafficPoints,
  valueAt,
  type ChartPoint,
  type DeviceHealthSeries,
} from '@/utils/linkTests/health'
import { compactBps } from '@/utils/linkTests/presentation'
import { HEALTH_HIGH_PERCENT } from '@/utils/topology/presentation'

const props = withDefaults(
  defineProps<{
    forward: readonly LinkTestSample[]
    reverse: readonly LinkTestSample[]
    health?: readonly LinkTestHealthSample[]
    /** Nome de cada equipamento lido. */
    names?: Readonly<Record<number, string>>
    /** Ordem do caminho (define as cores). */
    order?: readonly number[]
    height?: number
  }>(),
  { health: () => [], names: () => ({}), order: () => [], height: 140 }
)

const FORWARD_COLOR = '#4ade80'
const REVERSE_COLOR = '#38bdf8'
const PAD = { top: 8, right: 40, bottom: 20, left: 56 }

const box = ref<HTMLElement | null>(null)
const width = useElementWidth(box)
/** Tela estreita: a leitura do instante vai para baixo do gráfico, inteira. */
const NARROW_WIDTH = 480
const narrow = computed(() => width.value > 0 && width.value < NARROW_WIDTH)

const points = computed(() => trafficPoints(props.forward, props.reverse))
const series = computed(() => healthSeries(props.health, props.names, props.order))

const span = computed(() => {
  const all = [
    ...points.value.forward,
    ...points.value.reverse,
    ...series.value.flatMap((item) => [...item.cpu, ...item.memory]),
  ].map((point) => point.t)
  const start = all.length > 0 ? Math.min(...all) : 0
  const end = all.length > 0 ? Math.max(...all) : 1000
  return { start, end: Math.max(end, start + 1000) }
})
const maxBps = computed(() => {
  const peak = Math.max(0, ...[...points.value.forward, ...points.value.reverse].map((p) => p.v))
  return peak > 0 ? peak * 1.15 : 1
})

const plotWidth = computed(() => Math.max(1, width.value - PAD.left - PAD.right))
const plotHeight = computed(() => Math.max(1, props.height - PAD.top - PAD.bottom))

function x(t: number): number {
  return PAD.left + ((t - span.value.start) / (span.value.end - span.value.start)) * plotWidth.value
}
function yBps(v: number): number {
  return PAD.top + plotHeight.value * (1 - v / maxBps.value)
}
function yPercent(v: number): number {
  return PAD.top + plotHeight.value * (1 - Math.min(Math.max(v, 0), 100) / 100)
}

function linePath(list: readonly ChartPoint[], y: (v: number) => number): string | null {
  if (list.length === 0) return null
  if (list.length === 1) {
    const py = y(list[0].v)
    return `M${x(list[0].t) - 3},${py}L${x(list[0].t) + 3},${py}`
  }
  return list.map((p, i) => `${i === 0 ? 'M' : 'L'}${x(p.t)},${y(p.v)}`).join('')
}
function areaPath(list: readonly ChartPoint[]): string | null {
  const line = linePath(list, yBps)
  if (!line || list.length < 2) return null
  const base = PAD.top + plotHeight.value
  return `${line}L${x(list[list.length - 1].t)},${base}L${x(list[0].t)},${base}Z`
}

const flows = computed(() =>
  [
    { key: 'forward', color: FORWARD_COLOR, list: points.value.forward },
    { key: 'reverse', color: REVERSE_COLOR, list: points.value.reverse },
  ].map((flow) => ({ ...flow, line: linePath(flow.list, yBps), area: areaPath(flow.list) }))
)
const lines = computed(() =>
  series.value.map((item) => ({
    deviceId: item.deviceId,
    color: item.color,
    cpu: linePath(item.cpu, yPercent),
    memory: linePath(item.memory, yPercent),
  }))
)

/** As linhas da grade valem para os dois eixos: 0, metade e topo. */
const leftTicks = computed(() =>
  [0, 0.5, 1].map((fraction) => {
    const value = maxBps.value * fraction
    return { value, y: yBps(value), label: fraction === 0 ? '0' : compactBps(value) }
  })
)
const rightTicks = computed(() =>
  [0, 50, 100].map((value) => ({ value, y: yPercent(value), label: `${value}%` }))
)
const timeTicks = computed(() => {
  const { start, end } = span.value
  return [start, (start + end) / 2, end].map((value) => ({
    value,
    x: x(value),
    label: formatElapsedMs(value - start),
  }))
})

const cursor = ref<{ x: number; t: number } | null>(null)
function hover(event: PointerEvent) {
  const target = event.currentTarget as SVGElement
  const px = event.clientX - target.getBoundingClientRect().left
  const clamped = Math.min(Math.max(px, PAD.left), width.value - PAD.right)
  const t =
    span.value.start +
    ((clamped - PAD.left) / plotWidth.value) * (span.value.end - span.value.start)
  cursor.value = { x: clamped, t }
}

const readout = computed(() => {
  const at = cursor.value?.t
  if (at === undefined) return []
  const rows: { key: string; color: string; label: string; value: string }[] = []
  for (const flow of flows.value) {
    const value = valueAt(flow.list, at)
    if (value !== null)
      rows.push({
        key: flow.key,
        color: flow.color,
        label: flow.key === 'forward' ? 'Ida' : 'Volta',
        value: compactBps(value),
      })
  }
  for (const item of series.value) {
    const cpu = valueAt(item.cpu, at)
    const memory = valueAt(item.memory, at)
    if (cpu === null && memory === null) continue
    rows.push({
      key: `d${item.deviceId}`,
      color: item.color,
      label: item.name,
      value: [
        cpu !== null ? `CPU ${formatPercent(cpu, 0)}` : null,
        memory !== null ? `RAM ${formatPercent(memory, 0)}` : null,
      ]
        .filter(Boolean)
        .join(' · '),
    })
  }
  return rows
})

function legendText(item: DeviceHealthSeries): string {
  return [
    item.peakCpu !== null ? `CPU máx ${formatPercent(item.peakCpu, 0)}` : null,
    item.lastMemory !== null ? `RAM ${formatPercent(item.lastMemory, 0)}` : null,
  ]
    .filter(Boolean)
    .join(' · ')
}

const summary = computed(() => {
  const parts: string[] = []
  const peak = (list: ChartPoint[]) => Math.max(...list.map((p) => p.v))
  if (points.value.forward.length > 0)
    parts.push(`ida até ${compactBps(peak(points.value.forward))}`)
  if (points.value.reverse.length > 0)
    parts.push(`volta até ${compactBps(peak(points.value.reverse))}`)
  for (const item of series.value) {
    const text = legendText(item)
    if (text) parts.push(`${item.name}: ${text}`)
  }
  return `Vazão e CPU durante a medição — ${parts.join('; ') || 'sem amostras'}`
})
</script>

<style scoped>
.traffic-chart {
  margin: 0;
}
.plot {
  position: relative;
  width: 100%;
}
svg {
  display: block;
  touch-action: pan-y;
}
.grid line {
  stroke: rgba(var(--v-theme-on-surface), 0.14);
  stroke-width: 1;
}
.axis-text text {
  fill: rgb(var(--v-theme-on-surface));
  font-size: 10px;
  font-weight: 600;
}
.limit {
  stroke: rgb(var(--v-theme-error));
  stroke-width: 1;
  stroke-dasharray: 2 4;
  opacity: 0.7;
}
.flow-line {
  fill: none;
  stroke-width: 2;
  stroke-linejoin: round;
  stroke-linecap: round;
}
.cpu-line {
  fill: none;
  stroke-width: 2;
  stroke-dasharray: 6 4;
  stroke-linejoin: round;
}
.memory-line {
  fill: none;
  stroke-width: 1.5;
  stroke-dasharray: 1 4;
  stroke-linecap: round;
}
.cursor {
  stroke: rgb(var(--v-theme-on-surface));
  stroke-width: 1;
  opacity: 0.5;
}
.readout {
  position: absolute;
  top: 4px;
  transform: translateX(10px);
  padding: 6px 8px;
  border-radius: 6px;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.3);
  background: rgb(var(--v-theme-surface));
  color: rgb(var(--v-theme-on-surface));
  font-size: 0.75rem;
  pointer-events: none;
  white-space: nowrap;
  z-index: 1;
}
.readout.flip {
  transform: translateX(calc(-100% - 10px));
}
.readout.docked {
  position: static;
  transform: none;
  margin-top: 4px;
  white-space: normal;
}
.readout-time {
  font-weight: 700;
  margin-bottom: 2px;
}
.readout-row {
  display: flex;
  align-items: center;
  gap: 6px;
}
.legend {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 4px 14px;
  margin-top: 4px;
  font-size: 0.75rem;
  color: rgb(var(--v-theme-on-surface));
}
.legend-item {
  display: inline-flex;
  align-items: center;
  gap: 5px;
}
.legend-hint {
  font-style: italic;
}
.high {
  color: rgb(var(--v-theme-error));
  font-weight: 700;
}
.swatch {
  width: 10px;
  height: 10px;
  border-radius: 2px;
  flex-shrink: 0;
}
.key {
  width: 16px;
  height: 0;
  border-top-width: 2px;
}
.key-flow {
  border-top-style: solid;
}
.key-cpu {
  border-top-style: dashed;
}
</style>
