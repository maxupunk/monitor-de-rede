<template>
  <div class="traffic">
    <template v-if="measured">
      <div class="d-flex align-center flex-wrap ga-3 rates">
        <span class="rate">
          <v-icon size="16" color="success">mdi-arrow-down-bold</v-icon>
          {{ formatBps(edge.inBps ?? 0) }}
        </span>
        <span class="rate">
          <v-icon size="16" color="info">mdi-arrow-up-bold</v-icon>
          {{ formatBps(edge.outBps ?? 0) }}
        </span>
        <v-spacer></v-spacer>
        <span class="speed">{{ edgeSpeedLabel(edge) }}</span>
      </div>
      <template v-if="utilization !== null && level">
        <v-progress-linear
          :model-value="utilization * 100"
          :color="UTILIZATION_LEVELS[level].color"
          bg-color="on-surface"
          bg-opacity="0.15"
          height="8"
          rounded
          class="mt-2"
          :aria-label="`Uso do enlace: ${formatPercent(utilization * 100, 0)}`"
        ></v-progress-linear>
        <div class="usage mt-1">
          Uso {{ formatPercent(utilization * 100, 0) }} · {{ UTILIZATION_LEVELS[level].label }}
        </div>
      </template>
    </template>
    <div v-else class="d-flex align-center ga-2 no-data">
      <v-icon size="16" color="info">mdi-chart-line-variant</v-icon>
      Sem medição de tráfego · {{ edgeSpeedLabel(edge) }}
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { TopologyEdge } from '@/stores/topology'
import { formatBps, formatPercent } from '@/utils/formatters'
import {
  edgeSpeedLabel,
  edgeUtilization,
  hasTraffic,
  UTILIZATION_LEVELS,
  utilizationLevel,
} from '@/utils/topology/presentation'

const props = defineProps<{ edge: TopologyEdge }>()

const measured = computed(() => hasTraffic(props.edge))
const utilization = computed(() => edgeUtilization(props.edge))
const level = computed(() => utilizationLevel(utilization.value))
</script>

<style scoped>
.traffic {
  font-size: 0.85rem;
  color: rgb(var(--v-theme-on-surface));
}
.rate {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  font-weight: 700;
}
.speed,
.usage,
.no-data {
  font-size: 0.78rem;
  font-weight: 600;
}
</style>
