<template>
  <div class="bar-chart">
    <div
      ref="plotRef"
      class="bar-chart__plot"
      :style="{ height: `${height}px` }"
      role="img"
      :aria-label="ariaLabel"
      @mousemove="onPointer"
      @mouseleave="hoverIndex = null"
      @touchmove.passive="onTouch"
      @touchend="hoverIndex = null"
    >
      <svg
        v-if="width > 0 && categories.length > 0"
        class="bar-chart__svg"
        :width="width"
        :height="height"
        :viewBox="`0 0 ${width} ${height}`"
      >
        <g v-for="tick in ticks" :key="`t-${tick.y}`">
          <line
            :x1="area.left"
            :x2="area.left + area.width"
            :y1="tick.y"
            :y2="tick.y"
            class="bar-chart__grid"
            :class="{ 'bar-chart__grid--base': tick.base }"
          />
          <text :x="area.left - 8" :y="tick.y" class="bar-chart__label" text-anchor="end">
            {{ tick.label }}
          </text>
        </g>

        <rect
          v-if="hoverIndex !== null"
          :x="slotX(hoverIndex)"
          :y="area.top"
          :width="slotWidth"
          :height="area.height"
          class="bar-chart__hover"
        />

        <g v-for="(bar, index) in bars" :key="`b-${index}`">
          <rect
            v-for="segment in bar.segments"
            :key="segment.id"
            :x="bar.x"
            :y="segment.y"
            :width="bar.width"
            :height="segment.height"
            :fill="segment.color"
            rx="2"
          />
          <text
            v-if="bar.showLabel"
            :x="bar.x + bar.width / 2"
            :y="height - 4"
            class="bar-chart__label"
            text-anchor="middle"
          >
            {{ categories[index].label }}
          </text>
        </g>
      </svg>

      <ChartTooltipCard
        v-if="tooltip"
        :rows="tooltip.rows"
        :footer="tooltip.footer"
        :position-style="tooltip.style"
      />
    </div>

    <div v-if="showLegend" class="bar-chart__legend">
      <span v-for="s in series" :key="s.id" class="bar-chart__legend-item">
        <span class="bar-chart__legend-swatch" :style="{ backgroundColor: s.color }"></span>
        {{ s.label }} <strong>{{ formatValue(totals[s.id] ?? 0) }}</strong>
      </span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import ChartTooltipCard from '@/components/charts/ChartTooltipCard.vue'
import { useElementWidth } from '@/composables/useElementWidth'
import { chartTooltipStyle } from '@/utils/chartTooltip'
import { axisTicks, yAt, yDomain, type PlotArea } from '@/utils/charts/scale'

export interface BarSeries {
  id: string
  label: string
  color: string
}

export interface BarCategory {
  label: string
  /** Valor de cada série (id → valor). */
  values: Record<string, number>
  /** Texto do rodapé do tooltip (padrão: o rótulo). */
  detail?: string
}

/**
 * Barras empilhadas por categoria (ex.: eventos por hora e severidade),
 * desenhadas em pixels reais — mesma grade, rótulos e tooltip dos demais
 * gráficos do sistema.
 */
const props = withDefaults(
  defineProps<{
    categories: BarCategory[]
    series: BarSeries[]
    height?: number
    formatValue?: (value: number) => string
    showLegend?: boolean
    ariaLabel?: string
  }>(),
  {
    height: 200,
    formatValue: (value: number) => String(Math.round(value)),
    showLegend: true,
    ariaLabel: 'Gráfico de barras',
  }
)

const plotRef = ref<HTMLElement | null>(null)
const width = useElementWidth(plotRef)
const hoverIndex = ref<number | null>(null)
const pointer = ref<{ x: number; y: number } | null>(null)

const area = computed<PlotArea>(() => ({
  left: 48,
  top: 10,
  width: Math.max(10, width.value - 48 - 8),
  height: Math.max(10, props.height - 10 - 24),
}))

const stackTotals = computed(() =>
  props.categories.map((c) => props.series.reduce((sum, s) => sum + (c.values[s.id] ?? 0), 0))
)

