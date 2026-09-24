<template>
  <div class="stacked-usage">
    <div v-if="stack.times.length > 0" class="stacked-usage__body">
      <!-- Eixo Y: rótulos em HTML para não esticarem com o SVG -->
      <div class="stacked-usage__axis" aria-hidden="true">
        <span v-for="tick in ticks" :key="tick.ratio" :style="{ bottom: `${tick.ratio * 100}%` }">
          {{ tick.label }}
        </span>
      </div>

      <div
        ref="plotRef"
        class="stacked-usage__plot"
        role="img"
        :aria-label="ariaLabel"
        @mousemove="onPointer"
        @mouseleave="hoverIndex = null"
        @touchmove.passive="onTouch"
        @touchend="hoverIndex = null"
      >
        <div
          v-for="tick in ticks"
          :key="`grid-${tick.ratio}`"
          class="stacked-usage__grid"
          :style="{ bottom: `${tick.ratio * 100}%` }"
        ></div>

        <svg
          :viewBox="`0 0 ${WIDTH} ${HEIGHT}`"
          preserveAspectRatio="none"
          class="stacked-usage__svg"
        >
          <polygon
            v-for="layer in shapes"
            :key="layer.id"
            :points="layer.points"
            :fill="layer.color"
            :fill-opacity="highlighted && highlighted !== layer.id ? 0.3 : 0.85"
            class="stacked-usage__band"
          />
          <polyline :points="totalLine" class="stacked-usage__total" />
          <line
            v-if="hoverX !== null"
            :x1="hoverX"
            :x2="hoverX"
            y1="0"
            :y2="HEIGHT"
            class="stacked-usage__crosshair"
          />
        </svg>

        <span
          v-if="hoverX !== null && hoverIndex !== null"
          class="stacked-usage__marker"
          :style="{
            left: `${(hoverX / WIDTH) * 100}%`,
            bottom: `${ratioOf(stack.totals[hoverIndex]) * 100}%`,
          }"
        ></span>

        <div v-if="tooltip" class="stacked-usage__tooltip" :style="tooltip.style">
          <div
            v-for="row in tooltip.rows"
            :key="row.id"
            class="stacked-usage__row"
            :class="{ 'stacked-usage__row--active': row.id === tooltip.hovered }"
          >
            <span class="stacked-usage__swatch" :style="{ backgroundColor: row.color }"></span>
            <span class="stacked-usage__name" :title="row.label">{{ row.label }}</span>
            <span class="stacked-usage__value">{{ formatValue(row.value) }}</span>
            <span class="stacked-usage__share">{{ formatPercent(row.share, 0) }}</span>
          </div>
          <div class="stacked-usage__tooltip-foot">
            <span>{{ tooltip.time }}</span>
            <span v-if="tooltip.hidden > 0"
              >+{{ tooltip.hidden }} {{ tooltip.hidden === 1 ? 'outro' : 'outros' }}</span
            >
            <span>{{ totalLabel }} {{ formatValue(tooltip.total) }}</span>
          </div>
        </div>
      </div>

      <div class="stacked-usage__times text-medium-emphasis">
        <span>{{ formatClockTime(stack.times[0]) }}</span>
        <span>{{ formatClockTime(stack.times[stack.times.length - 1]) }}</span>
      </div>
    </div>

    <div v-else class="stacked-usage__empty text-medium-emphasis">
      <v-icon size="32">mdi-chart-areaspline-variant</v-icon>
      <div class="text-caption mt-1">Aguardando a primeira amostra do SSE.</div>
    </div>

    <!-- Legenda: identidade nunca só pela cor -->
    <div v-if="stack.layers.length > 0" class="stacked-usage__legend">
      <span class="stacked-usage__legend-item stacked-usage__legend-total">
        <span class="stacked-usage__line-swatch"></span>
        {{ totalLabel }} <strong>{{ formatValue(latestTotal) }}</strong>
      </span>
      <span
        v-for="layer in legend"
        :key="layer.id"
        class="stacked-usage__legend-item"
        @mouseenter="hoveredLayer = layer.id"
        @mouseleave="hoveredLayer = null"
      >
        <span class="stacked-usage__swatch" :style="{ backgroundColor: layer.color }"></span>
        {{ layer.label }} <strong>{{ formatValue(layer.latest) }}</strong>
      </span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { chartTooltipStyle } from '@/utils/chartTooltip'
import { formatClockTime, formatPercent } from '@/utils/formatters'
import { buildStackedUsage, niceCeiling, pickLayers, type UsageSample } from '@/utils/stackedUsage'

const props = withDefaults(
  defineProps<{
    samples: UsageSample[]
    /** Nome de cada série (id → nome). */
    labels: Record<string, string>
    formatValue: (value: number) => string
    /** Rótulo da linha do total ("CPU total", "RAM total"). */
    totalLabel: string
    /** Teto mínimo do eixo (ex.: 100 para percentuais). */
    minCeiling?: number
    /** Base do arredondamento do eixo: 1024 para bytes. */
    scaleBase?: 10 | 1024
    ariaLabel?: string
  }>(),
  { minCeiling: 0, scaleBase: 10, ariaLabel: 'Consumo empilhado por série' }
)

