<template>
  <figure class="timeline" :aria-label="summary">
    <!-- Faixas no mesmo eixo de tempo: a vazão em cima, o CPU de cada equipamento embaixo. -->
    <div class="lane">
      <div class="lane-label">
        <strong>Vazão</strong>
        <span class="lane-story">
          <span v-if="points.forward.length > 0" class="flow">
            <span class="swatch" :style="{ background: FORWARD_COLOR }"></span>Ida
            {{ compactBps(peakOf(points.forward)) }}
          </span>
          <span v-if="points.reverse.length > 0" class="flow">
            <span class="swatch" :style="{ background: REVERSE_COLOR }"></span>Volta
            {{ compactBps(peakOf(points.reverse)) }}
          </span>
        </span>
      </div>
      <div ref="plot" class="lane-plot">
        <svg
          v-if="width > 0"
          :width="width"
          :height="TRAFFIC_HEIGHT"
          @pointerdown="hover"
          @pointermove="hover"
          @pointerleave="cursor = null"
        >
          <rect v-if="testBox" class="test-window" v-bind="testBox" :height="TRAFFIC_HEIGHT" />
          <text v-if="phaseLabels.before !== null" class="phase" :x="phaseLabels.before" y="11">
            antes
          </text>
          <text v-if="phaseLabels.test !== null" class="phase" :x="phaseLabels.test" y="11">
            teste
          </text>
          <text v-if="phaseLabels.after !== null" class="phase" :x="phaseLabels.after" y="11">
            depois
          </text>
          <line class="base" x1="0" :x2="width" :y1="TRAFFIC_HEIGHT - 1" :y2="TRAFFIC_HEIGHT - 1" />
          <template v-for="flow in flowPaths" :key="flow.key">
            <path v-if="flow.area" :d="flow.area" :style="{ fill: flow.color }" class="area" />
            <path v-if="flow.line" :d="flow.line" :style="{ stroke: flow.color }" class="line" />
          </template>
          <line
            v-if="cursorX !== null"
            class="cursor"
            :x1="cursorX"
            :x2="cursorX"
            y1="0"
            :y2="TRAFFIC_HEIGHT"
          />
        </svg>
      </div>
    </div>

    <div v-for="lane in lanes" :key="lane.deviceId" class="lane">
      <div class="lane-label">
        <strong>CPU · {{ lane.name }}</strong>
        <span class="lane-story" :class="{ high: lane.high }">{{ lane.story }}</span>
      </div>
      <div class="lane-plot">
        <svg
          v-if="width > 0"
          :width="width"
          :height="CPU_HEIGHT"
          @pointerdown="hover"
          @pointermove="hover"
          @pointerleave="cursor = null"
        >
          <rect v-if="testBox" class="test-window" v-bind="testBox" :height="CPU_HEIGHT" />
          <line
            class="limit"
            x1="0"
            :x2="width"
            :y1="yCpu(HEALTH_HIGH_PERCENT)"
            :y2="yCpu(HEALTH_HIGH_PERCENT)"
          />
          <line class="base" x1="0" :x2="width" :y1="CPU_HEIGHT - 1" :y2="CPU_HEIGHT - 1" />
          <path v-if="lane.area" :d="lane.area" :style="{ fill: lane.color }" class="area" />
          <path v-if="lane.line" :d="lane.line" :style="{ stroke: lane.color }" class="line" />
          <line
            v-if="cursorX !== null"
            class="cursor"
            :x1="cursorX"
            :x2="cursorX"
            y1="0"
            :y2="CPU_HEIGHT"
          />
        </svg>
      </div>
    </div>

    <div class="lane axis-lane">
      <div class="lane-label"></div>
      <div class="lane-plot axis">
        <span v-for="tick in timeTicks" :key="tick.t" class="tick" :style="tick.style">
          {{ tick.label }}
        </span>
      </div>
    </div>

    <figcaption class="readout" role="status">
      <template v-if="readout">
        <strong>{{ readout.time }}</strong>
        <span v-for="row in readout.rows" :key="row.key" class="readout-item">
          <span class="swatch" :style="{ background: row.color }"></span>{{ row.label }}
          <strong>{{ row.value }}</strong>
        </span>
      </template>
      <span v-else class="hint">
        <v-icon size="16" color="primary">mdi-gesture-tap</v-icon>
        Passe o mouse ou toque no gráfico para ver cada segundo.
        <span v-if="lanes.length > 0">
          A faixa marcada é o teste; a linha tracejada, 90% de CPU.
        </span>
      </span>
    </figcaption>
  </figure>
