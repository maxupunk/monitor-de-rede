<template>
  <div class="ai-tool-chart">
    <div class="d-flex align-center justify-space-between flex-wrap ga-2 mb-2">
      <div class="min-w-0">
        <div class="text-body-medium font-weight-bold text-truncate">{{ chart.title }}</div>
        <div v-if="chart.subtitle" class="text-body-small text-medium-emphasis">
          {{ chart.subtitle }}
        </div>
      </div>
      <div v-if="series.length > 1" class="d-flex align-center flex-wrap ga-3">
        <div
          v-for="item in series"
          :key="item.id"
          class="d-flex align-center ga-1 text-body-small text-medium-emphasis"
        >
          <span class="legend-dot" :style="{ backgroundColor: item.color }"></span>
          {{ item.label }}
        </div>
      </div>
    </div>

    <BaseMetricChart
      :series="series"
      :unit-type="unitType"
      :show-avg-line="chart.avgValue !== null"
      :avg-value="chart.avgValue ?? undefined"
    />
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import BaseMetricChart, { type ChartSeriesInput } from '@/components/BaseMetricChart.vue'
import type { AiChart } from '@/bindings/AiChart'
import type { AiChartUnit } from '@/bindings/AiChartUnit'
import { formatShortDateTime } from '@/utils/formatters'

const props = defineProps<{
  chart: AiChart
}>()

type UnitType = 'latency' | 'bandwidth' | 'percentage' | 'generic'

const UNIT_TYPES: Record<AiChartUnit, UnitType> = {
  latency: 'latency',
  bandwidth: 'bandwidth',
  percentage: 'percentage',
  generic: 'generic',
}

/** Mesmas cores do gráfico de tráfego da interface: entrada verde, saída azul. */
const SERIES_COLORS: Record<string, string> = {
  inBps: '#4CAF50',
  outBps: '#2196F3',
}

const PALETTE = ['#1976D2', '#FF9800', '#9C27B0', '#009688']

const unitType = computed<UnitType>(() => UNIT_TYPES[props.chart.unit])

const series = computed<ChartSeriesInput[]>(() => {
  const multiple = props.chart.series.length > 1
  return props.chart.series.map((item, index) => ({
    id: item.id,
    label: item.label,
    color: SERIES_COLORS[item.id] ?? PALETTE[index % PALETTE.length],
    fillArea: !multiple,
    data: item.points.map((point) => ({
      time: props.chart.xAxis === 'label' ? point.time : formatShortDateTime(point.time),
      value: point.value,
    })),
  }))
})
</script>

<style scoped>
.legend-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  display: inline-block;
}
.min-w-0 {
  min-width: 0;
}
</style>