const WIDTH = 1000
const HEIGHT = 200
/** Faixa mais fina que isto (em px) é difícil de mirar: o tooltip junta as vizinhas. */
const MIN_BAND_PX = 10

const plotRef = ref<HTMLElement | null>(null)
const hoverIndex = ref<number | null>(null)
const hoveredLayer = ref<string | null>(null)
const pointer = ref<{ x: number; y: number; width: number; height: number } | null>(null)

const stack = computed(() => buildStackedUsage(props.samples, props.labels))
const ceiling = computed(() =>
  niceCeiling(Math.max(stack.value.peak, props.minCeiling), props.scaleBase)
)

function ratioOf(value: number): number {
  return ceiling.value > 0 ? Math.min(1, Math.max(0, value / ceiling.value)) : 0
}

function xOf(index: number): number {
  const count = stack.value.times.length
  return count > 1 ? (index / (count - 1)) * WIDTH : WIDTH / 2
}

function yOf(value: number): number {
  return HEIGHT - ratioOf(value) * HEIGHT
}

const ticks = computed(() =>
  [0, 0.25, 0.5, 0.75, 1].map((ratio) => ({
    ratio,
    label: props.formatValue(ceiling.value * ratio),
  }))
)

/** Cada faixa é o polígono entre o topo e a base dela. */
const shapes = computed(() =>
  stack.value.layers.map((layer) => {
    const top = layer.upper.map((value, index) => `${xOf(index)},${yOf(value)}`)
    const bottom = layer.lower.map((value, index) => `${xOf(index)},${yOf(value)}`).reverse()
    // Uma amostra só vira uma faixa estreita no centro, em vez de sumir.
    const points =
      layer.upper.length === 1
        ? `${WIDTH / 2 - 6},${yOf(layer.upper[0])} ${WIDTH / 2 + 6},${yOf(layer.upper[0])} ${WIDTH / 2 + 6},${yOf(layer.lower[0])} ${WIDTH / 2 - 6},${yOf(layer.lower[0])}`
        : [...top, ...bottom].join(' ')
    return { id: layer.id, color: layer.color, points }
  })
)

const totalLine = computed(() =>
  stack.value.totals.map((value, index) => `${xOf(index)},${yOf(value)}`).join(' ')
)

const latestTotal = computed(() => stack.value.totals.at(-1) ?? 0)

const legend = computed(() =>
  stack.value.layers.map((layer) => ({
    id: layer.id,
    label: layer.label,
    color: layer.color,
    latest: layer.values.at(-1) ?? 0,
  }))
)

const hoverX = computed(() => (hoverIndex.value === null ? null : xOf(hoverIndex.value)))

function locate(clientX: number, clientY: number) {
  const element = plotRef.value
  const count = stack.value.times.length
  if (!element || count === 0) return
  const rect = element.getBoundingClientRect()
  if (rect.width <= 0) return
  const fraction = Math.min(1, Math.max(0, (clientX - rect.left) / rect.width))
  hoverIndex.value = count > 1 ? Math.round(fraction * (count - 1)) : 0
  pointer.value = {
    x: clientX - rect.left,
    y: clientY - rect.top,
    width: rect.width,
    height: rect.height,
  }
}

function onPointer(event: MouseEvent) {
  locate(event.clientX, event.clientY)
}

function onTouch(event: TouchEvent) {
  const touch = event.touches[0]
  if (touch) locate(touch.clientX, touch.clientY)
}

/** Quais faixas o cursor aponta: a de baixo dele ou, se finas, as vizinhas. */
const pick = computed(() => {
  const index = hoverIndex.value
  const at = pointer.value
  if (index === null || !at || at.height <= 0) return null
  const value = (1 - at.y / at.height) * ceiling.value
  const minBand = (ceiling.value * MIN_BAND_PX) / at.height
  return { index, ...pickLayers(stack.value, index, value, minBand) }
})

/** Faixa em destaque: a da legenda sob o mouse ou a apontada no gráfico. */
const highlighted = computed(() => hoveredLayer.value ?? pick.value?.hovered ?? null)

const tooltip = computed(() => {
  const picked = pick.value
  if (!picked || !pointer.value) return null
  const { index } = picked
  const total = stack.value.totals[index] ?? 0
  const rows = picked.rows.flatMap((id) => {
    const layer = stack.value.layers.find((item) => item.id === id)
    if (!layer) return []
    const value = layer.values[index] ?? 0
    return [
      {
        id,
        label: layer.label,
        color: layer.color,
        value,
        share: total > 0 ? (value / total) * 100 : 0,
      },
    ]
  })
  return {
    time: formatClockTime(stack.value.times[index]),
    total,
    rows,
    hovered: picked.hovered,
    hidden: picked.hidden,
    // No máximo três linhas: o cartão cabe sempre, sem corte nem rolagem.
    style: {
      ...chartTooltipStyle({
        x: pointer.value.x,
        y: pointer.value.y,
        containerWidth: pointer.value.width,
        containerHeight: pointer.value.height,
        maxWidth: 320,
        minWidth: 250,
        estimatedHeight: 36 + rows.length * 24,
      }),
      maxHeight: 'none',
      overflowY: 'visible' as const,
    },
  }
})
</script>