</template>

<script setup lang="ts">
/**
 * A linha do tempo de uma perna do teste de velocidade: a vazão (ida e volta)
 * numa faixa e o CPU de cada equipamento lido numa faixa própria, todas no
 * mesmo eixo de tempo — antes do tráfego, durante (faixa marcada) e depois.
 * Uma série por faixa, com a mesma cor na faixa e na leitura do instante; a
 * memória vai no texto de cada faixa e na leitura do instante.
 */
import { computed, ref } from 'vue'
import { useElementWidth } from '@/composables/useElementWidth'
import type { LinkTestHealthSample, LinkTestSample } from '@/types/linkTests'
import { formatPercent } from '@/utils/formatters'
import {
  BEFORE_SHOWN_MS,
  deviceHealth,
  measuredSteps,
  relativeTime,
  stepValueAt,
  testWindow,
  trafficPoints,
  usageStory,
  valueAt,
  type ChartPoint,
  type MeasuredStep,
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
    /** Ordem do caminho. */
    order?: readonly number[]
  }>(),
  { health: () => [], names: () => ({}), order: () => [] }
)

const FORWARD_COLOR = '#4ade80'
const REVERSE_COLOR = '#38bdf8'
const TRAFFIC_HEIGHT = 72
const CPU_HEIGHT = 36
/** CPU a partir do qual a faixa fica amarela (o vermelho é o `HEALTH_HIGH_PERCENT`). */
const CPU_WARNING_PERCENT = 70

const plot = ref<HTMLElement | null>(null)
const width = useElementWidth(plot)

const points = computed(() => trafficPoints(props.forward, props.reverse))
const devices = computed(() => deviceHealth(props.health, props.names, props.order))
const test = computed(() => testWindow(points.value.forward, points.value.reverse, props.health))

/** O trecho mostrado: um pouco do antes, o teste e o depois. */
const span = computed(() => {
  const times = [
    ...points.value.forward,
    ...points.value.reverse,
    ...devices.value.flatMap((device) => [...device.cpu, ...device.memory]),
  ].map((point) => point.t)
  let start = times.length > 0 ? Math.min(...times) : 0
  if (test.value) start = Math.max(start, test.value.start - BEFORE_SHOWN_MS)
  const end = times.length > 0 ? Math.max(...times) : start + 1000
  return { start, end: Math.max(end, start + 1000) }
})

function x(t: number): number {
  return ((t - span.value.start) / (span.value.end - span.value.start)) * width.value
}
const maxBps = computed(
  () => Math.max(1, ...[...points.value.forward, ...points.value.reverse].map((p) => p.v)) * 1.1
)
function yBps(v: number): number {
  return TRAFFIC_HEIGHT - 1 - (v / maxBps.value) * (TRAFFIC_HEIGHT - 16)
}
function yCpu(v: number): number {
  return CPU_HEIGHT - 1 - (Math.min(Math.max(v, 0), 100) / 100) * (CPU_HEIGHT - 4)
}

