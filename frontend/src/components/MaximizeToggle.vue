<template>
  <v-btn
    :icon="modelValue ? 'mdi-window-restore' : 'mdi-window-maximize'"
    variant="text"
    size="small"
    :aria-label="label"
    :title="label"
    :aria-pressed="modelValue"
    @click="$emit('update:modelValue', !modelValue)"
  ></v-btn>
</template>

<script setup lang="ts">
/** "Maximizar / Restaurar" de uma janela de diálogo (tela cheia ↔ janela). */
import { computed } from 'vue'

const props = defineProps<{ modelValue: boolean }>()
defineEmits<{ 'update:modelValue': [value: boolean] }>()

const label = computed(() => (props.modelValue ? 'Restaurar o tamanho da janela' : 'Maximizar'))
</script>
