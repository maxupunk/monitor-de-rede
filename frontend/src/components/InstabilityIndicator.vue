<template>
  <v-alert
    v-if="summary && summary.oscillations > 1"
    :type="summary.flapping ? 'warning' : 'info'"
    variant="tonal"
    density="comfortable"
    :icon="summary.flapping ? 'mdi-sine-wave' : 'mdi-history'"
    class="mb-4"
  >
    <div class="d-flex flex-wrap align-center ga-2">
      <span class="font-weight-medium">{{ headline }}</span>
      <v-chip v-if="summary.flapping" size="x-small" color="warning" variant="flat">
        Oscilando agora
      </v-chip>
    </div>
    <div v-if="detail" class="text-caption mt-1">{{ detail }}</div>
  </v-alert>
</template>

<script setup lang="ts">
/**
 * Indicador de instabilidade do alvo (Fase 3 do roadmap de alertas
 * inteligentes): "este link oscilou 12x nas últimas 24h".
 *
 * Só aparece quando há história para contar — uma queda isolada não é
 * instabilidade, e um aviso permanente dizendo "0 oscilações" seria ruído numa
 * tela que já tem muito o que mostrar.
 */
import { ref, computed, onMounted, watch } from 'vue'
import { useAlertsStore, type ScopeInstability } from '@/stores/alerts'
import { formatRelativeTime } from '@/utils/formatters'

const props = withDefaults(
  defineProps<{
    /** Alvo consultado: `monitor:12`, `interface:34`, `vpn_peer:7` */
    scopeKey: string
    /** Janela da pergunta, em horas */
    hours?: number
  }>(),
  { hours: 24 }
)

const alertsStore = useAlertsStore()
const summary = ref<ScopeInstability | null>(null)

const windowLabel = computed(() =>
  props.hours === 24 ? 'nas últimas 24h' : `nas últimas ${props.hours}h`
)

const headline = computed(() => {
  if (!summary.value) return ''
  const { oscillations, episodes } = summary.value
  const episodeText = episodes === 1 ? '1 episódio' : `${episodes} episódios`
  return `Este alvo oscilou ${oscillations}x ${windowLabel.value} (${episodeText}).`
})

const detail = computed(() => {
  if (!summary.value) return ''
  const parts: string[] = []
  if (summary.value.lastProblemAt) {
    parts.push(`Último problema ${formatRelativeTime(summary.value.lastProblemAt)}`)
  }
  if (summary.value.flapping) {
    parts.push('as notificações estão suspensas até o alvo estabilizar')
  }
  return parts.join(' · ')
})

async function load() {
  if (!props.scopeKey) {
    summary.value = null
    return
  }
  const result = await alertsStore.fetchInstability({
    scopeKey: props.scopeKey,
    hours: props.hours,
  })
  summary.value = result[0] ?? null
}

onMounted(load)
watch(() => [props.scopeKey, props.hours], load)
</script>