/** Degraus no trecho que cada leitura mediu (`measuredSteps`). */
function stepPath(steps: readonly MeasuredStep[]): string | null {
  const shown = steps.filter((step) => step.to >= span.value.start)
  if (shown.length === 0) return null
  return shown
    .map((step, i) => {
      const from = x(Math.max(step.from, span.value.start))
      const y = yCpu(step.v)
      return `${i === 0 ? 'M' : 'L'}${from},${y}L${x(step.to)},${y}`
    })
    .join('')
}
function stepArea(line: string | null, steps: readonly MeasuredStep[]): string | null {
  const shown = steps.filter((step) => step.to >= span.value.start)
  if (!line || shown.length === 0) return null
  const base = CPU_HEIGHT - 1
  const first = x(Math.max(shown[0].from, span.value.start))
  return `${line}L${x(shown[shown.length - 1].to)},${base}L${first},${base}Z`
}
function linePath(list: readonly ChartPoint[]): string | null {
  if (list.length === 0) return null
  if (list.length === 1) return `M${x(list[0].t) - 3},${yBps(list[0].v)}h6`
  return list.map((p, i) => `${i === 0 ? 'M' : 'L'}${x(p.t)},${yBps(p.v)}`).join('')
}
function closed(line: string | null, list: readonly ChartPoint[], base: number): string | null {
  if (!line || list.length < 2) return null
  const first = Math.max(list[0].t, span.value.start)
  return `${line}L${x(list[list.length - 1].t)},${base}L${x(first)},${base}Z`
}

const flowPaths = computed(() =>
  [
    { key: 'forward', color: FORWARD_COLOR, list: points.value.forward },
    { key: 'reverse', color: REVERSE_COLOR, list: points.value.reverse },
  ].map((flow) => {
    const line = linePath(flow.list)
    return { ...flow, line, area: closed(line, flow.list, TRAFFIC_HEIGHT - 1) }
  })
)

/**
 * Cor da faixa pelo pico no teste: verde folgado, amarelo alto, vermelho no
 * teto — as cores do tema, as mesmas na faixa e na leitura do instante.
 */
function levelColor(peak: number | null): string {
  if (peak !== null && peak >= HEALTH_HIGH_PERCENT) return 'rgb(var(--v-theme-error))'
  if (peak !== null && peak >= CPU_WARNING_PERCENT) return 'rgb(var(--v-theme-warning))'
  return 'rgb(var(--v-theme-success))'
}

const lanes = computed(() =>
  devices.value.map((device) => {
    const cpuSteps = measuredSteps(device.cpu)
    const line = stepPath(cpuSteps)
    return {
      ...device,
      cpuSteps,
      memorySteps: measuredSteps(device.memory),
      color: levelColor(device.peak.cpu ?? device.before.cpu),
      high: (device.peak.cpu ?? 0) >= HEALTH_HIGH_PERCENT,
      story: usageStory(device) ?? 'sem leitura de CPU',
      line,
      area: stepArea(line, cpuSteps),
    }
  })
)

const testBox = computed(() => {
  if (!test.value) return null
  const from = x(Math.max(test.value.start, span.value.start))
  return { x: from, y: 0, width: Math.max(2, x(test.value.end) - from) }
})

const phaseLabels = computed(() => {
  if (!test.value || width.value === 0) return { before: null, test: null, after: null }
  const from = x(test.value.start)
  const to = x(test.value.end)
  return {
    before: from > 48 ? 4 : null,
    test: to - from > 40 ? from + 4 : null,
    after: width.value - to > 52 ? to + 4 : null,
  }
})

/** Começo do trecho, começo e fim do teste, fim do trecho — sem marcas encavaladas. */
const timeTicks = computed(() => {
  if (width.value === 0) return []
  const zero = test.value?.start ?? span.value.start
  const candidates = [span.value.start, zero, test.value?.end, span.value.end].filter(
    (t): t is number => t !== undefined
  )
  const kept: number[] = []
  for (const t of candidates) {
    if (kept.every((other) => Math.abs(x(other) - x(t)) >= 56)) kept.push(t)
  }
  return kept.map((t) => {
    const px = x(t)
    const shift = px < 20 ? '0' : px > width.value - 20 ? '-100%' : '-50%'
    return {
      t,
      label: relativeTime(t - zero),
      style: { left: `${px}px`, transform: `translateX(${shift})` },
    }
  })
})

const cursor = ref<number | null>(null)
const cursorX = computed(() => (cursor.value === null ? null : x(cursor.value)))
function hover(event: PointerEvent) {
  const target = event.currentTarget as SVGElement
  const px = Math.min(Math.max(event.clientX - target.getBoundingClientRect().left, 0), width.value)
  cursor.value = span.value.start + (px / width.value) * (span.value.end - span.value.start)
}

