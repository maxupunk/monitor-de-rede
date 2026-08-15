<template>
  <v-card elevation="2" class="rounded-lg fill-height">
    <v-card-title class="d-flex align-center justify-space-between py-3 px-4 flex-wrap ga-2">
      <div class="d-flex align-center">
        <v-icon start color="warning">mdi-sine-wave</v-icon>
        <span class="font-weight-bold text-h6">Alvos Instáveis</span>
        <v-chip v-if="targets.length" size="x-small" color="warning" class="ml-2" variant="tonal">
          {{ targets.length }}
        </v-chip>
      </div>
      <v-select
        v-model="hours"
        :items="RANGES"
        item-title="title"
        item-value="value"
        density="compact"
        variant="outlined"
        hide-details
        style="max-width: 160px"
      ></v-select>
    </v-card-title>
    <v-divider></v-divider>

    <v-card-text class="pa-0">
      <div v-if="loading" class="pa-6 text-center">
        <v-progress-circular indeterminate color="primary" size="28"></v-progress-circular>
      </div>

      <v-list v-else-if="targets.length" lines="two">
        <v-list-item
          v-for="target in targets"
          :key="target.scopeKey"
          :title="labelOf(target.scopeKey)"
          class="px-4 py-2 border-b"
          :class="{ 'cursor-pointer': linkOf(target.scopeKey) }"
          @click="open(target.scopeKey)"
        >
          <template #prepend>
            <v-avatar :color="target.flapping ? 'warning' : 'grey'" size="36">
              <v-icon color="white">mdi-sine-wave</v-icon>
            </v-avatar>
          </template>
          <template #subtitle>
            <span>
              {{ target.oscillations }}
              {{ target.oscillations === 1 ? 'queda' : 'quedas' }} ·
              {{ target.episodes }}
              {{ target.episodes === 1 ? 'episódio' : 'episódios' }}
              <template v-if="target.lastProblemAt">
                · último {{ formatRelativeTime(target.lastProblemAt) }}
              </template>
            </span>
          </template>
          <template #append>
            <v-chip v-if="target.flapping" size="x-small" color="warning" variant="flat">
              Oscilando
            </v-chip>
          </template>
        </v-list-item>
      </v-list>

      <div v-else class="pa-8 text-center text-grey">
        <v-icon size="42" color="success">mdi-check-circle-outline</v-icon>
        <div class="mt-2 text-body-2">Nenhum alvo oscilando no período.</div>
      </div>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
/**
 * Ranking dos alvos que mais oscilaram na janela (Fase 3 do roadmap de alertas
 * inteligentes). Responde no dashboard a pergunta "que link está me dando
 * trabalho?" — quem cai e volta o dia inteiro não aparece na lista de alertas
 * ativos, porque na hora do olhar ele pode estar no ar.
 */
import { ref, computed, onMounted, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useAlertsStore, type ScopeInstability } from '@/stores/alerts'
import { useMonitorsStore } from '@/stores/monitors'
import { formatRelativeTime } from '@/utils/formatters'

const RANGES = [
  { value: 6, title: 'Últimas 6h' },
  { value: 24, title: 'Últimas 24h' },
  { value: 168, title: 'Últimos 7 dias' },
]

/** Alvo com uma única queda não é instabilidade: é um incidente. */
const MIN_OSCILLATIONS = 2
const MAX_ITEMS = 6

const router = useRouter()
const alertsStore = useAlertsStore()
const monitorsStore = useMonitorsStore()

const hours = ref(24)
const loading = ref(false)
const all = ref<ScopeInstability[]>([])

const targets = computed(() =>
  all.value.filter((item) => item.oscillations >= MIN_OSCILLATIONS).slice(0, MAX_ITEMS)
)

/** `monitor:12` vira o nome do monitor; os demais escopos ficam legíveis. */
function labelOf(scopeKey: string): string {
  const [kind, rawId] = scopeKey.split(':')
  const id = Number(rawId)
  if (kind === 'monitor') {
    const monitor = monitorsStore.monitors.find((item) => item.id === id)
    if (monitor) {
      return monitor.device ? `${monitor.name} — ${monitor.device.name}` : monitor.name
    }
    return `Monitor #${id}`
  }
  if (kind === 'interface') return `Interface #${id}`
  if (kind === 'vpn_peer') return `Túnel VPN #${id}`
  return scopeKey
}

function linkOf(scopeKey: string) {
  const [kind, rawId] = scopeKey.split(':')
  if (kind !== 'monitor') return null
  return { name: 'monitor-detail', params: { id: Number(rawId) } }
}

function open(scopeKey: string) {
  const target = linkOf(scopeKey)
  if (target) router.push(target)
}

async function load() {
  loading.value = true
  all.value = await alertsStore.fetchInstability({ hours: hours.value })
  loading.value = false
}

onMounted(() => {
  if (monitorsStore.monitors.length === 0) monitorsStore.fetchMonitors()
  load()
})
watch(hours, load)
</script>
