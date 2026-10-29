<template>
  <DashboardCard
    title="Alvos Instáveis"
    icon="mdi-sine-wave"
    icon-color="warning"
    subtitle="Quem mais caiu e voltou na janela"
    :loading="loading"
    flush
  >
    <template #actions>
      <v-chip v-if="targets.length" size="small" color="warning" variant="tonal">
        {{ targets.length }}
      </v-chip>
      <v-select
        v-model="hours"
        :items="RANGES"
        item-title="title"
        item-value="value"
        density="compact"
        variant="outlined"
        hide-details
        aria-label="Janela"
        class="unstable-widget__select"
      ></v-select>
    </template>

    <v-list v-if="targets.length" lines="two" bg-color="transparent" class="py-0">
      <v-list-item
        v-for="target in targets"
        :key="target.scopeKey"
        :title="labelOf(target.scopeKey)"
        class="px-5"
        :class="{ 'cursor-pointer': monitorIdOf(target.scopeKey) != null }"
        @click="open(target.scopeKey)"
      >
        <template #prepend>
          <v-avatar :color="target.flapping ? 'warning' : 'info'" variant="tonal" size="36">
            <v-icon>mdi-sine-wave</v-icon>
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

    <div v-else-if="!loading" class="pa-8 text-center">
      <v-icon size="42" color="success">mdi-check-circle-outline</v-icon>
      <div class="mt-2 text-body-2">Nenhum alvo oscilando no período.</div>
    </div>
  </DashboardCard>

  <!--
    O alvo instável abre o **mesmo** diálogo de detalhe das outras listas de
    monitor, em vez de navegar para uma tela cheia. Quem está lendo o ranking
    quer confirmar o alvo e voltar ao painel; a navegação custava esse caminho
    de volta.
  -->
  <MonitorDetailDialog v-model="detalheAberto" :monitor-id="monitorEmDetalhe" />
</template>

<script setup lang="ts">
/**
 * Ranking dos alvos que mais oscilaram na janela (Fase 3 do roadmap de alertas
 * inteligentes). Responde no dashboard a pergunta "que link está me dando
 * trabalho?" — quem cai e volta o dia inteiro não aparece na lista de alertas
 * ativos, porque na hora do olhar ele pode estar no ar.
 */
import { ref, computed, onMounted, watch } from 'vue'
import MonitorDetailDialog from '@/components/monitors/MonitorDetailDialog.vue'
import { useMonitorDetail } from '@/composables/useMonitorDetail'
import { useAlertsStore, type ScopeInstability } from '@/stores/alerts'
import { useMonitorsStore } from '@/stores/monitors'
import { formatRelativeTime } from '@/utils/formatters'
import DashboardCard from '@/components/dashboard/DashboardCard.vue'

const RANGES = [
  { value: 6, title: 'Últimas 6h' },
  { value: 24, title: 'Últimas 24h' },
  { value: 168, title: 'Últimos 7 dias' },
]

/** Alvo com uma única queda não é instabilidade: é um incidente. */
const MIN_OSCILLATIONS = 2
const MAX_ITEMS = 6

const alertsStore = useAlertsStore()
const monitorsStore = useMonitorsStore()
const { detalheAberto, monitorEmDetalhe, abrirDetalhe } = useMonitorDetail()

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

/** Só escopo de monitor tem detalhe a abrir; interface e túnel, não. */
function monitorIdOf(scopeKey: string): number | null {
  const [kind, rawId] = scopeKey.split(':')
  if (kind !== 'monitor') return null
  const id = Number(rawId)
  return Number.isFinite(id) ? id : null
}

function open(scopeKey: string) {
  const id = monitorIdOf(scopeKey)
  if (id != null) abrirDetalhe(id)
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

<style scoped>
.unstable-widget__select {
  max-width: 170px;
}
</style>
