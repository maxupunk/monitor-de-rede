<template>
  <v-card elevation="2" class="rounded-lg fill-height d-flex flex-column">
    <v-card-title
      class="d-flex align-center justify-space-between py-3 px-4 flex-wrap ga-2 border-b"
    >
      <div class="d-flex align-center">
        <v-icon start color="deep-purple">mdi-dns-outline</v-icon>
        <span class="font-weight-bold text-h6">Tempo de Resolução DNS</span>
        <v-chip
          v-if="mode === 'benchmark'"
          size="x-small"
          color="deep-purple"
          class="ml-2"
          variant="tonal"
        >
          Escala Alinhada ({{
            store.slowestLatency > 0
              ? `0-${Math.round(store.slowestLatency)}ms`
              : 'menor tempo de consulta primeiro'
          }})
        </v-chip>
        <v-chip
          v-else-if="formattedSeriesList.length > 0"
          size="x-small"
          color="deep-purple"
          class="ml-2"
          variant="tonal"
        >
          {{ formattedSeriesList.length }} servidores monitorados
        </v-chip>
      </div>
      <div class="d-flex align-center ga-1 flex-wrap">
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
              Gráfico temporal dos monitores DNS
            </v-tooltip>
          </v-btn>
          <v-btn value="benchmark" size="small">
            Comparar
            <v-tooltip activator="parent" location="top">
              Mede os resolvedores públicos agora
            </v-tooltip>
          </v-btn>
        </v-btn-toggle>

        <v-btn-toggle
          v-if="mode === 'history'"
          v-model="selectedWindowHours"
          density="compact"
          variant="outlined"
          divided
          mandatory
          class="ml-1"
          @update:model-value="onWindowHoursChange"
        >
          <v-btn :value="1" size="x-small">1h</v-btn>
          <v-btn :value="6" size="x-small">6h</v-btn>
          <v-btn :value="24" size="x-small">24h</v-btn>
          <v-btn :value="168" size="x-small">7d</v-btn>
        </v-btn-toggle>

        <v-btn
          size="small"
          color="deep-purple"
          variant="tonal"
          prepend-icon="mdi-checkbox-multiple-marked-outline"
          class="text-none ml-1"
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

    <v-card-text class="pa-0 flex-grow-1">
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
                      {{ formatLatency(entry.avgLookupTimeMs, '—') }}
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
          <div class="text-subtitle-2 font-weight-medium">Nenhuma medição realizada</div>
          <div class="text-caption mb-3">
            Compare os servidores cadastrados para descobrir o mais rápido daqui.
          </div>
          <div class="d-flex justify-center ga-2 flex-wrap">
            <v-btn
              color="deep-purple"
              variant="flat"
              size="small"
              prepend-icon="mdi-checkbox-multiple-marked-outline"
              @click="batchDialog = true"
            >
              Monitorar servidores DNS
            </v-btn>
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
      </template>

      <!-- ============================================================ -->
      <!-- ABA HISTÓRICO (GRÁFICO MULTI-SÉRIE COM TOOLTIP COMPARATIVO) -->
      <!-- ============================================================ -->
      <template v-else>
        <div v-if="store.loading && formattedSeriesList.length === 0" class="pa-8 text-center">
          <v-progress-circular indeterminate color="deep-purple" size="36"></v-progress-circular>
          <div class="text-subtitle-2 font-weight-medium mt-3">Carregando histórico DNS…</div>
        </div>

        <div v-else-if="hasHistoryPoints" class="pa-3">
          <!-- Container do Gráfico SVG Interativo -->
          <div
            ref="chartContainerRef"
            class="dns-chart-container relative pa-2 rounded bg-surface border"
            @mousemove="onMouseMove"
            @mouseleave="onMouseLeave"
          >
            <svg class="w-100 dns-chart-svg" viewBox="0 0 800 240" preserveAspectRatio="none">
              <!-- Linhas de Grade e Eixo Y -->
              <line
                x1="65"
                y1="25"
                x2="780"
                y2="25"
                stroke="rgba(148, 163, 184, 0.2)"
                stroke-dasharray="3,3"
              />
              <text x="56" y="29" font-size="10" fill="#94a3b8" text-anchor="end">
                {{ formatLatency(maxLatencyVal) }}
              </text>

              <line
                x1="65"
                y1="110"
                x2="780"
                y2="110"
                stroke="rgba(148, 163, 184, 0.2)"
                stroke-dasharray="3,3"
              />
              <text x="56" y="114" font-size="10" fill="#94a3b8" text-anchor="end">
                {{ formatLatency(maxLatencyVal / 2) }}
              </text>

              <line
                x1="65"
                y1="195"
                x2="780"
                y2="195"
                stroke="rgba(148, 163, 184, 0.3)"
                stroke-width="1.5"
              />
              <text x="56" y="199" font-size="10" fill="#94a3b8" text-anchor="end">0 ms</text>

              <!-- Linha Vertical Guia (Crosshair) ao passar o mouse -->
              <line
                v-if="crosshairX !== null"
                :x1="crosshairX"
                y1="25"
                :x2="crosshairX"
                y2="195"
                stroke="#9333ea"
                stroke-dasharray="3,3"
                stroke-width="1.5"
              />

              <!-- Polylines de cada servidor DNS visível -->
              <polyline
                v-for="s in visibleSeriesList"
                :key="`line-${s.id}`"
                :points="s.polylinePoints"
                fill="none"
                :stroke="s.color"
                stroke-width="2.5"
                stroke-linecap="round"
                stroke-linejoin="round"
                class="dns-polyline"
              />

              <!-- Pontos de dados (círculos) -->
              <g v-for="s in visibleSeriesList" :key="`points-${s.id}`">
                <circle
                  v-for="(pt, pIdx) in s.points"
                  :key="pIdx"
                  :cx="pt.x"
                  :cy="pt.y"
                  :r="isPointActive(s.id, pt) ? 5.5 : 2.5"
                  :fill="pt.status === 'up' ? s.color : '#ef4444'"
                  stroke="#ffffff"
                  :stroke-width="isPointActive(s.id, pt) ? 2 : 1"
                  class="dns-point"
                />
              </g>

              <!-- Marcações de Tempo no Eixo X -->
              <g v-for="(tick, tIdx) in timeTicks" :key="tIdx">
                <text :x="tick.x" y="218" font-size="10" fill="#94a3b8" :text-anchor="tick.anchor">
                  {{ tick.label }}
                </text>
              </g>
            </svg>

            <!-- Tooltip Flutuante com Lista de Todos os Servidores no Momento -->
            <v-card
              v-if="hoverSnapshot && mousePos"
              elevation="10"
              class="active-point-tooltip pa-3 text-white pointer-events-none"
              :style="tooltipStyle"
            >
              <div class="d-flex align-center justify-space-between ga-2 border-b pb-1 mb-2">
                <div
                  class="d-flex align-center ga-1 text-caption font-weight-bold text-deep-purple-lighten-3"
                >
                  <v-icon size="14" color="deep-purple-lighten-3">mdi-clock-outline</v-icon>
                  <span>{{ formatShortDateTime(hoverSnapshot.timestamp) }}</span>
                </div>
                <v-chip size="x-small" color="deep-purple" variant="flat">
                  {{ hoverSnapshot.items.length }} servidores
                </v-chip>
              </div>

              <div class="dns-tooltip-list">
                <div
                  v-for="(item, idx) in hoverSnapshot.items"
                  :key="item.id"
                  class="d-flex align-center justify-space-between py-1 ga-2"
                  :class="{ 'opacity-50': item.latencyMs === null }"
                >
                  <div class="d-flex align-center ga-2 text-truncate" style="max-width: 175px">
                    <span
                      class="dns-dot flex-shrink-0"
                      :style="{ backgroundColor: item.color }"
                    ></span>
                    <span class="text-caption font-weight-medium text-truncate text-white">
                      {{ item.label }}
                    </span>
                    <span
                      class="text-caption text-grey flex-shrink-0 font-weight-light"
                      style="font-size: 10px"
                    >
                      ({{ protocolLabel(item.protocol) }})
                    </span>
                  </div>

                  <div class="d-flex align-center ga-1 flex-shrink-0">
                    <v-icon
                      v-if="idx === 0 && item.latencyMs !== null"
                      size="12"
                      color="amber"
                      class="mr-0.5"
                    >
                      mdi-trophy
                    </v-icon>
                    <span
                      v-if="item.latencyMs !== null"
                      class="text-caption font-weight-bold"
                      :class="idx === 0 ? 'text-amber-lighten-2' : 'text-cyan-lighten-3'"
                    >
                      {{ formatLatency(item.latencyMs) }}
                    </span>
                    <span v-else class="text-caption text-error font-weight-medium"> falha </span>
                  </div>
                </div>
              </div>
            </v-card>
          </div>

          <!-- Legenda Interativa de Servidores (Clique para ocultar/isolar) -->
          <div class="mt-3 d-flex align-center justify-space-between flex-wrap ga-2 px-1">
            <div class="d-flex align-center flex-wrap ga-1.5">
              <v-chip
                v-for="s in formattedSeriesList"
                :key="s.id"
                size="small"
                :variant="hiddenSeries.has(s.id) ? 'outlined' : 'tonal'"
                class="cursor-pointer transition-swing"
                :class="{ 'opacity-50 text-decoration-line-through': hiddenSeries.has(s.id) }"
                @click="toggleSeries(s.id)"
              >
                <span class="dns-dot mr-1.5" :style="{ backgroundColor: s.color }"></span>
                <span class="font-weight-medium">{{ s.label }}</span>
                <span class="text-caption text-grey ml-1">({{ protocolLabel(s.protocol) }})</span>
                <span v-if="s.avgLatency !== null" class="ml-1 font-weight-bold text-deep-purple">
                  · {{ formatLatency(s.avgLatency) }}
                </span>
              </v-chip>
            </div>

            <v-btn
              v-if="hiddenSeries.size > 0"
              size="x-small"
              variant="text"
              color="deep-purple"
              prepend-icon="mdi-eye"
              @click="showAllSeries"
            >
              Exibir todos
            </v-btn>
          </div>
        </div>

        <!-- Estado Vazio para Histórico -->
        <div v-else class="pa-6 text-center text-grey">
          <v-icon size="44" color="grey-lighten-1" class="mb-2">
            mdi-chart-timeline-variant-off
          </v-icon>
          <div class="text-subtitle-2 font-weight-medium">
            Nenhum monitor DNS com histórico ainda
          </div>
          <div class="text-caption mb-3">
            Cadastre servidores DNS no monitoramento contínuo para acompanhar o histórico gráfico de
            resolução.
          </div>
          <div class="d-flex justify-center ga-2 flex-wrap">
            <v-btn
              color="deep-purple"
              variant="flat"
              size="small"
              prepend-icon="mdi-checkbox-multiple-marked-outline"
              @click="batchDialog = true"
            >
              Monitorar servidores DNS
            </v-btn>
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
      </template>
    </v-card-text>

    <v-divider v-if="footerText"></v-divider>
    <v-card-actions v-if="footerText" class="px-4 py-2">
      <span class="text-caption text-grey">{{ footerText }}</span>
      <v-spacer></v-spacer>
      <v-btn
        v-if="mode === 'benchmark' && store.ranking.length > 0"
        variant="tonal"
        color="deep-purple"
        size="small"
        prepend-icon="mdi-checkbox-multiple-marked-outline"
        class="mr-2"
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
    </v-card-actions>

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
  </v-card>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, type CSSProperties } from 'vue'
