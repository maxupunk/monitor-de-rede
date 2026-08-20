<template>
  <v-col cols="12">
    <v-text-field
      :model-value="port"
      label="Porta TCP *"
      type="number"
      min="1"
      max="65535"
      prepend-inner-icon="mdi-numeric"
      variant="outlined"
      density="comfortable"
      :rules="[(v: unknown) => isValidPort(v) || 'Informe uma porta entre 1 e 65535']"
      hide-details="auto"
      @update:model-value="emit('update:port', Number($event) || null)"
    ></v-text-field>
    <div class="d-flex flex-wrap ga-1 mt-2">
      <v-chip
        v-for="preset in COMMON_TCP_PORTS"
        :key="preset.port"
        size="small"
        :variant="port === preset.port ? 'flat' : 'outlined'"
        :color="port === preset.port ? 'primary' : undefined"
        @click="emit('update:port', preset.port)"
      >
        {{ preset.label }} · {{ preset.port }}
      </v-chip>
    </div>
  </v-col>
</template>

<script setup lang="ts">
import { COMMON_TCP_PORTS, isValidPort } from '@/utils/monitorTypes'

defineProps<{
  port: number | null
}>()

const emit = defineEmits<{
  (e: 'update:port', value: number | null): void
}>()
</script>
