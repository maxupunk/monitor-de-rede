<template>
  <div
    ref="chartContainerRef"
    class="base-metric-chart-container relative pa-2 border rounded-lg bg-surface"
    @mousemove="handleMouseMove"
    @mouseleave="handleMouseLeave"
  >
    <svg
      v-if="seriesList.length > 0 && maxPointCount > 0"
      ref="svgRef"
      class="w-100 base-metric-svg"
      viewBox="0 0 800 240"
      preserveAspectRatio="none"
    >
      <defs>
        <linearGradient
          v-for="s in seriesList"
          :id="`grad-${s.id}`"
          :key="`grad-${s.id}`"
          x1="0"
          y1="0"
          x2="0"
          y2="1"
        >
          <stop offset="0%" :stop-color="s.color" stop-opacity="0.35" />
          <stop offset="100%" :stop-color="s.color" stop-opacity="0.0" />
        </linearGradient>
      </defs>

      <!-- Linhas de Grade de Fundo (Grid Lines) -->
      <line x1="75" y1="30" x2="785" y2="30" stroke="#E0E0E0" stroke-dasharray="3,3" />
      <text x="68" y="34" font-size="11" fill="#9E9E9E" text-anchor="end">
        {{ formattedMaxVal }}
      </text>

      <line x1="75" y1="110" x2="785" y2="110" stroke="#E0E0E0" stroke-dasharray="3,3" />
      <text x="68" y="114" font-size="11" fill="#9E9E9E" text-anchor="end">
        {{ formattedMidVal }}
      </text>

      <line x1="75" y1="190" x2="785" y2="190" stroke="#E0E0E0" stroke-width="1.5" />
      <text x="68" y="194" font-size="11" fill="#9E9E9E" text-anchor="end">
        {{ formattedMinVal }}
      </text>

      <!-- Linha Média Tracejada (opcional) -->
      <line
        v-if="showAvgLine && avgY !== null"
        x1="75"
        :y1="avgY"
        x2="785"
        :y2="avgY"
        stroke="#FF9800"
        stroke-width="1.5"
        stroke-dasharray="4,4"
      />

      <!-- Linha Guia Vertical (Crosshair Indicator) -->
      <line
        v-if="activeHoverIndex !== null && crosshairX !== null"
        :x1="crosshairX"
        y1="30"
        :x2="crosshairX"
        y2="190"
        stroke="#38BDF8"
        stroke-dasharray="3,3"
        stroke-width="1.5"
      />

      <!-- Áreas sob as Curvas com Gradiente -->
      <g v-for="s in seriesList" :key="`area-${s.id}`">
        <polygon
          v-if="s.fillArea && s.areaPoints"
          :points="s.areaPoints"
          :fill="`url(#grad-${s.id})`"
        />
      </g>

      <!-- Linhas Principais dos Gráficos -->
      <polyline
        v-for="s in seriesList"
        :key="`line-${s.id}`"
        :points="s.polylinePoints"
        fill="none"
        :stroke="s.color"
        stroke-width="2.5"
        stroke-linecap="round"
      />

      <!-- Pontos de Amostragem (Data Circles) -->
      <g v-for="s in seriesList" :key="`points-${s.id}`">
        <circle
          v-for="(pt, idx) in s.chartPoints"
          :key="idx"
          :cx="pt.x"
          :cy="pt.y"
          :r="activeHoverIndex === idx ? 7 : 4"
          :fill="pt.color || s.color"
          stroke="#FFFFFF"
          stroke-width="2"
          class="chart-point"
        />
      </g>
    </svg>

    <div v-else class="text-center text-grey py-10">
      <v-icon size="40" color="grey-lighten-1">mdi-chart-line-variant</v-icon>
      <div class="mt-2 text-subtitle-2">Nenhum dado disponível para o gráfico.</div>
    </div>

    <!-- Tooltip Card Flutuante com Rastreamento Inteligente ao Lado do Mouse -->
    <v-card
      v-if="activeHoverIndex !== null && activeItems.length > 0 && mousePos"
      elevation="8"
      class="active-point-tooltip pa-3 rounded-lg border text-white pointer-events-none"
      :style="tooltipStyle"
    >
      <div
        v-for="(item, idx) in activeItems"
        :key="idx"
        class="font-weight-bold mb-1 d-flex align-center ga-2"
        style="font-size: 13px"
      >
        <span
          class="status-indicator-dot"
          :style="{ backgroundColor: item.color || '#2196F3' }"
        ></span>
        <span style="color: #38bdf8">{{ item.label || 'Valor' }}:</span>
        <span class="text-white">{{ item.formattedValue }}</span>
      </div>

      <div style="font-size: 11px; color: #cbd5e1" class="mt-1 d-flex align-center ga-1">
        <v-icon size="12" color="#94a3b8">mdi-clock-outline</v-icon>
        <span>Data e Hora: {{ activeItems[0]?.time }}</span>
      </div>
      <div v-if="activeItems[0]?.status" style="font-size: 11px; color: #94a3b8" class="mt-1">
        Status: {{ activeItems[0].status.toUpperCase() }}
      </div>
    </v-card>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, type CSSProperties } from 'vue'