<style scoped>
.stacked-usage__body {
  display: grid;
  grid-template-columns: auto 1fr;
  grid-template-rows: 220px auto;
  column-gap: 8px;
}

.stacked-usage__axis {
  position: relative;
  min-width: 52px;
  font-size: 11px;
  color: rgba(var(--v-theme-on-surface), var(--v-medium-emphasis-opacity));
  text-align: right;
}

.stacked-usage__axis span {
  position: absolute;
  right: 0;
  transform: translateY(50%);
  white-space: nowrap;
}

.stacked-usage__plot {
  position: relative;
  cursor: crosshair;
  border-left: 1px solid rgba(var(--v-theme-on-surface), 0.2);
  border-bottom: 1px solid rgba(var(--v-theme-on-surface), 0.2);
}

.stacked-usage__grid {
  position: absolute;
  left: 0;
  right: 0;
  border-top: 1px dashed rgba(var(--v-theme-on-surface), 0.08);
  pointer-events: none;
}

.stacked-usage__svg {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  overflow: visible;
}

/* Contorno na cor da superfície: a folga que separa uma faixa da outra. */
.stacked-usage__band {
  stroke: rgb(var(--v-theme-surface));
  stroke-width: 1.5;
  vector-effect: non-scaling-stroke;
  transition: fill-opacity 0.15s ease;
}

.stacked-usage__total {
  fill: none;
  stroke: rgb(var(--v-theme-on-surface));
  stroke-width: 2;
  stroke-linejoin: round;
  vector-effect: non-scaling-stroke;
}

.stacked-usage__crosshair {
  stroke: rgba(var(--v-theme-on-surface), 0.55);
  stroke-width: 1;
  stroke-dasharray: 4 3;
  vector-effect: non-scaling-stroke;
}

.stacked-usage__marker {
  position: absolute;
  width: 10px;
  height: 10px;
  margin: 0 0 -5px -5px;
  border-radius: 50%;
  background: rgb(var(--v-theme-on-surface));
  box-shadow: 0 0 0 2px rgb(var(--v-theme-surface));
  pointer-events: none;
}

.stacked-usage__tooltip {
  z-index: 5;
  padding: 8px 10px;
  border-radius: 8px;
  background: rgb(var(--v-theme-surface));
  color: rgb(var(--v-theme-on-surface));
  border: 1px solid rgba(var(--v-theme-on-surface), 0.16);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
}

.stacked-usage__row {
  display: grid;
  grid-template-columns: 10px minmax(0, 1fr) auto 36px;
  align-items: center;
  column-gap: 8px;
  font-size: 12px;
  line-height: 1.8;
  padding: 0 4px;
  border-radius: 4px;
}

.stacked-usage__row--active {
  background: rgba(var(--v-theme-on-surface), 0.08);
  font-weight: 600;
}

.stacked-usage__tooltip-foot {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  margin-top: 4px;
  padding: 4px 4px 0;
  border-top: 1px solid rgba(var(--v-theme-on-surface), 0.12);
  font-size: 11px;
  color: rgba(var(--v-theme-on-surface), var(--v-medium-emphasis-opacity));
  white-space: nowrap;
}

.stacked-usage__name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.stacked-usage__value {
  font-variant-numeric: tabular-nums;
  text-align: right;
}

.stacked-usage__share {
  font-variant-numeric: tabular-nums;
  text-align: right;
  color: rgba(var(--v-theme-on-surface), var(--v-medium-emphasis-opacity));
}

.stacked-usage__swatch {
  display: inline-block;
  width: 10px;
  height: 10px;
  border-radius: 3px;
  flex: 0 0 auto;
}

.stacked-usage__line-swatch {
  display: inline-block;
  width: 14px;
  height: 2px;
  border-radius: 2px;
  background: rgb(var(--v-theme-on-surface));
}

.stacked-usage__times {
  grid-column: 2;
  display: flex;
  justify-content: space-between;
  padding-top: 4px;
  font-size: 11px;
}

.stacked-usage__legend {
  display: flex;
  flex-wrap: wrap;
  gap: 6px 14px;
  margin-top: 12px;
  font-size: 12px;
  color: rgba(var(--v-theme-on-surface), var(--v-medium-emphasis-opacity));
}

.stacked-usage__legend-item {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  cursor: default;
}

.stacked-usage__legend-item strong {
  color: rgb(var(--v-theme-on-surface));
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}

.stacked-usage__empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 220px;
}
</style>
