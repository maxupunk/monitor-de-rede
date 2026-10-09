<template>
  <v-text-field
    v-model="apiKey"
    :label="label"
    :placeholder="placeholder"
    :type="visible ? 'text' : 'password'"
    variant="outlined"
    density="compact"
    prepend-inner-icon="mdi-key-outline"
    :append-inner-icon="visible ? 'mdi-eye-off' : 'mdi-eye'"
    hide-details="auto"
    autocomplete="off"
    @click:append-inner="visible = !visible"
    @blur="emit('committed')"
  />
</template>

<script setup lang="ts">
import { ref } from 'vue'

/** Chave de API de um provedor, oculta por padrão. `committed` sai ao deixar o campo. */
defineProps<{
  label: string
  placeholder?: string
}>()

const apiKey = defineModel<string | null | undefined>({ required: true })

const emit = defineEmits<{
  (e: 'committed'): void
}>()

const visible = ref(false)
</script>
