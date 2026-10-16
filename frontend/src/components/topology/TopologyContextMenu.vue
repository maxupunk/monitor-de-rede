<template>
  <v-menu
    :model-value="modelValue"
    :target="[x, y]"
    location="end top"
    :close-on-content-click="true"
    max-height="70vh"
    @update:model-value="$emit('update:modelValue', $event)"
  >
    <v-card min-width="260" max-width="340" elevation="10" border>
      <div class="menu-header px-4 py-2 d-flex align-center ga-2">
        <v-icon :color="headerColor" size="20">{{ headerIcon }}</v-icon>
        <span class="font-weight-bold text-truncate">{{ title }}</span>
      </div>
      <v-divider></v-divider>
      <TopologyActionList :actions="actions" dense @ran="$emit('update:modelValue', false)" />
    </v-card>
  </v-menu>
</template>

<script setup lang="ts">
/** Menu de contexto (botão direito / toque longo) — as mesmas ações do inspetor. */
import { computed } from 'vue'
import {
  resolveActions,
  type TopologyHost,
  type TopologyTarget,
} from '@/utils/topology/inspectorRegistry'
import {
  edgeStatusKey,
  EDGE_STATUS,
  nodeIcon,
  nodeStatusPresentation,
} from '@/utils/topology/presentation'
import TopologyActionList from './TopologyActionList.vue'

const props = defineProps<{
  modelValue: boolean
  target: TopologyTarget | null
  host: TopologyHost
  x: number
  y: number
}>()

defineEmits<{ 'update:modelValue': [value: boolean] }>()

const actions = computed(() => (props.target ? resolveActions(props.target, props.host) : []))

const title = computed(() => {
  const target = props.target
  if (!target || target.kind === 'background') return 'Mapa'
  if (target.kind === 'node') return target.node.name
  return `${target.edge.sourceDeviceName || 'Origem'} ↔ ${target.edge.targetDeviceName || 'Destino'}`
})

const headerIcon = computed(() => {
  const target = props.target
  if (!target || target.kind === 'background') return 'mdi-sitemap'
  if (target.kind === 'node') return nodeIcon(target.node)
  return 'mdi-transit-connection-variant'
})

const headerColor = computed(() => {
  const target = props.target
  if (!target || target.kind === 'background') return 'primary'
  if (target.kind === 'node') return nodeStatusPresentation(target.node.status).color
  return EDGE_STATUS[edgeStatusKey(target.edge)].color
})
</script>

<style scoped>
.menu-header {
  color: rgb(var(--v-theme-on-surface));
}
</style>
