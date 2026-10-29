<template>
  <span class="traffic-rates" :class="{ dense }">
    <span
      v-for="direction in DIRECTIONS"
      :key="direction.key"
      class="rate"
      :title="direction.meta.label"
    >
      <v-icon :size="dense ? 14 : 16" :color="direction.meta.color">{{
        direction.meta.icon
      }}</v-icon>
      <span :class="`text-${direction.meta.color}`">{{ formatBps(direction.bps ?? 0) }}</span>
    </span>
  </span>
</template>

<script setup lang="ts">
/**
 * Download e upload de um enlace, cada um na cor do seu sentido — a mesma do
 * selo no mapa e da legenda (`TRAFFIC_DIRECTIONS`).
 */
import { computed } from 'vue'
import { formatBps } from '@/utils/formatters'
import { TRAFFIC_DIRECTIONS } from '@/utils/topology/presentation'

const props = defineProps<{
  inBps: number | null | undefined
  outBps: number | null | undefined
  /** Tamanho de tooltip. */
  dense?: boolean
}>()

const DIRECTIONS = computed(() => [
  { key: 'in', meta: TRAFFIC_DIRECTIONS.in, bps: props.inBps },
  { key: 'out', meta: TRAFFIC_DIRECTIONS.out, bps: props.outBps },
])
</script>

<style scoped>
.traffic-rates {
  display: inline-flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 4px 12px;
}
.rate {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}
</style>
