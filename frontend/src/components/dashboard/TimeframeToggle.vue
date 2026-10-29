<template>
  <v-btn-toggle
    :model-value="modelValue"
    density="compact"
    variant="tonal"
    color="primary"
    divided
    mandatory
    rounded="lg"
    aria-label="Janela de tempo"
    @update:model-value="(value: Timeframe) => emit('update:modelValue', value)"
  >
    <v-btn v-for="option in TIMEFRAMES" :key="option" :value="option" size="small">
      {{ option }}
    </v-btn>
  </v-btn-toggle>
</template>

<script setup lang="ts">
import { TIMEFRAMES, type Timeframe } from '@/composables/useWidgetTimeframe'

/** Seletor único de janela de tempo dos widgets (5m, 15m, 1h, 24h). */
defineProps<{ modelValue: Timeframe }>()
const emit = defineEmits<{ (e: 'update:modelValue', value: Timeframe): void }>()
</script>
