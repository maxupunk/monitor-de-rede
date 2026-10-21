<template>
  <v-bottom-sheet
    :model-value="modelValue"
    scrollable
    @update:model-value="$emit('update:modelValue', $event)"
  >
    <v-card class="mobile-sheet">
      <v-card-title class="d-flex align-center">
        <v-icon color="primary" class="mr-2">mdi-tune-variant</v-icon>
        Opções do mapa
        <v-spacer></v-spacer>
        <v-btn
          icon="mdi-close"
          variant="text"
          aria-label="Fechar"
          @click="$emit('update:modelValue', false)"
        ></v-btn>
      </v-card-title>
      <v-tabs v-model="tab" color="primary" grow density="compact">
        <v-tab value="filters">Filtros</v-tab>
        <v-tab value="view">Exibir</v-tab>
      </v-tabs>
      <v-divider></v-divider>
      <v-card-text class="pa-3">
        <v-window v-model="tab">
          <v-window-item value="filters">
            <TopologyFilterPanel
              :model-value="filters"
              :nodes="nodes"
              @update:model-value="$emit('update:filters', $event)"
            />
          </v-window-item>
          <v-window-item value="view">
            <TopologyViewOptions
              :legend-open="legendOpen"
              :group-by-site="groupBySite"
              :always-detail="alwaysDetail"
              :orthogonal-edges="orthogonalEdges"
              :show-health="showHealth"
              @update:show-health="$emit('update:showHealth', $event)"
              @update:legend-open="$emit('update:legendOpen', $event)"
              @update:group-by-site="$emit('update:groupBySite', $event)"
              @update:always-detail="$emit('update:alwaysDetail', $event)"
              @update:orthogonal-edges="$emit('update:orthogonalEdges', $event)"
            />
            <div class="d-flex flex-wrap ga-2 pa-2">
              <v-btn color="info" variant="flat" prepend-icon="mdi-image-move" @click="onExport">
                Exportar PNG
              </v-btn>
            </div>
          </v-window-item>
        </v-window>
      </v-card-text>
    </v-card>
  </v-bottom-sheet>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import type { TopologyNode } from '@/stores/topology'
import type { TopologyFilters } from '@/utils/topology/filters'
import TopologyFilterPanel from './TopologyFilterPanel.vue'
import TopologyViewOptions from './TopologyViewOptions.vue'

defineProps<{
  modelValue: boolean
  nodes: readonly TopologyNode[]
  filters: TopologyFilters
  legendOpen: boolean
  groupBySite: boolean
  alwaysDetail: boolean
  orthogonalEdges: boolean
  showHealth: boolean
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  'update:filters': [value: TopologyFilters]
  'update:legendOpen': [value: boolean]
  'update:groupBySite': [value: boolean]
  'update:alwaysDetail': [value: boolean]
  'update:orthogonalEdges': [value: boolean]
  'update:showHealth': [value: boolean]
  export: []
}>()

const tab = ref<'filters' | 'view'>('filters')

function close() {
  emit('update:modelValue', false)
}
function onExport() {
  close()
  emit('export')
}
</script>

<style scoped>
.mobile-sheet {
  max-height: 82vh;
}
</style>
