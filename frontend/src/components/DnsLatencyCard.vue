<template>
  <DashboardCard
    title="Tempo de Resolução DNS"
    icon="mdi-dns-outline"
    icon-color="secondary"
    :subtitle="headerSubtitle"
    :loading="store.loading || store.benchmarking"
    flush
  >
    <template #actions>
      <v-btn-toggle
        v-model="mode"
        density="compact"
        variant="tonal"
        color="primary"
        rounded="lg"
        divided
        mandatory
        @update:model-value="onModeChange"
      >
        <v-btn value="history" size="small">
          Histórico
          <v-tooltip activator="parent" location="top"
            >Gráfico temporal dos monitores DNS</v-tooltip
          >
        </v-btn>
        <v-btn value="benchmark" size="small">
          Comparar
          <v-tooltip activator="parent" location="top"
            >Mede os resolvedores públicos agora</v-tooltip
          >
        </v-btn>
      </v-btn-toggle>

      <v-btn-toggle
        v-if="mode === 'history'"
        v-model="selectedWindowHours"
        density="compact"
        variant="tonal"
        color="primary"
        rounded="lg"
        divided
        mandatory
        aria-label="Janela de tempo"
        @update:model-value="onWindowHoursChange"
      >
        <v-btn :value="1" size="small">1h</v-btn>
        <v-btn :value="6" size="small">6h</v-btn>
        <v-btn :value="24" size="small">24h</v-btn>
        <v-btn :value="168" size="small">7d</v-btn>
      </v-btn-toggle>

      <v-btn
        size="small"
        color="secondary"
        variant="tonal"
        prepend-icon="mdi-checkbox-multiple-marked-outline"
        class="text-none"
        @click="batchDialog = true"
      >
        <span class="hidden-xs">Monitorar DNS</span>
        <v-tooltip activator="parent" location="top">
          Adicionar múltiplos servidores DNS ao monitoramento contínuo
        </v-tooltip>
      </v-btn>
      <v-btn
        icon
        size="small"
        variant="text"
        color="primary"
        :loading="store.loading || store.benchmarking"
        @click="refresh"
      >
        <v-icon>mdi-refresh</v-icon>
        <v-tooltip activator="parent" location="top">Atualizar</v-tooltip>
      </v-btn>
      <v-btn icon size="small" variant="text" color="primary" @click="serversDialog = true">
        <v-icon>mdi-cog-outline</v-icon>
        <v-tooltip activator="parent" location="top">
          Gerenciar os servidores DNS comparados
        </v-tooltip>
      </v-btn>
    </template>

    <div>
      <v-alert
        v-if="store.error"
        type="error"
        variant="tonal"
        density="compact"
        class="ma-4"
        :text="store.error"
      ></v-alert>

      <!-- ============================================================ -->
      <!-- ABA COMPARAR (BENCHMARK AO VIVO) - MANTIDA INTACTA           -->
      <!-- ============================================================ -->
      <template v-if="mode === 'benchmark'">
        <div v-if="store.benchmarking" class="pa-6 text-center">
          <v-progress-circular indeterminate color="secondary" size="32"></v-progress-circular>
          <div class="text-subtitle-2 font-weight-medium mt-3">Medindo resolvedores…</div>
          <div class="text-caption">As consultas rodam em série para a comparação ser justa.</div>
        </div>
        <v-list v-else-if="store.ranking.length > 0" class="py-0">
          <v-list-item
            v-for="(entry, index) in store.ranking"
            :key="`${entry.server}-${entry.protocol}`"
            class="px-5 py-3"
          >
            <template #prepend>
              <v-avatar :color="positionColor(index, entry)" size="30" class="mr-3">
                <span class="text-caption font-weight-bold">{{ index + 1 }}</span>
              </v-avatar>
            </template>

            <v-list-item-title class="d-flex align-center ga-2 flex-wrap">
              <span class="font-weight-medium">{{ entry.label }}</span>
              <v-chip size="x-small" variant="tonal" color="secondary">
                {{ protocolLabel(entry.protocol) }}
              </v-chip>
              <v-chip
                v-if="index === 0 && entry.avgLookupTimeMs !== null"
                size="x-small"
                color="success"
                variant="tonal"
              >
                <v-icon start size="12">mdi-trophy-outline</v-icon>
                mais rápido
              </v-chip>
            </v-list-item-title>

            <v-list-item-subtitle class="mt-1">
              <div class="d-flex align-center ga-2">
                <v-progress-linear
                  :model-value="barValue(entry)"
                  :color="positionColor(index, entry)"
                  height="6"
                  rounded
                  style="max-width: 180px"
                ></v-progress-linear>
                <span class="text-caption">
                  {{ rangeLabel(entry) }}
                </span>
              </div>
              <div class="text-caption mt-1">
                {{ sampleLabel(entry) }}
              </div>
            </v-list-item-subtitle>

            <template #append>
              <div class="d-flex flex-column flex-md-row align-end align-md-center ga-2">
                <div class="text-right">
                  <div
                    v-if="entry.avgLookupTimeMs !== null"
                    class="d-flex align-baseline ga-1 justify-end"
                  >
                    <span
                      class="text-h6 font-weight-bold"
                      :class="`text-${positionColor(index, entry)}`"
                    >
                      {{ formatLatency(entry.avgLookupTimeMs, '—') }}
                    </span>
                    <span class="text-caption">ms</span>
                  </div>
                  <v-chip v-else size="x-small" color="error" variant="tonal">sem resposta</v-chip>
                  <div v-if="entry.error" class="text-caption text-error" style="max-width: 190px">
                    {{ entry.error }}
                  </div>
                </div>

                <!-- Atalho para acompanhar continuamente o servidor recém-medido -->
                <v-chip v-if="isMonitored(entry)" size="small" color="success" variant="tonal">
                  <v-icon start size="14">mdi-check-circle-outline</v-icon>
                  <span class="hidden-sm-and-down">Monitorado</span>
                  <span class="hidden-md-and-up">OK</span>
                  <v-tooltip activator="parent" location="top">
                    Já existe um monitor DNS para este servidor
                  </v-tooltip>
                </v-chip>
                <v-btn
                  v-else
                  icon
                  size="small"
                  variant="tonal"
                  color="secondary"
                  @click="startMonitoring(entry)"
                >
                  <v-icon size="18">mdi-plus-circle-outline</v-icon>
                  <v-tooltip activator="parent" location="top">
                    Adicionar ao monitoramento
                  </v-tooltip>
                </v-btn>
              </div>
            </template>
          </v-list-item>
        </v-list>
        <div v-else class="pa-6 text-center">
          <v-icon size="44" color="secondary" class="mb-2">mdi-dns-outline</v-icon>
          <div class="text-subtitle-2 font-weight-medium">Nenhuma medição realizada</div>
          <div class="text-caption mb-3">
            Compare os servidores cadastrados para descobrir o mais rápido daqui.
          </div>
          <div class="d-flex justify-center ga-2 flex-wrap">
            <v-btn
              color="secondary"
              variant="flat"
              size="small"
              prepend-icon="mdi-checkbox-multiple-marked-outline"
              @click="batchDialog = true"
            >
              Monitorar servidores DNS
            </v-btn>
            <v-btn
              color="secondary"
              variant="tonal"
              size="small"
              prepend-icon="mdi-timer-play-outline"
              :loading="store.benchmarking"
              @click="compareNow"
            >
              Comparar servidores agora
            </v-btn>
            <v-btn
              variant="text"
              size="small"
              prepend-icon="mdi-playlist-plus"
              @click="serversDialog = true"
            >
              Gerenciar servidores
            </v-btn>
          </div>
        </div>
      </template>

      <!-- ============================================================ -->
      <!-- ABA HISTÓRICO (GRÁFICO MULTI-SÉRIE COM TOOLTIP COMPARATIVO) -->
      <!-- ============================================================ -->
      <template v-else>
        <div v-if="store.loading && formattedSeriesList.length === 0" class="pa-8 text-center">
          <v-progress-circular indeterminate color="secondary" size="36"></v-progress-circular>
          <div class="text-subtitle-2 font-weight-medium mt-3">Carregando histórico DNS…</div>
        </div>

        <div v-else-if="hasHistoryPoints" class="px-5 pt-2">
          <BaseMetricChart
            :series="chartSeries"
            unit-type="latency"
            :height="240"
            :show-legend="false"
            aria-label="Tempo de resolução DNS por servidor"
          />

          <!-- Legenda interativa: clique oculta/mostra o servidor -->
          <div class="mt-3 d-flex align-center justify-space-between flex-wrap ga-2">
            <div class="d-flex align-center flex-wrap ga-2">
              <v-chip
                v-for="s in formattedSeriesList"
                :key="s.id"
                size="small"
                :variant="hiddenSeries.has(s.id) ? 'outlined' : 'tonal'"
                color="secondary"
                class="cursor-pointer"
                :class="{ 'text-decoration-line-through': hiddenSeries.has(s.id) }"
                @click="toggleSeries(s.id)"
              >
                <span class="dns-dot mr-2" :style="{ backgroundColor: s.color }"></span>
                <span class="font-weight-medium">{{ s.label }}</span>
                <span class="ml-1">({{ protocolLabel(s.protocol) }})</span>
                <span v-if="s.avgLatency !== null" class="ml-1 font-weight-bold">
                  · {{ formatLatency(s.avgLatency) }}
                </span>
              </v-chip>
            </div>

            <v-btn
              v-if="hiddenSeries.size > 0"
              size="small"
              variant="text"
              color="secondary"
              prepend-icon="mdi-eye"
              @click="showAllSeries"
            >
              Exibir todos
            </v-btn>
          </div>
        </div>

        <!-- Estado Vazio para Histórico -->
        <div v-else class="pa-6 text-center">
          <v-icon size="44" color="secondary" class="mb-2"> mdi-chart-timeline-variant-off </v-icon>
          <div class="text-subtitle-2 font-weight-medium">
            Nenhum monitor DNS com histórico ainda
          </div>
          <div class="text-caption mb-3">
            Cadastre servidores DNS no monitoramento contínuo para acompanhar o histórico gráfico de
            resolução.
          </div>
          <div class="d-flex justify-center ga-2 flex-wrap">
            <v-btn
              color="secondary"
              variant="flat"
              size="small"
              prepend-icon="mdi-checkbox-multiple-marked-outline"
              @click="batchDialog = true"
            >
              Monitorar servidores DNS
            </v-btn>
            <v-btn
              color="secondary"
              variant="tonal"
              size="small"
              prepend-icon="mdi-timer-play-outline"
              :loading="store.benchmarking"
              @click="compareNow"
            >
              Comparar servidores agora
            </v-btn>
            <v-btn
              variant="text"
              size="small"
              prepend-icon="mdi-playlist-plus"
              @click="serversDialog = true"
            >
              Gerenciar servidores
            </v-btn>
          </div>
        </div>
      </template>
    </div>

    <template v-if="footerText" #footer>
      <div class="d-flex align-center flex-wrap ga-2 pt-2">
        <span class="text-caption">{{ footerText }}</span>
        <v-spacer></v-spacer>
        <v-btn
          v-if="mode === 'benchmark' && store.ranking.length > 0"
          variant="tonal"
          color="secondary"
          size="small"
          prepend-icon="mdi-checkbox-multiple-marked-outline"
          @click="batchDialog = true"
        >
          Monitorar em lote
        </v-btn>
        <v-btn
          variant="text"
          color="primary"
          size="small"
          append-icon="mdi-arrow-right"
          to="/monitors"
        >
          Monitores
        </v-btn>
      </div>
    </template>

    <DnsServersDialog v-model="serversDialog" @saved="onServersChanged"></DnsServersDialog>

    <DnsBatchMonitorDialog
      v-model="batchDialog"
      :initial-hostnames="store.benchmarkHostnames"
      @provisioned="onBatchProvisioned"
    ></DnsBatchMonitorDialog>

    <!-- Criação do monitor DNS já apontando para o servidor escolhido no ranking -->
    <MonitorFormDialog
      v-model="monitorDialog"
      :defaults="monitorDefaults"
      @saved="onMonitorSaved"
    ></MonitorFormDialog>
  </DashboardCard>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useDnsPerformanceStore, type DnsRankingEntry } from '@/stores/dnsPerformance'