import { formatBps, formatBytes, formatLatency } from '@/utils/formatters'

export interface ChartDataPoint {
  time: string
  value: number
  formattedValue?: string
  status?: string
  color?: string
}

export interface ChartSeriesInput {
  id: string
  label: string
  color: string
  fillArea?: boolean
  data: ChartDataPoint[]
}

const props = withDefaults(
  defineProps<{
    series: ChartSeriesInput[]
    unitType?: 'bandwidth' | 'bytes' | 'latency' | 'percentage' | 'generic'
    customUnit?: string
    showAvgLine?: boolean
    avgValue?: number
  }>(),
  {
    unitType: 'generic',
    customUnit: '',
    showAvgLine: false,
    avgValue: undefined,
  }
)

const chartContainerRef = ref<HTMLElement | null>(null)
const mousePos = ref<{ x: number; y: number } | null>(null)
const activeHoverIndex = ref<number | null>(null)

// Registra movimentação do mouse no container do gráfico
function handleMouseMove(e: MouseEvent) {
  if (!chartContainerRef.value) return
  const rect = chartContainerRef.value.getBoundingClientRect()
  const mouseX = e.clientX - rect.left
  const mouseY = e.clientY - rect.top

  mousePos.value = { x: mouseX, y: mouseY }

  const svgWidth = rect.width
  if (svgWidth <= 0) return

  const marginX = (75 / 800) * svgWidth
  const contentWidth = ((785 - 75) / 800) * svgWidth

  const pointCount = maxPointCount.value
  if (pointCount === 0) return

  let relX = mouseX - marginX
  if (relX < 0) relX = 0
  if (relX > contentWidth) relX = contentWidth

  const step = pointCount > 1 ? contentWidth / (pointCount - 1) : 0
  const index = step > 0 ? Math.round(relX / step) : 0

  activeHoverIndex.value = Math.min(pointCount - 1, Math.max(0, index))
}

function handleMouseLeave() {
  mousePos.value = null
  activeHoverIndex.value = null
}

// Coleta todos os pontos numéricos de todas as séries
const allValues = computed(() => {
  const vals: number[] = []
  for (const s of props.series) {
    for (const d of s.data) {
      if (d.value !== undefined && d.value !== null && !isNaN(d.value)) {
        vals.push(d.value)
      }
    }
  }
  return vals
})

const maxPointCount = computed(() => {
  let max = 0
  for (const s of props.series) {
    if (s.data.length > max) max = s.data.length
  }
  return max
})

const maxVal = computed(() => {
  if (allValues.value.length === 0) return 100
  const max = Math.max(...allValues.value)
  if (props.unitType === 'percentage') return Math.min(100, max > 0 ? max * 1.15 : 100)
  return max > 0 ? max * 1.15 : 100
})

const minVal = computed(() => {
  if (allValues.value.length === 0) return 0
  const min = Math.min(...allValues.value)
  return min < 0 ? min : 0
})

function getSvgY(val: number): number {
  const svgTop = 30
  const svgBottom = 190
  const svgHeight = svgBottom - svgTop
  const range = maxVal.value - minVal.value
  const ratio = range > 0 ? (val - minVal.value) / range : 0
  return svgBottom - ratio * svgHeight
}

const avgY = computed(() => {
  if (!props.showAvgLine || props.avgValue === undefined || props.avgValue === null) return null
  return getSvgY(props.avgValue)
})

