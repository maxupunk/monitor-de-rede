<template>
  <div class="chart-tooltip-card" :style="positionStyle">
    <div v-for="row in rows" :key="row.label" class="chart-tooltip-card__row">
      <span class="chart-tooltip-card__swatch" :style="{ backgroundColor: row.color }"></span>
      <span class="chart-tooltip-card__label">{{ row.label }}</span>
      <span class="chart-tooltip-card__value">{{ row.value }}</span>
    </div>
    <div v-if="footer" class="chart-tooltip-card__footer">{{ footer }}</div>
  </div>
</template>

<script setup lang="ts">
import type { CSSProperties } from 'vue'

/** Uma linha do tooltip: a cor identifica a série, o rótulo a nomeia. */
export interface ChartTooltipRow {
  label: string
  value: string
  color: string
}

/**
 * Cartão único de tooltip de todos os gráficos: mesmo fundo, tipografia e
 * ordem (séries, depois o instante) em qualquer tela.
 */
defineProps<{
  rows: ChartTooltipRow[]
  footer?: string
  positionStyle?: CSSProperties
}>()
</script>

<style scoped>
.chart-tooltip-card {
  position: absolute;
  z-index: 30;
  pointer-events: none;
  padding: 8px 10px;
  border-radius: 8px;
  background: #0f172a;
  color: #f8fafc;
  font-size: 12px;
  line-height: 1.45;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
}

.chart-tooltip-card__row {
  display: flex;
  align-items: center;
  gap: 8px;
  white-space: nowrap;
}

.chart-tooltip-card__swatch {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex: 0 0 auto;
}

.chart-tooltip-card__label {
  color: #cbd5e1;
}

.chart-tooltip-card__value {
  margin-left: auto;
  padding-left: 12px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
}

.chart-tooltip-card__footer {
  margin-top: 4px;
  color: #94a3b8;
  font-size: 11px;
}
</style>
