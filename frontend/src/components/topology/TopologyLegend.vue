<template>
  <v-card class="topology-legend" elevation="6" border>
    <div class="d-flex align-center px-3 pt-2">
      <v-icon size="18" color="primary" class="mr-1">mdi-map-legend</v-icon>
      <span class="text-subtitle-2 font-weight-bold">Legenda</span>
      <v-spacer></v-spacer>
      <v-btn
        icon="mdi-close"
        size="x-small"
        variant="text"
        aria-label="Esconder legenda (L)"
        @click="$emit('close')"
      ></v-btn>
    </div>
    <div class="px-3 pb-3 legend-body">
      <div class="legend-title">Dispositivo</div>
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
        Servidor NetMonitor
      </div>

      <div class="legend-title">Enlace — estado e uso</div>
      <div class="legend-row">
        <svg width="34" height="10" aria-hidden="true">
          <line x1="2" y1="5" x2="32" y2="5" class="neutral" stroke-width="3" />
        </svg>
        Ativo, sem medição
      </div>
      <div v-for="(level, key) in UTILIZATION_LEVELS" :key="key" class="legend-row">
        <svg width="34" height="10" aria-hidden="true">
          <line
            x1="2"
            y1="5"
            x2="32"
            y2="5"
            stroke-width="3"
            :style="{ stroke: `rgb(var(--v-theme-${level.color}))` }"
          />
        </svg>
        {{ level.label }}
      </div>
      <div class="legend-row">
        <svg width="34" height="10" aria-hidden="true">
          <line x1="2" y1="5" x2="32" y2="5" stroke-width="3" class="down" />
          <line x1="17" y1="0" x2="17" y2="10" stroke-width="2.5" class="down" />
        </svg>
        Fora do ar
      </div>

      <div class="legend-title">Enlace — tipo</div>
      <div v-for="(kind, key) in LINK_KINDS" :key="key" class="legend-row">
        <svg width="34" height="10" aria-hidden="true">
          <line
            v-if="key === 'fiber'"
            x1="2"
            y1="5"
            x2="32"
            y2="5"
            stroke-width="6"
            class="fiber-outline"
          />
          <line
            x1="2"
            y1="5"
            x2="32"
            y2="5"
            stroke-width="2.5"
            class="neutral"
            :stroke-dasharray="DASH[key]"
          />
        </svg>
        {{ kind.label }}
      </div>
      <div class="legend-row">
        <svg width="34" height="10" aria-hidden="true">
          <line
            x1="2"
            y1="5"
            x2="32"
            y2="5"
            stroke-width="2.5"
            class="neutral stale"
            stroke-dasharray="1 4"
          />
        </svg>
        Não visto há algum tempo
      </div>
      <div class="legend-hint">Espessura = velocidade (≤100M, 1G, ≥10G)</div>
    </div>
  </v-card>
</template>

<script setup lang="ts">
import {
  LINK_KINDS,
  NODE_STATUS,
  UTILIZATION_LEVELS,
  type LinkKind,
} from '@/utils/topology/presentation'

defineEmits<{ close: [] }>()

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
  width: 214px;
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
.legend-row {
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 18px;
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
.legend-hint {
  margin-top: 8px;
  font-style: italic;
}
</style>
