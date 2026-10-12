<template>
  <div v-if="summary" class="ai-alert-summary d-flex align-start ga-2 pa-2 mt-1 rounded">
    <v-icon size="16" color="deep-purple" class="mt-1">mdi-robot-outline</v-icon>
    <div class="min-w-0">
      <div class="text-body-small font-weight-bold text-deep-purple">
        Resumo da IA · {{ formatRelativeTime(summary.generatedAt) }}
      </div>
      <div class="text-body-small summary-text">{{ summary.text }}</div>
      <div v-if="triageReading" class="text-body-small mt-1">
        <v-icon size="14" color="primary">mdi-lightning-bolt-circle</v-icon>
        Decisor: {{ triageReading }}
      </div>
    </div>
  </div>
  <div
    v-else-if="triage?.suppressed"
    class="decider-triage d-flex align-center flex-wrap ga-2 pa-2 mt-1 rounded"
  >
    <v-icon size="16" color="primary">mdi-lightning-bolt-circle</v-icon>
    <div class="text-body-small flex-grow-1 min-w-0">
      <strong>Resumo pulado pelo Decisor:</strong> {{ triageReading }}.
    </div>
    <v-btn
      size="small"
      color="primary"
      variant="flat"
      prepend-icon="mdi-robot-outline"
      :loading="generating"
      :disabled="!alertId"
      @click="generate"
    >
      Gerar resumo agora
    </v-btn>
    <div v-if="error" class="text-body-small text-error w-100">{{ error }}</div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import type { AiIncidentSummary } from '@/bindings/AiIncidentSummary'
import type { DeciderTriage } from '@/bindings/DeciderTriage'
import { useAlertsStore } from '@/stores/alerts'
import { formatPercent, formatRelativeTime } from '@/utils/formatters'

/**
 * Resumo automático gravado no alerta quando ele abriu (IA proativa) e, com a
 * triagem do Decisor ligada, a opinião dele. Resumo pulado pela triagem oferece
 * gerar agora: o Decisor sugere, o operador decide.
 */
const props = defineProps<{
  alertId?: number | null
  summary?: AiIncidentSummary | null
  triage?: DeciderTriage | null
}>()

const alertsStore = useAlertsStore()
const generating = ref(false)
const error = ref<string | null>(null)

/** A leitura mais provável do Decisor, em português. */
const triageReading = computed(() => {
  const triage = props.triage
  if (!triage) return ''
  const readings = [
    { value: triage.actionable, text: 'incidente novo, vale investigar' },
    { value: triage.symptom, text: 'provável sintoma de outra falha' },
    { value: triage.transient, text: 'oscilação que deve se resolver sozinha' },
  ]
  const top = readings.reduce((best, item) => (item.value > best.value ? item : best))
  return `${top.text} · ${formatPercent(top.value, 0)}`
})

async function generate(): Promise<void> {
  if (!props.alertId) return
  generating.value = true
  error.value = await alertsStore.requestAiSummary(props.alertId)
  generating.value = false
}
</script>

<style scoped>
.ai-alert-summary {
  background-color: rgba(var(--v-theme-deep-purple, 103, 58, 183), 0.08);
  border-left: 3px solid rgb(103, 58, 183);
}
.decider-triage {
  background-color: rgba(var(--v-theme-primary), 0.08);
  border-left: 3px solid rgb(var(--v-theme-primary));
}
.summary-text {
  white-space: pre-wrap;
}
.min-w-0 {
  min-width: 0;
}
</style>