import { useMonitorsStore, type Monitor } from '@/stores/monitors'
import DnsServersDialog from '@/components/DnsServersDialog.vue'
import DnsBatchMonitorDialog from '@/components/DnsBatchMonitorDialog.vue'
import MonitorFormDialog from '@/components/MonitorFormDialog.vue'
import type { DnsProtocol, MonitorFormModel } from '@/utils/monitorTypes'
import { formatLatency } from '@/utils/formatters'
import { CHART_STATUS_COLORS, paletteColor } from '@/utils/charts/scale'
import BaseMetricChart, { type ChartSeriesInput } from '@/components/BaseMetricChart.vue'
import DashboardCard from '@/components/dashboard/DashboardCard.vue'

const store = useDnsPerformanceStore()
const monitorsStore = useMonitorsStore()
const mode = ref<'history' | 'benchmark'>('history')
const selectedWindowHours = ref(24)
const serversDialog = ref(false)
const batchDialog = ref(false)
const monitorDialog = ref(false)
const monitorDefaults = ref<Partial<MonitorFormModel> | null>(null)

const hiddenSeries = ref<Set<string>>(new Set())

/** Nomes medidos quando a comparação ainda não definiu os seus */
const FALLBACK_HOSTNAMES = ['google.com', 'cloudflare.com', 'globo.com']