const readout = computed(() => {
  const at = cursor.value
  if (at === null) return null
  const rows: { key: string; color: string; label: string; value: string }[] = []
  for (const flow of flowPaths.value) {
    const value = valueAt(flow.list, at)
    if (value !== null)
      rows.push({
        key: flow.key,
        color: flow.color,
        label: flow.key === 'forward' ? 'Ida' : 'Volta',
        value: compactBps(value),
      })
  }
  for (const lane of lanes.value) {
    const cpu = stepValueAt(lane.cpuSteps, at)
    const memory = stepValueAt(lane.memorySteps, at)
    if (cpu === null && memory === null) continue
    rows.push({
      key: `d${lane.deviceId}`,
      color: lane.color,
      label: lane.name,
      value: [
        cpu !== null ? `CPU ${formatPercent(cpu, 0)}` : null,
        memory !== null ? `RAM ${formatPercent(memory, 0)}` : null,
      ]
        .filter(Boolean)
        .join(' · '),
    })
  }
  return { time: relativeTime(at - (test.value?.start ?? span.value.start)), rows }
})

function peakOf(list: readonly ChartPoint[]): number {
  return Math.max(...list.map((p) => p.v))
}

const summary = computed(() => {
  const parts: string[] = []
  if (points.value.forward.length > 0)
    parts.push(`ida até ${compactBps(peakOf(points.value.forward))}`)
  if (points.value.reverse.length > 0)
    parts.push(`volta até ${compactBps(peakOf(points.value.reverse))}`)
  for (const lane of lanes.value) parts.push(`${lane.name}: ${lane.story}`)
  return `Vazão e CPU antes, durante e depois do teste — ${parts.join('; ') || 'sem amostras'}`
})
</script>

<style scoped>
.timeline {
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
  color: rgb(var(--v-theme-on-surface));
}
.lane {
  display: grid;
  grid-template-columns: 210px minmax(0, 1fr);
  align-items: center;
  gap: 12px;
}
.lane-label {
  display: flex;
  flex-direction: column;
  min-width: 0;
  font-size: 0.8rem;
  line-height: 1.3;
}
.lane-label strong {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.lane-story {
  display: flex;
  flex-wrap: wrap;
  gap: 2px 10px;
  font-size: 0.75rem;
}
.lane-story.high {
  color: rgb(var(--v-theme-error));
  font-weight: 700;
}
.flow {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}
.lane-plot {
  position: relative;
  min-width: 0;
}
svg {
  display: block;
  touch-action: pan-y;
  border-radius: 4px;
  background: rgba(var(--v-theme-on-surface), 0.04);
}
.test-window {
  fill: rgb(var(--v-theme-primary));
  fill-opacity: 0.12;
}
.phase {
  fill: rgb(var(--v-theme-on-surface));
  font-size: 10px;
  font-weight: 700;
}
.base {
  stroke: rgba(var(--v-theme-on-surface), 0.25);
}
.limit {
  stroke: rgb(var(--v-theme-error));
  stroke-dasharray: 3 3;
  opacity: 0.6;
}
.area {
  fill-opacity: 0.22;
}
.line {
  fill: none;
  stroke-width: 2;
  stroke-linejoin: round;
}
.cursor {
  stroke: rgb(var(--v-theme-on-surface));
  stroke-width: 1;
}
.axis {
  height: 14px;
  font-size: 0.7rem;
  font-weight: 600;
}
.tick {
  position: absolute;
  top: 0;
  white-space: nowrap;
}
.readout {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 4px 14px;
  min-height: 30px;
  padding: 4px 8px;
  border-radius: 6px;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.2);
  font-size: 0.78rem;
}
.readout-item {
  display: inline-flex;
  align-items: center;
  gap: 5px;
}
.hint {
  display: inline-flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 4px;
}
.swatch {
  width: 10px;
  height: 10px;
  border-radius: 2px;
  flex-shrink: 0;
}
@media (max-width: 599px) {
  .lane {
    grid-template-columns: minmax(0, 1fr);
    gap: 2px;
  }
  .axis-lane .lane-label {
    display: none;
  }
}
</style>
