<template>
  <v-col cols="12" md="6">
    <v-select
      :model-value="httpMethod"
      :items="['GET', 'HEAD', 'POST']"
      label="Método HTTP"
      prepend-inner-icon="mdi-swap-horizontal"
      variant="outlined"
      density="comfortable"
      hide-details="auto"
      @update:model-value="emit('update:httpMethod', $event)"
    ></v-select>
  </v-col>
</template>

<script setup lang="ts">
import type { HttpMethod } from '@/utils/monitorTypes'

defineProps<{
  httpMethod: HttpMethod
}>()

const emit = defineEmits<{
  (e: 'update:httpMethod', value: HttpMethod): void
}>()
</script>