const PROTOCOL_LABELS: Record<DnsProtocol, string> = {
  udp: 'UDP',
  tcp: 'TCP',
  doh: 'DoH',
  system: 'Sistema',
}

onMounted(() => {
  store.fetchPerformance(selectedWindowHours.value)
  if (monitorsStore.monitors.length === 0) monitorsStore.fetchMonitors()
})

function onWindowHoursChange(hours: number) {
  selectedWindowHours.value = hours
  store.fetchPerformance(hours)
}

/** Endereço de um monitor DNS já cadastrado, no formato usado pelo ranking */
function monitorDnsServer(monitor: Monitor): { server: string; protocol: string } | null {
  if (monitor.type !== 'dns') return null
  const config = (monitor.configuration || {}) as Record<string, unknown>
  const protocol = String(config.protocol ?? 'udp')
  const server = String(protocol === 'doh' ? (config.dohUrl ?? '') : (config.dnsServer ?? ''))
  return server ? { server, protocol } : null
}

function isMonitored(entry: DnsRankingEntry): boolean {
  if (entry.monitorIds && entry.monitorIds.length > 0) return true

  return monitorsStore.monitors.some((monitor) => {
    const existing = monitorDnsServer(monitor)
    return existing?.server === entry.server && existing.protocol === entry.protocol
  })
}

