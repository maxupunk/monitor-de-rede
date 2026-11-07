<template>
  <v-sheet border rounded="lg" class="pa-4 mb-3 bg-surface d-flex align-center ga-3">
    <v-avatar :color="color" size="40" rounded="lg" variant="tonal">
      <v-icon size="22">{{ state.icon }}</v-icon>
    </v-avatar>
    <div class="flex-grow-1 min-w-0">
      <div class="font-weight-bold text-subtitle-1">{{ state.title }}</div>
      <div class="text-body-2">{{ state.summary }}</div>
      <v-chip
        v-if="state.evidence?.reason"
        color="info"
        size="small"
        variant="tonal"
        prepend-icon="mdi-database-check-outline"
        class="mt-2"
      >
        Em uso: {{ state.evidence.reason }}
      </v-chip>
    </div>
    <v-switch
      :model-value="state.enabled"
      color="success"
      inset
      hide-details
      :disabled="disabled"
      :loading="loading ? 'success' : false"
      :aria-label="(state.enabled ? 'Desligar ' : 'Ligar ') + state.title"
      @update:model-value="(on) => emit('toggle', Boolean(on))"
    ></v-switch>
  </v-sheet>
</template>

<script setup lang="ts">
import type { AreaState } from '@/bindings/AreaState'

defineProps<{
  state: AreaState
  /** Cor semântica do avatar da área. */
  color: string
  disabled?: boolean
  loading?: boolean
}>()

const emit = defineEmits<{ toggle: [on: boolean] }>()
</script>

<style scoped>
.min-w-0 {
  min-width: 0;
}
</style>
