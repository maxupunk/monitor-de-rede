<template>
  <v-col cols="12" md="6">
    <v-text-field
      :model-value="packetCount"
      label="Pacotes por checagem"
      type="number"
      min="1"
      max="20"
      variant="outlined"
      density="comfortable"
      hint="Mais pacotes dão uma medida de perda mais confiável"
      persistent-hint
      @update:model-value="emit('update:packetCount', Number($event))"
    ></v-text-field>
  </v-col>
</template>

<script setup lang="ts">
defineProps<{
  packetCount: number
}>()

const emit = defineEmits<{
  (e: 'update:packetCount', value: number): void
}>()
</script>