function startMonitoring(entry: DnsRankingEntry) {
  const hostnames = store.benchmarkHostnames.length ? store.benchmarkHostnames : FALLBACK_HOSTNAMES
  const [primary, ...extras] = hostnames

  monitorDefaults.value = {
    kind: 'dns',
    target: primary,
    extraHostnames: extras,
    dnsProtocol: entry.protocol,
    dnsServer: entry.protocol === 'doh' ? '' : entry.server,
    dohUrl: entry.protocol === 'doh' ? entry.server : '',
    name: `DNS ${entry.label}`,
  }
  monitorDialog.value = true
}

async function onMonitorSaved() {
  monitorDefaults.value = null
  await monitorsStore.fetchMonitors()
  refresh()
}

function protocolLabel(protocol: DnsProtocol): string {
  return PROTOCOL_LABELS[protocol] ?? String(protocol).toUpperCase()
}

function barValue(entry: DnsRankingEntry): number {
  if (entry.avgLookupTimeMs === null || store.slowestLatency <= 0) return 0
  return Math.max(6, (entry.avgLookupTimeMs / store.slowestLatency) * 100)
}

function positionColor(index: number, entry: DnsRankingEntry): string {
  if (entry.avgLookupTimeMs === null) return 'error'
  if (index === 0) return 'success'
  if (index === 1) return 'info'
  if (index === 2) return 'warning'
  return 'primary'
}

function rangeLabel(entry: DnsRankingEntry): string {
  if (entry.minLookupTimeMs === null || entry.maxLookupTimeMs === null) return '—'
  return `min ${formatLatency(entry.minLookupTimeMs)} / máx ${formatLatency(entry.maxLookupTimeMs)}`
}

function sampleLabel(entry: DnsRankingEntry): string {
  if (store.source === 'benchmark') {
    const total = entry.totalQueries ?? 0
    const failed = entry.failedQueries ?? 0
    return `${total} consulta(s)${failed > 0 ? ` · ${failed} falha(s)` : ''}`
  }

  const total = entry.totalChecks ?? 0
  const monitors = entry.monitorIds?.length ?? 0
  return `${total} checagem(ns) · ${monitors} monitor(es) · ${entry.successRate}% de sucesso`
}

const footerText = computed(() => {
  if (store.ranking.length === 0 && store.series.length === 0) return ''
  if (store.source === 'benchmark') {
    return `Medido agora com ${store.benchmarkHostnames.join(', ')}`
  }
  return `Últimas ${store.windowHours}h · ${store.monitorCount} monitor(es) DNS cadastrado(s)`
})

function onModeChange(value: unknown) {
  if (value === 'benchmark') compareNow()
  else store.fetchPerformance(selectedWindowHours.value)
}

function compareNow() {
  mode.value = 'benchmark'
  store.runBenchmark()
}

function refresh() {
  if (mode.value === 'benchmark') store.runBenchmark()
  else store.fetchPerformance(selectedWindowHours.value)
}

function onServersChanged() {
  if (mode.value === 'benchmark') store.runBenchmark()
}

async function onBatchProvisioned() {
  mode.value = 'history'
  await monitorsStore.fetchMonitors()
  await store.fetchPerformance(selectedWindowHours.value)
}

