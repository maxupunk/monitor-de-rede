<template>
  <v-card elevation="2" class="rounded-lg fill-height">
    <v-card-title class="d-flex align-center justify-space-between py-3 px-4 flex-wrap ga-2">
      <div class="d-flex align-center">
        <v-icon start color="deep-purple">mdi-dns-outline</v-icon>
        <span class="font-weight-bold text-h6">Latência de DNS</span>
        <v-chip size="x-small" color="deep-purple" class="ml-2" variant="tonal">
          menor tempo primeiro
        </v-chip>
      </div>
      <div class="d-flex align-center ga-1">
        <v-btn-toggle
          v-model="mode"
          density="compact"
          variant="outlined"
          divided
          mandatory
          @update:model-value="onModeChange"
        >
          <v-btn value="history" size="small">
            Histórico
            <v-tooltip activator="parent" location="top">
              Média dos monitores DNS nas últimas {{ store.windowHours }}h
            </v-tooltip>
          </v-btn>
          <v-btn value="benchmark" size="small">
            Comparar
            <v-tooltip activator="parent" location="top">
              Mede os resolvedores públicos agora
            </v-tooltip>
          </v-btn>
        </v-btn-toggle>
        <v-btn
          icon
          size="small"
          variant="text"
          :loading="store.loading || store.benchmarking"
          @click="refresh"
        >
          <v-icon>mdi-refresh</v-icon>
          <v-tooltip activator="parent" location="top">Atualizar</v-tooltip>
        </v-btn>
        <v-btn icon size="small" variant="text" @click="serversDialog = true">
          <v-icon>mdi-cog-outline</v-icon>
          <v-tooltip activator="parent" location="top">
            Gerenciar os servidores DNS comparados
          </v-tooltip>
        </v-btn>
      </div>
    </v-card-title>
    <v-divider></v-divider>

    <v-card-text class="pa-0">
      <v-alert
        v-if="store.error"
        type="error"
        variant="tonal"
        density="compact"
        class="ma-4"
        :text="store.error"
      ></v-alert>

      <div v-if="store.benchmarking" class="pa-6 text-center">
        <v-progress-circular indeterminate color="deep-purple" size="32"></v-progress-circular>
        <div class="text-subtitle-2 font-weight-medium mt-3">Medindo resolvedores…</div>
        <div class="text-caption text-grey">
          As consultas rodam em série para a comparação ser justa.
        </div>
      </div>
      <v-list v-else-if="store.ranking.length > 0" class="py-0">
        <v-list-item
          v-for="(entry, index) in store.ranking"
          :key="`${entry.server}-${entry.protocol}`"
          class="px-4 py-3 border-b"
        >
          <template #prepend>
            <v-avatar :color="positionColor(index, entry)" size="30" class="mr-3">
              <span class="text-caption font-weight-bold">{{ index + 1 }}</span>
            </v-avatar>
          </template>

          <v-list-item-title class="d-flex align-center ga-2 flex-wrap">
            <span class="font-weight-medium">{{ entry.label }}</span>
            <v-chip size="x-small" variant="tonal" color="grey">
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
              <span class="text-caption text-grey-darken-1">
                {{ rangeLabel(entry) }}
              </span>
            </div>
            <div class="text-caption text-grey mt-1">
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
                    {{ formatMs(entry.avgLookupTimeMs) }}
                  </span>
                  <span class="text-caption text-grey">ms</span>
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
                color="deep-purple"
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
      <div v-else class="pa-6 text-center text-grey">
        <v-icon size="44" color="grey-lighten-1" class="mb-2">mdi-dns-outline</v-icon>
        <div class="text-subtitle-2 font-weight-medium">
          {{
            mode === 'history'
              ? 'Nenhum monitor DNS com histórico ainda'
              : 'Nenhuma medição realizada'
          }}
        </div>
        <div class="text-caption mb-3">
          {{
            mode === 'history'
              ? 'Cadastre um monitor DNS apontando para o servidor que deseja acompanhar.'
              : 'Compare os servidores cadastrados para descobrir o mais rápido daqui.'
          }}
        </div>
        <div class="d-flex justify-center ga-2 flex-wrap">
          <v-btn
            color="deep-purple"
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
    </v-card-text>

    <v-divider v-if="footerText"></v-divider>
    <v-card-actions v-if="footerText" class="px-4 py-2">
      <span class="text-caption text-grey">{{ footerText }}</span>
      <v-spacer></v-spacer>
      <v-btn
        variant="text"
        color="primary"
        size="small"
        append-icon="mdi-arrow-right"
        to="/monitors"
      >
        Monitores
      </v-btn>
    </v-card-actions>

    <DnsServersDialog v-model="serversDialog" @saved="onServersChanged"></DnsServersDialog>

    <!-- Criação do monitor DNS já apontando para o servidor escolhido no ranking -->
    <MonitorFormDialog
      v-model="monitorDialog"
      :defaults="monitorDefaults"
      @saved="onMonitorSaved"
    ></MonitorFormDialog>
  </v-card>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useDnsPerformanceStore, type DnsRankingEntry } from '@/stores/dnsPerformance'