import { useDnsPerformanceStore, type DnsRankingEntry } from '@/stores/dnsPerformance'
import { useMonitorsStore, type Monitor } from '@/stores/monitors'
import DnsServersDialog from '@/components/DnsServersDialog.vue'
import DnsBatchMonitorDialog from '@/components/DnsBatchMonitorDialog.vue'
import MonitorFormDialog from '@/components/MonitorFormDialog.vue'
import type { DnsProtocol, MonitorFormModel } from '@/utils/monitorTypes'
import { formatLatency, formatShortDateTime } from '@/utils/formatters'

const store = useDnsPerformanceStore()
const monitorsStore = useMonitorsStore()
const mode = ref<'history' | 'benchmark'>('history')
const selectedWindowHours = ref(24)
const serversDialog = ref(false)
const batchDialog = ref(false)
const monitorDialog = ref(false)
const monitorDefaults = ref<Partial<MonitorFormModel> | null>(null)

const chartContainerRef = ref<HTMLElement | null>(null)
const mousePos = ref<{ x: number; y: number } | null>(null)
const hoverTimeMs = ref<number | null>(null)
const hiddenSeries = ref<Set<string>>(new Set())

/** Nomes medidos quando a comparação ainda não definiu os seus */
const FALLBACK_HOSTNAMES = ['google.com', 'cloudflare.com', 'globo.com']