const headerSubtitle = computed(() => {
  if (mode.value === 'benchmark') {
    return store.slowestLatency > 0
      ? `Escala alinhada: 0–${Math.round(store.slowestLatency)} ms, mais rápido primeiro`
      : 'Comparação ao vivo, mais rápido primeiro'
  }
  return formattedSeriesList.value.length > 0
    ? `${formattedSeriesList.value.length} servidores monitorados`
    : 'Histórico dos monitores DNS'
})

/* ========================================================================= */
/* HISTÓRICO MULTI-SÉRIE                                                     */
/* ========================================================================= */

/** Máximo de colunas do gráfico: cada servidor mede no seu ritmo. */
const MAX_BUCKETS = 60

interface DnsSamplePoint {
  timestampMs: number
  latencyMs: number | null
  status: string
}

interface FormattedDnsSeries {
  id: string
  server: string
  label: string
  protocol: DnsProtocol
  color: string
  points: DnsSamplePoint[]
  avgLatency: number | null
}

const hasHistoryPoints = computed(() => store.series.some((s) => s.points && s.points.length > 0))

const formattedSeriesList = computed<FormattedDnsSeries[]>(() =>
  store.series.map((s, index) => {
    const points = s.points
      .map((pt) => ({
        timestampMs: new Date(pt.timestamp).getTime(),
        latencyMs: pt.latencyMs,
        status: pt.status,
      }))
      .filter((pt) => Number.isFinite(pt.timestampMs) && pt.timestampMs > 0)
      .sort((a, b) => a.timestampMs - b.timestampMs)
    const latencies = points
      .map((p) => p.latencyMs)
      .filter((v): v is number => typeof v === 'number')
    return {
      id: `${s.server}-${s.protocol}`,
      server: s.server,
      label: s.label || s.server,
      protocol: s.protocol,
      color: paletteColor(index),
      points,
      avgLatency:
        latencies.length > 0 ? latencies.reduce((acc, v) => acc + v, 0) / latencies.length : null,
    }
  })
)

const visibleSeriesList = computed(() =>
  formattedSeriesList.value.filter((s) => !hiddenSeries.value.has(s.id))
)

/**
 * Cada servidor tem os próprios instantes de medição. Para o gráfico (que
 * alinha as séries por coluna) a janela vira colunas de mesma largura; cada
 * coluna leva a média do servidor ali, ou falta de amostra. Coluna com alguma
 * consulta falha ganha marcador vermelho.
 */
const chartSeries = computed<ChartSeriesInput[]>(() => {
  const all = visibleSeriesList.value.flatMap((s) => s.points.map((p) => p.timestampMs))
  if (all.length === 0) return []
  const min = Math.min(...all)
  const max = Math.max(...all)
  const longest = Math.max(...visibleSeriesList.value.map((s) => s.points.length))
  const buckets = Math.max(1, Math.min(MAX_BUCKETS, longest))
  const span = Math.max(1, max - min)
  const bucketOf = (t: number) => Math.min(buckets - 1, Math.floor(((t - min) / span) * buckets))
  const times = Array.from({ length: buckets }, (_, i) =>
    new Date(buckets === 1 ? max : min + ((i + 0.5) * span) / buckets).toISOString()
  )

  return visibleSeriesList.value.map((s) => {
    const sums = new Array<number>(buckets).fill(0)
    const counts = new Array<number>(buckets).fill(0)
    const failed = new Array<boolean>(buckets).fill(false)
    for (const point of s.points) {
      const b = bucketOf(point.timestampMs)
      if (typeof point.latencyMs === 'number') {
        sums[b] += point.latencyMs
        counts[b]++
      }
      if (point.status !== 'up') failed[b] = true
    }
    return {
      id: s.id,
      label: `${s.label} (${protocolLabel(s.protocol)})`,
      color: s.color,
      fillArea: false,
      data: times.map((time, b) => ({
        time,
        value: counts[b] > 0 ? sums[b] / counts[b] : null,
        color: failed[b] ? CHART_STATUS_COLORS.critical : undefined,
      })),
    }
  })
})

function toggleSeries(id: string) {
  if (hiddenSeries.value.has(id)) {
    hiddenSeries.value.delete(id)
  } else if (hiddenSeries.value.size < formattedSeriesList.value.length - 1) {
    hiddenSeries.value.add(id)
  }
}

function showAllSeries() {
  hiddenSeries.value.clear()
}
</script>

<style scoped>
.dns-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  display: inline-block;
}
</style>
