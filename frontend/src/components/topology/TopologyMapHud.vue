<template>
  <div v-if="showPanels" class="float top-left">
    <TopologyLegend v-if="legendOpen" @close="$emit('update:legendOpen', false)" />
    <v-btn
      v-else
      color="primary"
      variant="flat"
      size="small"
      prepend-icon="mdi-map-legend"
      @click="$emit('update:legendOpen', true)"
    >
      Legenda
    </v-btn>
  </div>
  <div
    class="float top-center"
    :class="{
      'beside-legend': showPanels && legendOpen,
      'beside-button': showPanels && !legendOpen,
    }"
  >
    <TopologyEditBar
      v-if="editMode"
      :cable-active="cableActive"
      :cable-source-name="cableSourceName"
      @cable="$emit('cable')"
      @cancel-cable="$emit('cancel-cable')"
      @done="$emit('edit-done')"
    />
    <v-chip
      v-if="highlighted"
      color="primary"
      variant="flat"
      closable
      prepend-icon="mdi-map-marker-path"
      @click:close="$emit('clear-highlight')"
    >
      Caminho destacado
    </v-chip>
    <v-chip v-if="computing" color="info" variant="flat" prepend-icon="mdi-auto-fix">
      Organizando o mapa…
    </v-chip>
  </div>
  <div v-if="showPanels" class="float bottom-left">
    <TopologyMinimap :source="minimapSource" :palette="palette" />
  </div>
  <template v-if="hasNodes">
    <div class="float bottom-center">
      <TopologyStatusBar
        :summary="summary"
        :problems-only="problemsOnly"
        @toggle-problems="$emit('toggle-problems')"
      />
    </div>
    <div class="float bottom-right">
      <TopologyZoomControls
        :zoom="zoom"
        @zoom-in="$emit('zoom', 1.25)"
        @zoom-out="$emit('zoom', 0.8)"
        @fit="$emit('fit')"
        @reset="$emit('reset-zoom')"
      />
    </div>
  </template>
</template>

<script setup lang="ts">
/**
 * Painéis flutuantes sobre o mapa (legenda, barra de edição, minimapa,
 * resumo e zoom). Só posiciona e repassa eventos — o estado é da página.
 */
import { computed } from 'vue'
import type { MinimapSource } from '@/utils/topology/canvasTypes'
import type { TopologyPalette } from '@/utils/topology/style'
import TopologyLegend from './TopologyLegend.vue'
import TopologyEditBar from './TopologyEditBar.vue'
import TopologyMinimap from './TopologyMinimap.vue'
import TopologyStatusBar, { type TopologySummary } from './TopologyStatusBar.vue'
import TopologyZoomControls from './TopologyZoomControls.vue'

const props = defineProps<{
  hasNodes: boolean
  /** Celular: sem legenda e minimapa flutuantes (a tela é do mapa). */
  narrow: boolean
  legendOpen: boolean
  editMode: boolean
  cableActive: boolean
  cableSourceName: string | null
  highlighted: boolean
  computing: boolean
  minimapSource: MinimapSource | null
  palette: TopologyPalette
  summary: TopologySummary
  problemsOnly: boolean
  zoom: number
}>()

defineEmits<{
  'update:legendOpen': [value: boolean]
  cable: []
  'cancel-cable': []
  'edit-done': []
  'clear-highlight': []
  'toggle-problems': []
  zoom: [factor: number]
  fit: []
  'reset-zoom': []
}>()

const showPanels = computed(() => props.hasNodes && !props.narrow)
</script>

<style scoped>
.float {
  position: absolute;
  z-index: 5;
  display: flex;
  gap: 8px;
  pointer-events: none;
}
.float > * {
  pointer-events: auto;
}
.top-left {
  top: 12px;
  left: 12px;
  max-height: calc(100% - 164px);
  align-items: flex-start;
}
.top-center {
  top: 12px;
  left: 12px;
  right: 12px;
  flex-direction: column;
  align-items: center;
}
/* Não cobre o botão nem o cartão da legenda. */
.top-center.beside-button {
  left: 132px;
}
.top-center.beside-legend {
  left: 240px;
}
.bottom-left {
  bottom: 12px;
  left: 12px;
}
.bottom-center {
  bottom: 12px;
  left: 210px;
  right: 76px;
  justify-content: center;
}
.bottom-right {
  bottom: 12px;
  right: 12px;
}
@media (max-width: 959px) {
  .bottom-center {
    left: 12px;
    justify-content: flex-start;
  }
}
</style>
