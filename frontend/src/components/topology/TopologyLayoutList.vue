<template>
  <v-list density="comfortable" class="py-1" min-width="320">
    <v-list-subheader class="font-weight-bold">Posicionar automaticamente</v-list-subheader>
    <v-list-item
      :disabled="!canWrite || newCount === 0"
      prepend-icon="mdi-star-four-points-outline"
      title="Organizar só os novos"
      :subtitle="
        newCount > 0
          ? `${newCount} dispositivo(s) colocado(s) automaticamente`
          : 'Nenhum dispositivo novo para organizar'
      "
      base-color="primary"
      @click="$emit('arrange-new')"
    ></v-list-item>
    <v-divider class="my-1"></v-divider>
    <v-list-subheader class="font-weight-bold">Reorganizar tudo</v-list-subheader>
    <v-list-item
      v-for="option in LAYOUT_OPTIONS"
      :key="option.value"
      :disabled="!canWrite"
      :prepend-icon="option.icon"
      :title="option.title"
      :subtitle="option.subtitle"
      base-color="primary"
      @click="$emit('layout', option.value)"
    ></v-list-item>
    <div v-if="!canWrite" class="px-4 py-2 text-caption read-only-note">
      <v-icon size="16" color="info" class="mr-1">mdi-information</v-icon>
      {{ READ_ONLY_REASON }}
    </div>
  </v-list>
</template>

<script setup lang="ts">
import { LAYOUT_OPTIONS, type LayoutKind } from '@/utils/topology/layouts'
import { READ_ONLY_REASON } from '@/utils/topology/inspectorRegistry'

defineProps<{ canWrite: boolean; newCount: number }>()
defineEmits<{ layout: [kind: LayoutKind]; 'arrange-new': [] }>()
</script>

<style scoped>
.read-only-note {
  max-width: 320px;
  color: rgb(var(--v-theme-on-surface));
}
</style>