const domain = computed(() => {
  const d = yDomain(stackTotals.value)
  // Contagens: o teto mínimo é 4, para as marcas saírem inteiras.
  return { min: 0, max: Math.max(4, d.max) }
})

const ticks = computed(() =>
  axisTicks(domain.value.min, domain.value.max, 4).map((value) => ({
    y: yAt(value, domain.value, area.value),
    label: props.formatValue(value),
    base: value === domain.value.min,
  }))
)

const slotWidth = computed(() => area.value.width / Math.max(1, props.categories.length))

function slotX(index: number): number {
  return area.value.left + index * slotWidth.value
}

const bars = computed(() => {
  const barWidth = Math.max(4, Math.min(36, slotWidth.value * 0.6))
  // Rótulo a cada ~48 px, para não sobrepor.
  const every = Math.max(1, Math.ceil(48 / slotWidth.value))
  return props.categories.map((category, index) => {
    let base = domain.value.min
    const segments = props.series.flatMap((s) => {
      const value = category.values[s.id] ?? 0
      if (value <= 0) return []
      const top = yAt(base + value, domain.value, area.value)
      const bottom = yAt(base, domain.value, area.value)
      base += value
      return [{ id: s.id, y: top, height: Math.max(1, bottom - top), color: s.color }]
    })
    return {
      x: slotX(index) + (slotWidth.value - barWidth) / 2,
      width: barWidth,
      segments,
      showLabel: index % every === 0,
    }
  })
})

const totals = computed(() => {
  const result: Record<string, number> = {}
  for (const s of props.series) {
    result[s.id] = props.categories.reduce((sum, c) => sum + (c.values[s.id] ?? 0), 0)
  }
  return result
})

const tooltip = computed(() => {
  const index = hoverIndex.value
  if (index === null || !pointer.value) return null
  const category = props.categories[index]
  if (!category) return null
  const rows = props.series.map((s) => ({
    label: s.label,
    color: s.color,
    value: props.formatValue(category.values[s.id] ?? 0),
  }))
  return {
    rows,
    footer: category.detail ?? category.label,
    style: chartTooltipStyle({
      x: pointer.value.x,
      y: pointer.value.y,
      containerWidth: width.value,
      containerHeight: props.height,
      estimatedHeight: 30 + rows.length * 18,
    }),
  }
})

function track(clientX: number, clientY: number) {
  const el = plotRef.value
  if (!el || props.categories.length === 0) return
  const rect = el.getBoundingClientRect()
  const x = clientX - rect.left
  pointer.value = { x, y: clientY - rect.top }
  const index = Math.floor((x - area.value.left) / slotWidth.value)
  hoverIndex.value = index >= 0 && index < props.categories.length ? index : null
}

function onPointer(e: MouseEvent) {
  track(e.clientX, e.clientY)
}

function onTouch(e: TouchEvent) {
  const touch = e.touches[0]
  if (touch) track(touch.clientX, touch.clientY)
}
</script>

<style scoped>
.bar-chart {
  width: 100%;
  user-select: none;
}

.bar-chart__plot {
  position: relative;
  width: 100%;
}

.bar-chart__svg {
  display: block;
  overflow: visible;
}

.bar-chart__grid {
  stroke: rgba(var(--v-theme-on-surface), 0.1);
  stroke-width: 1;
  shape-rendering: crispEdges;
}

.bar-chart__grid--base {
  stroke: rgba(var(--v-theme-on-surface), 0.28);
}

.bar-chart__hover {
  fill: rgba(var(--v-theme-on-surface), 0.05);
}

.bar-chart__label {
  fill: rgba(var(--v-theme-on-surface), 0.72);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  dominant-baseline: middle;
}

.bar-chart__legend {
  display: flex;
  flex-wrap: wrap;
  gap: 4px 16px;
  margin-top: 8px;
  padding-left: 48px;
  font-size: 12px;
}

.bar-chart__legend-item {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.bar-chart__legend-swatch {
  width: 10px;
  height: 10px;
  border-radius: 2px;
}
</style>
