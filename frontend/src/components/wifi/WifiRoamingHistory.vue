<template>
  <section aria-label="Histórico de roaming">
    <div class="d-flex align-center justify-space-between mb-2">
      <div class="section-title">Trocas de AP</div>
      <v-progress-circular
        v-if="history?.loading"
        indeterminate
        size="18"
        width="2"
        color="primary"
        aria-label="Carregando o histórico"
      ></v-progress-circular>
    </div>
    <v-alert v-if="history?.error" type="error" variant="tonal" density="compact" class="mb-2">
      {{ history.error }}
    </v-alert>
    <div v-if="events.length === 0 && !history?.loading" class="text-body-2">
      Nenhuma troca de AP registrada nos últimos {{ retentionText }}.
    </div>
    <v-list v-else density="compact" class="py-0 bg-transparent">
      <v-list-item
        v-for="event in events"
        :key="event.id"
        :title="roamRoute(event)"
        :subtitle="subtitle(event)"
        lines="two"
        class="px-0"
      >
        <template #prepend>
          <v-icon :color="event.pingPong ? 'warning' : 'info'" class="mr-2">
            {{ event.pingPong ? 'mdi-swap-horizontal-bold' : 'mdi-swap-horizontal' }}
          </v-icon>
        </template>
        <template v-if="event.pingPong" #append>
          <v-chip size="x-small" color="warning" variant="flat" class="font-weight-bold">
            Ping-pong
          </v-chip>
        </template>
      </v-list-item>
    </v-list>
  </section>
</template>

<script setup lang="ts">
import { computed, watch } from 'vue'
import type { WifiRoamEvent } from '@/bindings/WifiRoamEvent'
import { useWifiClientsStore } from '@/stores/wifiClients'
import { formatShortDateTime } from '@/utils/formatters'
import { roamRoute } from '@/utils/wifiPresentation'

const props = defineProps<{
  mac: string
  /** A retenção configurada, para a frase de "nenhuma troca". */
  retentionDays?: number
}>()

const store = useWifiClientsStore()
const history = computed(() => store.roamHistoryOf(props.mac))
const events = computed(() => history.value?.events ?? [])
const retentionText = computed(() => `${props.retentionDays ?? 30} dias`)

function signal(value: number | null): string {
  return value === null ? '?' : `${value} dBm`
}

function subtitle(event: WifiRoamEvent): string {
  return `${formatShortDateTime(event.occurredAt)} · sinal ${signal(event.signalBeforeDbm)} → ${signal(event.signalAfterDbm)}`
}

// Abrir o painel de um aparelho pede o histórico dele (uma vez); as trocas
// seguintes chegam pelo SSE (`wifi:roamed`).
watch(
  () => props.mac,
  (mac) => void store.fetchRoaming(mac),
  { immediate: true }
)
</script>

<style scoped>
.section-title {
  font-size: 0.75rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}
</style>
