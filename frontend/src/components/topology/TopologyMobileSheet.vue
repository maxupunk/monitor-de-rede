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
        <v-tab value="layout">Organizar</v-tab>
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
          <v-window-item value="layout">
            <TopologyLayoutList
              :can-write="canWrite"
              :new-count="newCount"
              @layout="onLayout"
              @arrange-new="onArrangeNew"
            />
            <template v-if="canWrite">
              <v-divider class="my-1"></v-divider>
              <TopologyAddList @add="onAdd" />
            </template>
          </v-window-item>
          <v-window-item value="view">
            <TopologyViewOptions
              :legend-open="legendOpen"
              :group-by-site="groupBySite"
              :always-detail="alwaysDetail"
              @update:legend-open="$emit('update:legendOpen', $event)"
              @update:group-by-site="$emit('update:groupBySite', $event)"
              @update:always-detail="$emit('update:alwaysDetail', $event)"
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
import type { LayoutKind } from '@/utils/topology/layouts'
import TopologyFilterPanel from './TopologyFilterPanel.vue'
import TopologyLayoutList from './TopologyLayoutList.vue'
import TopologyViewOptions from './TopologyViewOptions.vue'
import TopologyAddList, { type AddKind } from './TopologyAddList.vue'

defineProps<{
  modelValue: boolean
  nodes: readonly TopologyNode[]
  filters: TopologyFilters
  canWrite: boolean
  newCount: number
  legendOpen: boolean
  groupBySite: boolean
  alwaysDetail: boolean
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  'update:filters': [value: TopologyFilters]
  'update:legendOpen': [value: boolean]
  'update:groupBySite': [value: boolean]
  'update:alwaysDetail': [value: boolean]
  layout: [kind: LayoutKind]
  'arrange-new': []
  add: [kind: AddKind]
  export: []
}>()

const tab = ref<'filters' | 'layout' | 'view'>('filters')

function close() {
  emit('update:modelValue', false)
}
function onLayout(kind: LayoutKind) {
  close()
  emit('layout', kind)
}
function onAdd(kind: AddKind) {
  close()
  emit('add', kind)
}
function onArrangeNew() {
  close()
  emit('arrange-new')
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
