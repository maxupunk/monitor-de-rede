<template>
  <div class="live-badges" :style="layerStyle" aria-hidden="true">
    <template v-for="badge in visible" :key="badge.id">
      <div
        v-if="badge.kind === 'traffic'"
        class="badge traffic"
        :style="boxStyle(badge, badge.width + 2, badge.height + 2)"
      >
        <span class="rate rate-in">{{ DOWN.arrow }} {{ formatBps(badge.inBps) }}</span>
        <span class="rate rate-out">{{ UP.arrow }} {{ formatBps(badge.outBps) }}</span>
      </div>
      <div v-else class="badge health" :style="boxStyle(badge, badge.width, badge.height + 1)">
        <span
          v-for="metric in badge.metrics"
          :key="metric.key"
          class="metric"
          :class="`level-${metric.level}`"
        >
          <span v-if="metric.level === 'critical'" class="warn">⚠</span>
          {{ metric.label }} {{ metric.percent }}%
        </span>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
/**
 * Camada HTML dos selos coloridos (`utils/topology/liveBadges`): posicionada
 * nas coordenadas do modelo e levada junto com o mapa por uma única
 * transformação (pan e zoom) — mover o mapa não recalcula nenhum selo. Não
 * recebe clique: o mouse continua falando com o Cytoscape por baixo.
 */
import { computed } from 'vue'
import { formatBps } from '@/utils/formatters'
import type { LiveBadge } from '@/utils/topology/liveBadges'
import { TRAFFIC_DIRECTIONS } from '@/utils/topology/presentation'
import type { LiveColors } from '@/utils/topology/style'

const props = defineProps<{
  badges: readonly LiveBadge[]
  zoom: number
  pan: { x: number; y: number }
  colors: LiveColors
}>()

const DOWN = TRAFFIC_DIRECTIONS.in
const UP = TRAFFIC_DIRECTIONS.out

/** Some junto com o rótulo do Cytoscape quando a fonte fica pequena demais. */
const visible = computed(() =>
  props.badges.filter((badge) => badge.fontSize * props.zoom >= badge.minFont)
)

const layerStyle = computed(() => ({
  transform: `translate(${props.pan.x}px, ${props.pan.y}px) scale(${props.zoom})`,
  '--live-bg': props.colors.labelBackground,
  '--live-border': props.colors.labelBorder,
  '--live-text': props.colors.text,
  '--live-in': props.colors.trafficIn,
  '--live-out': props.colors.trafficOut,
  '--live-normal': props.colors.health.normal,
  '--live-high': props.colors.health.high,
  '--live-critical': props.colors.health.critical,
}))

function boxStyle(badge: LiveBadge, width: number, height: number) {
  return {
    left: `${badge.x}px`,
    top: `${badge.y}px`,
    minWidth: `${width}px`,
    height: `${height}px`,
    fontSize: `${badge.fontSize}px`,
    opacity: badge.opacity,
  }
}
</script>

<style scoped>
.live-badges {
  position: absolute;
  inset: 0 auto auto 0;
  width: 0;
  height: 0;
  transform-origin: 0 0;
  pointer-events: none;
}
.badge {
  position: absolute;
  transform: translate(-50%, -50%);
  display: flex;
  align-items: center;
  justify-content: center;
  box-sizing: border-box;
  white-space: nowrap;
  font-family: Roboto, system-ui, sans-serif;
  font-weight: 700;
  line-height: 1;
  background: var(--live-bg);
  color: var(--live-text);
  transition: opacity 150ms ease;
}
.traffic {
  gap: 0.6em;
  padding: 0 0.3em;
  border: 1px solid var(--live-border);
  border-radius: 4px;
}
.rate {
  transition: color 300ms ease;
}
.rate-in {
  color: var(--live-in);
}
.rate-out {
  color: var(--live-out);
}
.health {
  gap: 0.35em;
}
.metric {
  display: inline-flex;
  align-items: center;
  gap: 0.2em;
  padding: 0.1em 0.35em;
  border-radius: 999px;
  color: var(--metric);
  background: color-mix(in srgb, var(--metric) 18%, transparent);
  transition:
    color 300ms ease,
    background-color 300ms ease;
}
.level-normal {
  --metric: var(--live-normal);
}
.level-high {
  --metric: var(--live-high);
}
.level-critical {
  --metric: var(--live-critical);
  animation: metric-alert 1.6s ease-in-out infinite;
}
@keyframes metric-alert {
  50% {
    background: color-mix(in srgb, var(--metric) 38%, transparent);
  }
}
@media (prefers-reduced-motion: reduce) {
  .level-critical {
    animation: none;
  }
}
</style>
