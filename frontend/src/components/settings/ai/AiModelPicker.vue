<template>
  <div>
    <v-combobox
      :model-value="model"
      :label="label"
      :placeholder="placeholder"
      :items="items"
      item-title="id"
      item-value="id"
      :return-object="false"
      :custom-filter="filterModelChoices"
      :loading="loading"
      :clearable="clearable"
      variant="outlined"
      density="compact"
      prepend-inner-icon="mdi-cube-outline"
      hide-details="auto"
      class="mb-1"
      @update:model-value="(value) => (model = extractModelId(value))"
    >
      <template #append-inner>
        <v-btn
          icon="mdi-magnify"
          variant="text"
          size="x-small"
          color="primary"
          title="Abrir o catálogo de modelos"
          @click.stop="emit('open-catalog')"
        />
        <v-btn
          icon="mdi-refresh"
          variant="text"
          size="x-small"
          color="primary"
          :loading="loading"
          title="Atualizar a lista de modelos"
          @click.stop="emit('refresh')"
        />
      </template>
      <template #item="{ item, props: itemProps }">
        <v-list-item
          v-bind="itemProps"
          :title="item.name"
          :subtitle="item.id !== item.name ? item.id : (item.description ?? undefined)"
        >
          <template #append>
            <AiModelChips :item="item" :driver="driver" class="ms-2" />
          </template>
        </v-list-item>
      </template>
    </v-combobox>

    <div class="d-flex align-center justify-space-between flex-wrap ga-1">
      <div class="text-body-2">
        <slot name="hint" />
      </div>
      <v-btn
        variant="text"
        size="small"
        color="primary"
        prepend-icon="mdi-magnify"
        @click="emit('open-catalog')"
      >
        {{ catalogLabel }}
      </v-btn>
    </div>
  </div>
</template>

<script setup lang="ts">
import AiModelChips from './AiModelChips.vue'
import { filterModelChoices, type AiModelChoice } from './aiModelCatalog'
import { extractModelId, type AiDriver } from './aiProviders'

/** Seletor de modelo de um provedor: lista, busca, atualizar e atalho para o catálogo. */
withDefaults(
  defineProps<{
    label: string
    items: AiModelChoice[]
    driver: AiDriver
    placeholder?: string
    loading?: boolean
    clearable?: boolean
    catalogLabel?: string
  }>(),
  { catalogLabel: 'Ver catálogo completo' }
)

const model = defineModel<string | null | undefined>({ required: true })

const emit = defineEmits<{
  (e: 'refresh'): void
  (e: 'open-catalog'): void
}>()
</script>
