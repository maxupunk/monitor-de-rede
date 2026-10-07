<template>
  <v-card elevation="2" rounded="lg" class="h-100 d-flex flex-column">
    <div class="d-flex align-start ga-3 pa-4 pb-2">
      <v-avatar :color="info.color" variant="tonal" rounded="lg" size="48">
        <v-icon size="28">{{ info.icon }}</v-icon>
      </v-avatar>
      <div class="flex-grow-1 min-w-0">
        <div class="text-subtitle-1 font-weight-bold text-truncate">{{ storage.name }}</div>
        <div v-if="storage.name !== info.label" class="text-caption text-high-emphasis">
          {{ info.label }}
        </div>
        <code v-if="storage.target" class="target d-block text-truncate mt-1">
          {{ storage.target }}
        </code>
        <div v-else class="text-caption text-error mt-1">
          Credencial ilegível — edite e informe de novo.
        </div>
      </div>
      <v-menu location="bottom end">
        <template #activator="{ props: menuProps }">
          <v-btn icon size="small" variant="text" color="primary" v-bind="menuProps">
            <v-icon>mdi-dots-vertical</v-icon>
            <v-tooltip activator="parent" location="top">Mais ações</v-tooltip>
          </v-btn>
        </template>
        <v-list density="compact">
          <v-list-item prepend-icon="mdi-pencil" title="Editar" @click="emit('edit')" />
          <v-list-item
            prepend-icon="mdi-delete-outline"
            title="Excluir"
            base-color="error"
            @click="emit('remove')"
          />
        </v-list>
      </v-menu>
    </div>

    <div class="px-4 pb-3 flex-grow-1">
      <div class="text-body-2 font-weight-medium mb-1">Guarda os backups de:</div>
      <div v-if="contents.length" class="d-flex flex-wrap ga-1">
        <v-chip
          v-for="item in contents"
          :key="item.label"
          :color="item.color"
          variant="tonal"
          size="small"
          :prepend-icon="item.icon"
        >
          {{ item.label }}
        </v-chip>
      </div>
      <div v-else class="text-body-2 text-high-emphasis">
        Nada ainda — escolha este destino ao configurar o backup do NetMonitor ou de um banco.
      </div>
    </div>

    <v-divider></v-divider>
    <div class="d-flex flex-wrap ga-2 pa-3">
      <v-btn
        color="primary"
        variant="tonal"
        prepend-icon="mdi-folder-search-outline"
        @click="emit('explore')"
      >
        Ver arquivos
      </v-btn>
      <v-btn color="info" variant="tonal" prepend-icon="mdi-connection" @click="emit('test')">
        Testar conexão
      </v-btn>
    </div>
  </v-card>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { StorageDestinationResponse } from '@/bindings/StorageDestinationResponse'
import { providerInfo } from '@/utils/storagePresentation'

/** O que um destino guarda, para o chip. */
export interface DestinationContent {
  label: string
  icon: string
  color: string
}

/** Um destino: onde fica e de quem ele guarda os backups. Só o **onde**. */
const props = defineProps<{
  storage: StorageDestinationResponse
  contents: DestinationContent[]
}>()

const emit = defineEmits<{
  explore: []
  test: []
  edit: []
  remove: []
}>()

const info = computed(() => providerInfo(props.storage.provider))
</script>

<style scoped>
.target {
  font-size: 0.78rem;
}
.min-w-0 {
  min-width: 0;
}
</style>