function formatUnitValue(val: number): string {
  if (props.unitType === 'bandwidth') return formatBps(val)
  if (props.unitType === 'bytes') return formatBytes(val)
  if (props.unitType === 'latency') return formatLatency(val)
  if (props.unitType === 'percentage') return `${val.toFixed(1)}%`
  return `${val} ${props.customUnit}`.trim()
}

const formattedMaxVal = computed(() => formatUnitValue(maxVal.value))
const formattedMidVal = computed(() => formatUnitValue((maxVal.value + minVal.value) / 2))
const formattedMinVal = computed(() => formatUnitValue(minVal.value))

const seriesList = computed(() => {
  const svgLeft = 75
  const svgRight = 785
  const maxCount = maxPointCount.value

  return props.series.map((s) => {
    const list = s.data
    const step = maxCount > 1 ? (svgRight - svgLeft) / (maxCount - 1) : 0

    const chartPoints = list.map((d, idx) => {
      const x = maxCount === 1 ? (svgLeft + svgRight) / 2 : svgLeft + idx * step
      const y = getSvgY(d.value)
      return {
        x,
        y,
        value: d.value,
        formattedValue: d.formattedValue || formatUnitValue(d.value),
        time: d.time,
        status: d.status,
        color: d.color || s.color,
      }
    })

    const polylinePoints = chartPoints
      .map((pt) => `${pt.x.toFixed(1)},${pt.y.toFixed(1)}`)
      .join(' ')

    let areaPoints = ''
    if (chartPoints.length > 0) {
      const firstX = chartPoints[0].x.toFixed(1)
      const lastX = chartPoints[chartPoints.length - 1].x.toFixed(1)
      areaPoints = `${firstX},190 ${polylinePoints} ${lastX},190`
    }

    return {
      id: s.id,
      label: s.label,
      color: s.color,
      fillArea: s.fillArea !== false,
      chartPoints,
      polylinePoints,
      areaPoints,
    }
  })
})

const crosshairX = computed(() => {
  if (activeHoverIndex.value === null) return null
  const s = seriesList.value[0]
  if (!s || !s.chartPoints[activeHoverIndex.value]) return null
  return s.chartPoints[activeHoverIndex.value].x
})

const activeItems = computed(() => {
  if (activeHoverIndex.value === null) return []
  const idx = activeHoverIndex.value
  const items = []

  for (const s of seriesList.value) {
    const pt = s.chartPoints[idx]
    if (pt) {
      items.push({
        label: s.label,
        color: pt.color || s.color,
        formattedValue: pt.formattedValue,
        time: pt.time,
        status: pt.status,
      })
    }
  }
  return items
})

// Tooltip com Rastreamento Inteligente ao Lado do Cursor (Flips laterais e verticais)
const tooltipStyle = computed<CSSProperties>(() => {
  if (!mousePos.value || !chartContainerRef.value) return {}
  const { x, y } = mousePos.value
  const rect = chartContainerRef.value.getBoundingClientRect()

  const cardWidth = 230
  const cardHeight = 90

  // Se o mouse estiver próximo da borda direita, inverte o tooltip para a esquerda
  let left = x + 16
  if (x > rect.width - cardWidth - 20) {
    left = x - cardWidth - 16
  }

  // Se o mouse estiver na metade superior, coloca o tooltip abaixo do cursor para não cobrir
  let top = y - cardHeight - 16
  if (y < cardHeight + 20) {
    top = y + 16
  }

  left = Math.max(8, Math.min(rect.width - cardWidth - 8, left))
  top = Math.max(8, Math.min(rect.height - cardHeight - 8, top))

  return {
    position: 'absolute',
    left: `${left}px`,
    top: `${top}px`,
    pointerEvents: 'none',
    zIndex: 30,
    background: '#0F172A',
    borderColor: '#0284C7',
    boxShadow: '0 10px 25px -5px rgba(0, 0, 0, 0.5), 0 8px 10px -6px rgba(0, 0, 0, 0.5)',
    whiteSpace: 'nowrap',
    transition: 'left 0.08s ease-out, top 0.08s ease-out',
  }
})
</script>

<style scoped>
.base-metric-chart-container {
  position: relative;
  min-height: 250px;
  user-select: none;
}

.base-metric-svg {
  height: 240px;
  overflow: visible;
}

.chart-point {
  transition:
    r 0.15s ease,
    fill 0.15s ease;
}

.status-indicator-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  display: inline-block;
}

.pointer-events-none {
  pointer-events: none;
}
</style>
