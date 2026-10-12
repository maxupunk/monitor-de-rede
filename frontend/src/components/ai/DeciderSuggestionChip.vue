<template>
  <div v-if="suggestion" class="laya-suggestion d-inline-flex align-center flex-wrap ga-1">
    <v-chip
      size="small"
      color="primary"
      variant="tonal"
      prepend-icon="mdi-lightning-bolt-circle"
      :title="`Respondido por ${suggestion.model}`"
    >
      Laya sugere: <strong class="ms-1">{{ formatLabel(suggestion.value) }}</strong>
      <span class="ms-1">· {{ formatPercent(suggestion.confidence, 0) }}</span>
    </v-chip>
    <v-btn
      v-if="actionLabel"
      size="small"
      color="primary"
      variant="flat"
      :disabled="disabled"
      @click="emit('apply', suggestion.value)"
    >
      {{ actionLabel }}
    </v-btn>
  </div>
</template>

<script setup lang="ts">
import type { LayaSuggestion } from '@/bindings/LayaSuggestion'
import { formatPercent } from '@/utils/formatters'

/**
 * Um palpite do Laya com a confiança dele. Só mostra e oferece a ação: quem
 * decide aplicar é o usuário, no clique.
 */
withDefaults(
  defineProps<{
    /** Sem sugestão, o componente não desenha nada. */
    suggestion: LayaSuggestion | null
    /** Como o valor aparece para o usuário (ex.: "Câmera" para `camera`). */
    formatLabel?: (value: string) => string
    /** Sem rótulo, o chip é só informativo. */
    actionLabel?: string
    disabled?: boolean
  }>(),
  { formatLabel: (value: string) => value, actionLabel: undefined, disabled: false }
)

const emit = defineEmits<{
  (e: 'apply', value: string): void
}>()
</script>