import { useMonitorsStore, type Monitor } from '@/stores/monitors'
import DnsServersDialog from '@/components/DnsServersDialog.vue'
import MonitorFormDialog from '@/components/MonitorFormDialog.vue'
import type { DnsProtocol, MonitorFormModel } from '@/utils/monitorTypes'

const store = useDnsPerformanceStore()
const monitorsStore = useMonitorsStore()
const mode = ref<'history' | 'benchmark'>('history')
const serversDialog = ref(false)
const monitorDialog = ref(false)
const monitorDefaults = ref<Partial<MonitorFormModel> | null>(null)

/** Nomes medidos quando a comparação ainda não definiu os seus */
const FALLBACK_HOSTNAMES = ['google.com', 'cloudflare.com', 'globo.com']

const PROTOCOL_LABELS: Record<DnsProtocol, string> = {
  udp: 'UDP',
  tcp: 'TCP',
  doh: 'DoH',
  system: 'Sistema',
}

onMounted(() => {
  store.fetchPerformance()
  // Necessário para saber quais servidores do ranking já estão sendo vigiados
  if (monitorsStore.monitors.length === 0) monitorsStore.fetchMonitors()
})

/** Endereço de um monitor DNS já cadastrado, no formato usado pelo ranking */
function monitorDnsServer(monitor: Monitor): { server: string; protocol: string } | null {
  if (monitor.type !== 'dns') return null
  const config = (monitor.configuration || {}) as Record<string, unknown>
  const protocol = String(config.protocol ?? 'udp')
  const server = String(protocol === 'doh' ? (config.dohUrl ?? '') : (config.dnsServer ?? ''))
  return server ? { server, protocol } : null
}

/**
 * No modo histórico o próprio ranking já sabe de quais monitores veio; no modo
 * comparação a medição é avulsa, então a checagem passa pelos monitores
 * cadastrados — endereço e transporte precisam bater, porque o mesmo IP medido
 * por UDP e por DoH são dois monitores diferentes.
 */
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
  // O selo "Monitorado" e o ranking histórico dependem da lista atualizada
  await monitorsStore.fetchMonitors()
  refresh()
}

function protocolLabel(protocol: DnsProtocol): string {
  return PROTOCOL_LABELS[protocol] ?? String(protocol).toUpperCase()
}

function formatMs(value: number): string {
  return value >= 100 ? String(Math.round(value)) : value.toFixed(1)
}

/** Barra proporcional ao pior tempo do ranking — leitura comparativa imediata */
function barValue(entry: DnsRankingEntry): number {
  if (entry.avgLookupTimeMs === null || store.slowestLatency <= 0) return 0
  return Math.max(6, (entry.avgLookupTimeMs / store.slowestLatency) * 100)
}

function positionColor(index: number, entry: DnsRankingEntry): string {
  if (entry.avgLookupTimeMs === null) return 'grey'
  if (index === 0) return 'success'
  if (index === 1) return 'light-green-darken-2'
  if (index === 2) return 'amber-darken-2'
  return 'grey-darken-1'
}

function rangeLabel(entry: DnsRankingEntry): string {
  if (entry.minLookupTimeMs === null || entry.maxLookupTimeMs === null) return '—'
  return `min ${formatMs(entry.minLookupTimeMs)} / máx ${formatMs(entry.maxLookupTimeMs)} ms`
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
  if (store.ranking.length === 0) return ''
  if (store.source === 'benchmark') {
    return `Medido agora com ${store.benchmarkHostnames.join(', ')}`
  }
  return `Últimas ${store.windowHours}h · ${store.monitorCount} monitor(es) DNS`
})

function onModeChange(value: unknown) {
  if (value === 'benchmark') compareNow()
  else store.fetchPerformance()
}

function compareNow() {
  mode.value = 'benchmark'
  store.runBenchmark()
}

function refresh() {
  if (mode.value === 'benchmark') store.runBenchmark()
  else store.fetchPerformance()
}

/** Mudou a lista de servidores: refaz a comparação para refletir o cadastro */
function onServersChanged() {
  if (mode.value === 'benchmark') store.runBenchmark()
}
</script>