const PROTOCOL_LABELS: Record<DnsProtocol, string> = {
  udp: 'UDP',
  tcp: 'TCP',
  doh: 'DoH',
  system: 'Sistema',
}

const PRESET_COLORS: Record<string, string> = {
  '1.1.1.1': '#f97316',
  '1.0.0.1': '#fb923c',
  '8.8.8.8': '#3b82f6',
  '8.8.4.4': '#60a5fa',
  '9.9.9.9': '#10b981',
  '149.112.112.112': '#34d399',
  '208.67.222.222': '#8b5cf6',
  '208.67.220.220': '#a78bfa',
  'dns.adguard.com': '#06b6d4',
  '8.26.56.26': '#ec4899',
}

const PALETTE = [
  '#f97316',
  '#3b82f6',
  '#10b981',
  '#8b5cf6',
  '#06b6d4',
  '#ec4899',
  '#f59e0b',
  '#6366f1',
  '#14b8a6',
  '#e11d48',
]

function getSeriesColor(server: string, index: number): string {
  const clean = server.toLowerCase().trim()
  for (const [key, color] of Object.entries(PRESET_COLORS)) {
    if (clean.includes(key.toLowerCase())) return color
  }
  return PALETTE[index % PALETTE.length]
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
  if (entry.avgLookupTimeMs === null) return 'grey'
  if (index === 0) return 'success'
  if (index === 1) return 'light-green-darken-2'
  if (index === 2) return 'amber-darken-2'
  return 'grey-darken-1'
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

/* ========================================================================= */
/* LÓGICA DO GRÁFICO MULTI-SÉRIE DE HISTÓRICO                                */
/* ========================================================================= */

interface ChartPoint {
  timestampMs: number
  timestampIso: string
  latencyMs: number | null
  status: string
  x: number
  y: number
}

interface FormattedDnsSeries {
  id: string
  server: string
  label: string
  protocol: DnsProtocol
  color: string
  points: ChartPoint[]
  polylinePoints: string
  avgLatency: number | null
}

const hasHistoryPoints = computed(() => {
  return store.series.some((s) => s.points && s.points.length > 0)
})

const timeRange = computed(() => {
  let min = Infinity
  let max = -Infinity

  for (const s of store.series) {
    for (const pt of s.points) {
      const t = new Date(pt.timestamp).getTime()
      if (!isNaN(t)) {
        if (t < min) min = t
        if (t > max) max = t
      }
    }
  }

  if (min === Infinity || max === -Infinity || min === max) {
    const now = Date.now()
    return { min: now - selectedWindowHours.value * 3600 * 1000, max: now }
  }

  return { min, max }
})

const maxLatencyVal = computed(() => {
  let max = 0
  for (const s of store.series) {
    for (const pt of s.points) {
      if (typeof pt.latencyMs === 'number' && pt.latencyMs > max) {
        max = pt.latencyMs
      }
    }
  }
  return max > 0 ? Math.max(10, max * 1.15) : 50
})

const formattedSeriesList = computed<FormattedDnsSeries[]>(() => {
  const svgLeft = 65
  const svgRight = 780
  const svgTop = 25
  const svgBottom = 195
  const { min: minTime, max: maxTime } = timeRange.value
  const maxLat = maxLatencyVal.value

  return store.series.map((s, sIdx) => {
    const id = `${s.server}-${s.protocol}`
    const color = getSeriesColor(s.server, sIdx)

    const validPoints = s.points
      .map((pt) => {
        const t = new Date(pt.timestamp).getTime()
        return {
          timestampMs: isNaN(t) ? 0 : t,
          timestampIso: pt.timestamp,
          latencyMs: pt.latencyMs,
          status: pt.status,
        }
      })
      .filter((pt) => pt.timestampMs > 0)
      .sort((a, b) => a.timestampMs - b.timestampMs)

    const points: ChartPoint[] = validPoints.map((pt) => {
      const timeRatio = maxTime > minTime ? (pt.timestampMs - minTime) / (maxTime - minTime) : 0.5
      const x = svgLeft + timeRatio * (svgRight - svgLeft)

      const latVal = pt.latencyMs ?? 0
      const latRatio = maxLat > 0 ? latVal / maxLat : 0
      const y = svgBottom - latRatio * (svgBottom - svgTop)

      return {
        timestampMs: pt.timestampMs,
        timestampIso: pt.timestampIso,
        latencyMs: pt.latencyMs,
        status: pt.status,
        x,
        y,
      }
    })

    const polylinePoints = points.map((pt) => `${pt.x.toFixed(1)},${pt.y.toFixed(1)}`).join(' ')

    const validLatencies = points
      .map((p) => p.latencyMs)
      .filter((v): v is number => typeof v === 'number')
    const avgLatency =
      validLatencies.length > 0
        ? validLatencies.reduce((acc, v) => acc + v, 0) / validLatencies.length
        : null

    return {
      id,
      server: s.server,
      label: s.label || s.server,
      protocol: s.protocol,
      color,
      points,
      polylinePoints,
      avgLatency,
    }
  })
})

const visibleSeriesList = computed(() => {
  return formattedSeriesList.value.filter((s) => !hiddenSeries.value.has(s.id))
})

const timeTicks = computed(() => {
  const { min, max } = timeRange.value
  if (max <= min) return []

  const ticksCount = 5
  const svgLeft = 65
  const svgRight = 780
  const ticks: Array<{ x: number; label: string; anchor: string }> = []

  for (let i = 0; i < ticksCount; i++) {
    const ratio = i / (ticksCount - 1)
    const timeMs = min + ratio * (max - min)
    const x = svgLeft + ratio * (svgRight - svgLeft)
    const date = new Date(timeMs)

    const hoursLabel = date.toLocaleTimeString('pt-BR', { hour: '2-digit', minute: '2-digit' })
    const anchor = i === 0 ? 'start' : i === ticksCount - 1 ? 'end' : 'middle'

    ticks.push({ x, label: hoursLabel, anchor })
  }

  return ticks
})

function onMouseMove(e: MouseEvent) {
  if (!chartContainerRef.value) return
  const rect = chartContainerRef.value.getBoundingClientRect()
  const mouseX = e.clientX - rect.left
  const mouseY = e.clientY - rect.top
  mousePos.value = { x: mouseX, y: mouseY }

  const svgWidth = rect.width
  if (svgWidth <= 0) return

  const marginX = (65 / 800) * svgWidth
  const contentWidth = ((780 - 65) / 800) * svgWidth

  if (contentWidth <= 0 || !timeRange.value) return

  let relX = mouseX - marginX
  if (relX < 0) relX = 0
  if (relX > contentWidth) relX = contentWidth

  const ratio = relX / contentWidth
  hoverTimeMs.value = timeRange.value.min + ratio * (timeRange.value.max - timeRange.value.min)
}

function onMouseLeave() {
  mousePos.value = null
  hoverTimeMs.value = null
}

const crosshairX = computed(() => {
  if (hoverTimeMs.value === null || !timeRange.value) return null
  const { min, max } = timeRange.value
  if (max <= min) return null
  const ratio = (hoverTimeMs.value - min) / (max - min)
  return 65 + ratio * (780 - 65)
})

const hoverSnapshot = computed(() => {
  if (hoverTimeMs.value === null || visibleSeriesList.value.length === 0) return null

  const targetTime = hoverTimeMs.value
  let closestTimestamp = ''
  let minDiff = Infinity

  const items = visibleSeriesList.value
    .map((s) => {
      if (s.points.length === 0) return null

      let bestPt = s.points[0]
      let bestDiff = Math.abs(bestPt.timestampMs - targetTime)

      for (let i = 1; i < s.points.length; i++) {
        const diff = Math.abs(s.points[i].timestampMs - targetTime)
        if (diff < bestDiff) {
          bestDiff = diff
          bestPt = s.points[i]
        }
      }

      if (bestDiff < minDiff) {
        minDiff = bestDiff
        closestTimestamp = bestPt.timestampIso
      }

      return {
        id: s.id,
        server: s.server,
        label: s.label,
        protocol: s.protocol,
        color: s.color,
        latencyMs: bestPt.latencyMs,
        status: bestPt.status,
        pointX: bestPt.x,
        pointY: bestPt.y,
        timeDiff: bestDiff,
        timestamp: bestPt.timestampIso,
      }
    })
    .filter((item): item is NonNullable<typeof item> => item !== null)

  items.sort((a, b) => {
    if (a.latencyMs === null && b.latencyMs === null) return 0
    if (a.latencyMs === null) return 1
    if (b.latencyMs === null) return -1
    return a.latencyMs - b.latencyMs
  })

  return {
    timestamp: closestTimestamp,
    items,
  }
})

function isPointActive(seriesId: string, pt: ChartPoint): boolean {
  if (!hoverSnapshot.value) return false
  const match = hoverSnapshot.value.items.find((item) => item.id === seriesId)
  if (!match) return false
  return Math.abs(match.pointX - pt.x) < 2 && Math.abs(match.pointY - pt.y) < 2
}

const tooltipStyle = computed<CSSProperties>(() => {
  if (!mousePos.value || !chartContainerRef.value) return {}
  const { x, y } = mousePos.value
  const rect = chartContainerRef.value.getBoundingClientRect()

  const cardWidth = 280
  const cardHeight = Math.min(300, 60 + (hoverSnapshot.value?.items.length || 1) * 28)

  let left = x + 16
  if (x > rect.width - cardWidth - 20) {
    left = x - cardWidth - 16
  }

  let top = y - cardHeight / 2
  if (top < 10) top = 10
  if (top + cardHeight > rect.height - 10) {
    top = Math.max(10, rect.height - cardHeight - 10)
  }

  left = Math.max(8, Math.min(rect.width - cardWidth - 8, left))

  return {
    position: 'absolute',
    left: `${left}px`,
    top: `${top}px`,
    pointerEvents: 'none',
    zIndex: 40,
    background: 'rgba(15, 23, 42, 0.95)',
    backdropFilter: 'blur(8px)',
    border: '1px solid rgba(147, 51, 234, 0.5)',
    boxShadow: '0 10px 25px -5px rgba(0, 0, 0, 0.5), 0 8px 10px -6px rgba(0, 0, 0, 0.5)',
    borderRadius: '8px',
    maxWidth: `${cardWidth}px`,
    width: `${cardWidth}px`,
    transition: 'left 0.06s ease-out, top 0.06s ease-out',
  }
})

function toggleSeries(id: string) {
  if (hiddenSeries.value.has(id)) {
    hiddenSeries.value.delete(id)
  } else {
    if (hiddenSeries.value.size < formattedSeriesList.value.length - 1) {
      hiddenSeries.value.add(id)
    }
  }
}

function showAllSeries() {
  hiddenSeries.value.clear()
}
</script>

<style scoped>
.dns-chart-container {
  position: relative;
  min-height: 240px;
  user-select: none;
}

.dns-chart-svg {
  height: 240px;
  overflow: visible;
}

.dns-polyline {
  transition: stroke-width 0.15s ease;
}

.dns-point {
  transition:
    r 0.15s ease,
    fill 0.15s ease;
}

.dns-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  display: inline-block;
}

.dns-tooltip-list {
  max-height: 220px;
  overflow-y: auto;
}

.pointer-events-none {
  pointer-events: none;
}
</style>
