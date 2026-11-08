<template>
  <v-btn
    :icon="icon"
    :color="color"
    variant="text"
    size="small"
    :aria-label="label"
    :loading="loading"
    :disabled="disabled"
    @click.stop="emit('click')"
  >
    <v-icon>{{ icon }}</v-icon>
    <v-tooltip activator="parent" location="top" max-width="280">{{ label }}</v-tooltip>
  </v-btn>
</template>

<script setup lang="ts">
/**
 * Botão só de ícone numa linha de tabela: o rótulo vira tooltip e
 * `aria-label` (o `title` nativo não aparece no toque nem é bem lido).
 */
withDefaults(
  defineProps<{
    icon: string
    label: string
    color?: string
    loading?: boolean
    disabled?: boolean
  }>(),
  { color: 'primary', loading: false, disabled: false }
)

const emit = defineEmits<{ click: [] }>()
</script>
