<template>
  <span v-if="metrics.length > 0" class="health-chips">
    <v-chip
      v-for="metric in metrics"
      :key="metric.key"
      :color="HEALTH_LEVELS[metric.level].color"
      :prepend-icon="metric.level === 'critical' ? 'mdi-alert' : undefined"
      size="x-small"
      variant="tonal"
      label
      class="font-weight-bold"
      :title="HEALTH_LEVELS[metric.level].label"
    >
      {{ metric.label }} {{ metric.percent }}%
    </v-chip>
  </span>
</template>

<script setup lang="ts">
/** CPU e memória do dispositivo, cada um na cor da sua faixa de uso. */
import { computed } from 'vue'
import type { TopologyNode } from '@/stores/topology'
import { HEALTH_LEVELS, nodeHealthMetrics } from '@/utils/topology/presentation'

const props = defineProps<{
  node: Partial<Pick<TopologyNode, 'cpuPercent' | 'memoryPercent'>>
}>()

const metrics = computed(() => nodeHealthMetrics(props.node))
</script>

<style scoped>
.health-chips {
  display: inline-flex;
  flex-wrap: wrap;
  gap: 4px;
}
</style>
