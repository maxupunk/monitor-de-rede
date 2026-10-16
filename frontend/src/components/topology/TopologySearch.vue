<template>
  <v-dialog
    :model-value="modelValue"
    max-width="560"
    :fullscreen="xs"
    location="top"
    class="topology-search"
    @update:model-value="$emit('update:modelValue', $event)"
  >
    <v-card class="search-card">
      <div class="d-flex align-center pa-3 ga-2">
        <v-text-field
          ref="field"
          v-model="query"
          autofocus
          placeholder="Nome, IP, fabricante, modelo ou site…"
          prepend-inner-icon="mdi-magnify"
          variant="outlined"
          density="comfortable"
          hide-details
          clearable
          aria-label="Buscar dispositivo no mapa"
          @keydown.down.prevent="move(1)"
          @keydown.up.prevent="move(-1)"
          @keydown.enter.prevent="choose(active)"
        ></v-text-field>
        <v-btn
          icon="mdi-close"
          variant="text"
          aria-label="Fechar busca"
          @click="$emit('update:modelValue', false)"
        ></v-btn>
      </div>
      <v-divider></v-divider>
      <v-list v-if="hits.length > 0" density="comfortable" class="results py-1" role="listbox">
        <v-list-item
          v-for="(hit, index) in hits"
          :key="hit.node.id"
          :active="index === active"
          color="primary"
          role="option"
          :aria-selected="index === active"
          :title="hit.node.name"
          :subtitle="subtitleOf(hit)"
          @click="choose(index)"
          @mouseenter="active = index"
        >
          <template #prepend>
            <v-avatar :color="statusOf(hit.node).color" size="34" class="mr-1">
              <v-icon color="white" size="20">{{ nodeIcon(hit.node) }}</v-icon>
            </v-avatar>
          </template>
          <template #append>
            <v-chip :color="statusOf(hit.node).color" size="x-small" variant="flat" label>
              {{ statusOf(hit.node).label }}
            </v-chip>
          </template>
        </v-list-item>
      </v-list>
      <div v-else-if="query.trim()" class="empty pa-8 text-center">
        <v-icon size="40" color="warning" class="mb-2">mdi-magnify-close</v-icon>
        <div class="text-subtitle-1 font-weight-bold">Nada encontrado</div>
        <div class="text-body-2">
          Tente parte do nome, o começo do IP (ex.: 10.0.1) ou o fabricante.
        </div>
      </div>
      <div v-else class="empty pa-6 text-body-2">
        <v-icon color="info" class="mr-1">mdi-keyboard</v-icon>
        Use <kbd>↑</kbd> <kbd>↓</kbd> para navegar e <kbd>Enter</kbd> para ir até o dispositivo.
      </div>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useDisplay } from 'vuetify'
import type { TopologyNode } from '@/stores/topology'
import { searchNodes, type SearchHit } from '@/utils/topology/search'
import {
  nodeIcon,
  nodeIp,
  nodeStatusPresentation,
  nodeTypeLabel,
} from '@/utils/topology/presentation'

const props = defineProps<{ modelValue: boolean; nodes: readonly TopologyNode[] }>()
const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  select: [deviceId: number]
}>()

const { xs } = useDisplay()
const query = ref('')
const active = ref(0)

const hits = computed(() => searchNodes(props.nodes, query.value ?? '', 12))

watch(hits, () => {
  active.value = 0
})
watch(
  () => props.modelValue,
  (open) => {
    if (open) query.value = ''
  }
)

const FIELD_LABEL: Record<SearchHit['matchedField'], string> = {
  name: '',
  ip: 'IP',
  vendor: 'Fabricante',
  model: 'Modelo',
  type: 'Tipo',
  site: 'Site',
}

function subtitleOf(hit: SearchHit): string {
  const parts = [nodeIp(hit.node), nodeTypeLabel(hit.node)]
  if (hit.node.vendor) parts.push(hit.node.vendor)
  if (hit.node.siteName) parts.push(hit.node.siteName)
  const via = FIELD_LABEL[hit.matchedField]
  return parts.filter(Boolean).join(' · ') + (via ? ` — por ${via}` : '')
}

function statusOf(node: TopologyNode) {
  return nodeStatusPresentation(node.status)
}

function move(delta: number) {
  if (hits.value.length === 0) return
  active.value = (active.value + delta + hits.value.length) % hits.value.length
}

function choose(index: number) {
  const hit = hits.value[index]
  if (!hit) return
  emit('select', hit.node.id)
  emit('update:modelValue', false)
}
</script>

<style scoped>
.search-card {
  margin-top: 8vh;
}
.results {
  max-height: 60vh;
  overflow-y: auto;
}
.empty {
  color: rgb(var(--v-theme-on-surface));
}
kbd {
  font-family: inherit;
  font-size: 0.75rem;
  font-weight: 700;
  padding: 1px 6px;
  border-radius: 4px;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.4);
}
@media (max-width: 599px) {
  .search-card {
    margin-top: 0;
  }
}
</style>
