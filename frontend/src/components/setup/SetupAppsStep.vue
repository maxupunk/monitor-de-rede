<template>
  <div>
    <div class="mb-4">
      <div class="text-h6 font-weight-bold">Aplicativos</div>
      <div class="text-body-2">
        Aplicativos gerenciam vários equipamentos de uma vez (ex.: o Wi-Fi de todos os roteadores
        OpenWrt). Começam desligados; ligue os que for usar. Eles também ligam sozinhos quando você
        cadastra um equipamento compatível.
      </div>
    </div>

    <v-alert v-if="apps.length === 0" type="info" variant="tonal" density="compact">
      Nenhum aplicativo disponível.
    </v-alert>

    <v-sheet
      v-for="app in apps"
      :key="app.id"
      border
      rounded="lg"
      class="pa-4 mb-3 bg-surface d-flex align-center ga-3"
    >
      <v-avatar color="primary" size="40" rounded="lg" variant="tonal">
        <v-icon size="22">{{ app.fleet?.icon ?? 'mdi-apps' }}</v-icon>
      </v-avatar>
      <div class="flex-grow-1 min-w-0">
        <div class="font-weight-bold text-subtitle-1">{{ app.fleet?.title ?? app.name }}</div>
        <div class="text-body-2">{{ app.fleet?.description ?? app.description }}</div>
      </div>
      <v-switch
        :model-value="modelValue[app.id] ?? false"
        color="success"
        inset
        hide-details
        :aria-label="'Ligar ' + (app.fleet?.title ?? app.name)"
        @update:model-value="
          (on) => emit('update:modelValue', { ...modelValue, [app.id]: Boolean(on) })
        "
      ></v-switch>
    </v-sheet>
  </div>
</template>

<script setup lang="ts">
import type { PluginSummary } from '@/bindings/PluginSummary'

defineProps<{
  /** Plugins com página de frota. */
  apps: PluginSummary[]
  /** id do plugin → ligado. */
  modelValue: Record<number, boolean>
}>()

const emit = defineEmits<{ 'update:modelValue': [value: Record<number, boolean>] }>()
</script>

<style scoped>
.min-w-0 {
  min-width: 0;
}
</style>
