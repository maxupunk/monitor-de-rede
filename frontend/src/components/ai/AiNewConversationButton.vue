<template>
  <v-btn
    v-if="labeled"
    block
    color="primary"
    variant="flat"
    prepend-icon="mdi-plus"
    :disabled="!canStart"
    @click="start"
  >
    Nova conversa
  </v-btn>
  <v-btn
    v-else
    icon="mdi-plus"
    color="primary"
    aria-label="Nova conversa"
    title="Nova conversa"
    :disabled="!canStart"
    @click="start"
  ></v-btn>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useAiStore } from '@/stores/ai'

/**
 * Começa uma conversa nova. Fica desligado enquanto a IA responde e quando a
 * conversa atual já está vazia — não há o que deixar para trás.
 */
withDefaults(
  defineProps<{
    /** Botão largo com texto (lista de conversas); sem isso, só o ícone. */
    labeled?: boolean
  }>(),
  { labeled: false }
)

const emit = defineEmits<{ started: [] }>()

const aiStore = useAiStore()
const canStart = computed(() => aiStore.messages.length > 0 && !aiStore.isStreaming)

function start() {
  aiStore.newConversation()
  emit('started')
}
</script>
