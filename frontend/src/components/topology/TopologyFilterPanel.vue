<template>
  <div class="filter-panel">
    <div class="d-flex align-center mb-3">
      <v-icon color="primary" class="mr-2">mdi-filter-variant</v-icon>
      <span class="text-subtitle-1 font-weight-bold">Filtros</span>
      <v-spacer></v-spacer>
      <v-btn
        v-if="count > 0"
        size="small"
        color="primary"
        variant="text"
        prepend-icon="mdi-filter-remove"
        @click="reset"
      >
        Limpar
      </v-btn>
    </div>

    <v-switch
      :model-value="modelValue.problemsOnly"
      color="error"
      density="compact"
      hide-details
      inset
      class="mb-2"
      label="Só problemas (offline, alerta, enlace fora ou saturado)"
      @update:model-value="patch({ problemsOnly: Boolean($event) })"
    ></v-switch>

    <div class="section-title">Estado do dispositivo</div>
    <v-chip-group
      :model-value="modelValue.statuses"
      multiple
      column
      @update:model-value="patch({ statuses: $event as NodeStatusKey[] })"
    >
      <v-chip
        v-for="(status, key) in NODE_STATUS"
        :key="key"
        :value="key"
        :color="status.color"
        :base-color="status.color"
        variant="outlined"
        filter
        size="small"
        :prepend-icon="status.icon"
      >
        {{ status.label }}
      </v-chip>
    </v-chip-group>

    <div class="section-title">Tipo de dispositivo</div>
    <v-chip-group
      :model-value="modelValue.categories"
      multiple
      column
      @update:model-value="patch({ categories: $event as string[] })"
    >
      <v-chip
        v-for="option in categoryOptions"
        :key="option.value"
        :value="option.value"
        color="primary"
        base-color="primary"
        variant="outlined"
        filter
        size="small"
        :prepend-icon="option.icon"
      >
        {{ option.title }}
        <span class="ml-1 font-weight-bold">{{ option.count }}</span>
      </v-chip>
    </v-chip-group>

    <div class="section-title">Tipo de enlace</div>
    <v-chip-group
      :model-value="modelValue.linkKinds"
      multiple
      column
      @update:model-value="patch({ linkKinds: $event as LinkKind[] })"
    >
      <v-chip
        v-for="(kind, key) in LINK_KINDS"
        :key="key"
        :value="key"
        :color="kind.color"
        :base-color="kind.color"
        variant="outlined"
        filter
        size="small"
        :prepend-icon="kind.icon"
      >
        {{ kind.label }}
      </v-chip>
    </v-chip-group>

    <div class="section-title">O que não casa com o filtro</div>
    <v-btn-toggle
      :model-value="modelValue.mode"
      mandatory
      color="primary"
      density="compact"
      variant="outlined"
      divided
      @update:model-value="patch({ mode: $event === 'hide' ? 'hide' : 'dim' })"
    >
      <v-btn value="dim" prepend-icon="mdi-circle-opacity">Esmaecer</v-btn>
      <v-btn value="hide" prepend-icon="mdi-eye-off">Esconder</v-btn>
    </v-btn-toggle>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { TopologyNode } from '@/stores/topology'
import { activeFilterCount, EMPTY_FILTERS, type TopologyFilters } from '@/utils/topology/filters'
import {
  LINK_KINDS,
  NODE_CATEGORY_OPTIONS,
  NODE_STATUS,
  nodeCategory,
  type LinkKind,
  type NodeStatusKey,
} from '@/utils/topology/presentation'

const props = defineProps<{
  modelValue: TopologyFilters
  nodes: readonly TopologyNode[]
}>()

const emit = defineEmits<{ 'update:modelValue': [value: TopologyFilters] }>()

const count = computed(() => activeFilterCount(props.modelValue))

/** Só os tipos presentes no mapa, com a contagem. */
const categoryOptions = computed(() => {
  const counts = new Map<string, number>()
  for (const node of props.nodes) {
    const key = nodeCategory(node)
    counts.set(key, (counts.get(key) ?? 0) + 1)
  }
  return NODE_CATEGORY_OPTIONS.filter(
    (option) => counts.has(option.value) || props.modelValue.categories.includes(option.value)
  ).map((option) => ({ ...option, count: counts.get(option.value) ?? 0 }))
})

function patch(change: Partial<TopologyFilters>) {
  emit('update:modelValue', { ...props.modelValue, ...change })
}

function reset() {
  emit('update:modelValue', { ...EMPTY_FILTERS, mode: props.modelValue.mode })
}
</script>

<style scoped>
.section-title {
  font-size: 0.75rem;
  font-weight: 700;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: rgb(var(--v-theme-on-surface));
  margin-top: 12px;
  margin-bottom: 2px;
}
</style>
