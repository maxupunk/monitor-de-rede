<template>
  <v-card variant="tonal" color="primary" class="my-2 pa-3 rounded-lg">
    <div class="d-flex align-start ga-2 mb-2">
      <v-icon size="18" color="primary">mdi-account-question-outline</v-icon>
      <span class="text-body-medium font-weight-medium">{{ question.question }}</span>
    </div>
    <div v-if="question.options.length > 0" class="d-flex flex-wrap ga-2">
      <v-btn
        v-for="option in question.options"
        :key="option"
        size="small"
        color="primary"
        variant="outlined"
        class="text-none"
        :disabled="!answerable"
        @click="aiStore.sendMessage(option)"
      >
        {{ option }}
      </v-btn>
    </div>
    <div v-if="answerable" class="text-body-small text-medium-emphasis mt-2">
      Escolha uma opção ou responda no campo — use <strong>@</strong> para marcar o recurso.
    </div>
  </v-card>
</template>

<script setup lang="ts">
import { useAiStore } from '@/stores/ai'
import type { AiQuestion } from '@/utils/aiChatStream'

defineProps<{
  question: AiQuestion
  /** Só a pergunta mais recente, com a IA parada, aceita resposta pelos botões. */
  answerable: boolean
}>()

const aiStore = useAiStore()
</script>
