<template>
  <div v-if="summary" class="ai-alert-summary d-flex align-start ga-2 pa-2 mt-1 rounded">
    <v-icon size="16" color="deep-purple" class="mt-1">mdi-robot-outline</v-icon>
    <div class="min-w-0">
      <div class="text-caption font-weight-bold text-deep-purple">
        Resumo da IA · {{ formatRelativeTime(summary.generatedAt) }}
      </div>
      <div class="text-caption summary-text">{{ summary.text }}</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { AiIncidentSummary } from '@/bindings/AiIncidentSummary'
import { formatRelativeTime } from '@/utils/formatters'

/** Resumo automático gravado no alerta quando ele abriu (IA proativa). */
defineProps<{
  summary?: AiIncidentSummary | null
}>()
</script>

<style scoped>
.ai-alert-summary {
  background-color: rgba(var(--v-theme-deep-purple, 103, 58, 183), 0.08);
  border-left: 3px solid rgb(103, 58, 183);
}
.summary-text {
  white-space: pre-wrap;
}
.min-w-0 {
  min-width: 0;
}
</style>
