<template>
  <v-card class="topology-legend" elevation="6" border>
    <div class="d-flex align-center px-3 pt-1">
      <v-icon size="18" color="primary" class="mr-1">mdi-map-legend</v-icon>
      <span class="text-subtitle-2 font-weight-bold">Legenda</span>
      <v-spacer></v-spacer>
      <v-btn
        icon="mdi-chevron-up"
        size="small"
        variant="text"
        color="primary"
        aria-label="Recolher legenda (L)"
        @click="$emit('close')"
      ></v-btn>
    </div>
    <div class="px-3 pb-3 legend-body">
      <div class="legend-title">Dispositivo</div>
      <div class="legend-grid">
        <div v-for="(status, key) in NODE_STATUS" :key="key" class="legend-row">
          <span
            class="ring"
            :class="{ dashed: key === 'unknown' }"
            :style="{ borderColor: `rgb(var(--v-theme-${status.color}))` }"
          ></span>
          {{ status.label }}
        </div>
        <div class="legend-row">
          <span class="ring system"></span>
          Servidor
        </div>
      </div>

      <div class="legend-title">Enlace — estado e uso</div>
      <div class="legend-grid">
        <div class="legend-row">
          <svg width="26" height="10" aria-hidden="true">
            <line x1="2" y1="5" x2="24" y2="5" class="neutral" stroke-width="3" />
          </svg>
          Ativo, sem medição
        </div>
        <div v-for="(level, key) in UTILIZATION_LEVELS" :key="key" class="legend-row">
          <svg width="26" height="10" aria-hidden="true">
            <line
              x1="2"
              y1="5"
              x2="24"
              y2="5"
              stroke-width="3"
              :style="{ stroke: `rgb(var(--v-theme-${level.color}))` }"
            />
          </svg>
          {{ level.label }}
        </div>
        <div class="legend-row">
          <svg width="26" height="10" aria-hidden="true">
            <line x1="2" y1="5" x2="24" y2="5" stroke-width="3" class="down" />
            <line x1="13" y1="0" x2="13" y2="10" stroke-width="2.5" class="down" />
          </svg>
          Fora do ar
        </div>
      </div>

      <div class="legend-title">Enlace — tipo</div>
      <div class="legend-grid">
        <div v-for="(kind, key) in LINK_KINDS" :key="key" class="legend-row">
          <svg width="26" height="10" aria-hidden="true">
            <line
              v-if="key === 'fiber'"
              x1="2"
              y1="5"
              x2="24"
              y2="5"
              stroke-width="6"
              class="fiber-outline"
            />
            <line
              x1="2"
              y1="5"
              x2="24"
              y2="5"
              stroke-width="2.5"
              class="neutral"
              :stroke-dasharray="DASH[key]"
            />
          </svg>
          {{ kind.label }}
        </div>
        <div class="legend-row">
          <svg width="26" height="10" aria-hidden="true">
            <line
              x1="2"
              y1="5"
              x2="24"
              y2="5"
              stroke-width="2.5"
              class="neutral stale"
              stroke-dasharray="1 4"
            />
          </svg>
          Defasado
        </div>
      </div>
      <div class="legend-hint">Espessura = velocidade (≤100M, 1G, ≥10G)</div>

      <div class="legend-title">Tráfego e saúde</div>
      <div class="legend-grid">
        <div v-for="(direction, key) in TRAFFIC_DIRECTIONS" :key="key" class="legend-row">
          <v-icon size="16" :color="direction.color">{{ direction.icon }}</v-icon>
          {{ direction.label }}
        </div>
        <div v-for="(level, key) in HEALTH_LEVELS" :key="key" class="legend-row">
          <span
            class="chip"
            :style="{
              color: `rgb(var(--v-theme-${level.color}))`,
              background: `rgba(var(--v-theme-${level.color}), 0.18)`,
            }"
            >CPU</span
          >
          {{ level.label }}
        </div>
      </div>
    </div>
  </v-card>
</template>

<script setup lang="ts">
import { TOPOLOGY_LEGEND_WIDTH } from '@/utils/topology/canvasTypes'
import {
  HEALTH_LEVELS,
  LINK_KINDS,
  NODE_STATUS,
  TRAFFIC_DIRECTIONS,
  UTILIZATION_LEVELS,
  type LinkKind,
} from '@/utils/topology/presentation'

defineEmits<{ close: [] }>()

/** Largura do cartão — a página desconta do enquadramento quando aberto. */
const LEGEND_WIDTH = TOPOLOGY_LEGEND_WIDTH

/** Mesmos traços de `style.ts` (padrão de tracejado do canvas). */
const DASH: Record<LinkKind, string | undefined> = {
  ethernet: undefined,
  fiber: undefined,
  wireless: '7 5',
  vpn: '14 6',
  inferred: '4 6',
  parent: '2 6',
}
</script>

<style scoped>
.topology-legend {
  width: v-bind('`${LEGEND_WIDTH}px`');
  max-height: calc(100% - 24px);
  overflow-y: auto;
  background: rgb(var(--v-theme-surface));
}
.legend-body {
  font-size: 0.75rem;
  color: rgb(var(--v-theme-on-surface));
}
.legend-title {
  font-weight: 700;
  font-size: 0.7rem;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  margin: 8px 0 2px;
}
.legend-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  column-gap: 10px;
}
.legend-row {
  display: flex;
  align-items: center;
  gap: 6px;
  min-height: 20px;
  white-space: nowrap;
}
.ring {
  width: 16px;
  height: 16px;
  border-radius: 50%;
  border: 3px solid;
  background: rgb(var(--v-theme-surface));
  flex-shrink: 0;
}
.ring.dashed {
  border-style: dashed;
}
.ring.system {
  border-radius: 4px;
  border-color: rgb(var(--v-theme-primary));
  background: rgb(var(--v-theme-primary));
}
svg {
  flex-shrink: 0;
}
.neutral {
  stroke: rgba(var(--v-theme-on-surface), 0.6);
}
.down {
  stroke: rgb(var(--v-theme-error));
}
.fiber-outline {
  stroke: rgb(var(--v-theme-secondary));
}
.stale {
  opacity: 0.6;
}
.chip {
  font-size: 0.62rem;
  font-weight: 700;
  padding: 0 4px;
  border-radius: 999px;
  flex-shrink: 0;
}
.legend-hint {
  margin-top: 8px;
  font-style: italic;
}
</style>
