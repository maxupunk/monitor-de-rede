<template>
  <v-tooltip v-if="available" location="top" :text="tooltip">
    <template #activator="{ props: tipProps }">
      <v-btn
        v-bind="tipProps"
        :size="size"
        color="deep-purple"
        :variant="variant"
        :icon="iconOnly ? 'mdi-robot-outline' : undefined"
        :prepend-icon="iconOnly ? undefined : 'mdi-robot-outline'"
        :disabled="busy"
        @click.stop="ask(prompt, context)"
      >
        <template v-if="!iconOnly">{{ label }}</template>
      </v-btn>
    </template>
  </v-tooltip>
</template>

<script setup lang="ts">
import { useAiAssistant } from '@/composables/useAiAssistant'
import type { AiChatContext } from '@/stores/ai'

/**
 * "Diagnosticar com IA": abre o assistente com a pergunta pronta e o
 * contexto da tela. Some quando o assistente está desativado.
 */
withDefaults(
  defineProps<{
    prompt: string
    context?: AiChatContext
    label?: string
    tooltip?: string
    iconOnly?: boolean
    size?: 'x-small' | 'small' | 'default'
    variant?: 'flat' | 'tonal' | 'outlined' | 'text'
  }>(),
  {
    context: () => ({}),
    label: 'Diagnosticar com IA',
    tooltip: 'Abre o Assistente IA já investigando este item',
    iconOnly: false,
    size: 'small',
    variant: 'tonal',
  }
)

const { available, busy, ask } = useAiAssistant()
</script>
