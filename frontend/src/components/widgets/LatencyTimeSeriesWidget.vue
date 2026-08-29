<template>
  <v-card elevation="2" class="rounded-lg fill-height d-flex flex-column">
    <v-card-title class="d-flex align-center justify-space-between py-3 px-4 flex-wrap ga-2">
      <div>
        <div class="d-flex align-center ga-2">
          <v-icon color="info">mdi-chart-timeline-variant</v-icon>
          <span class="font-weight-bold text-h6">Latência & Perda de Pacotes</span>
        </div>
        <div class="text-caption text-grey mt-1 d-flex align-center ga-1">
          <v-icon size="14" color="info">mdi-information-outline</v-icon>
          <span>Origem: {{ sourceDescription }}</span>
        </div>
      </div>

      <div class="d-flex align-center ga-2 flex-wrap">
        <v-select
          v-model="selectedMonitorId"
          :items="monitorOptions"
          item-title="name"
          item-value="id"
          density="compact"
          variant="outlined"
          hide-details
          style="min-width: 220px; max-width: 280px"
          class="text-caption"
        ></v-select>

        <v-btn-toggle v-model="timeframe" density="compact" variant="outlined" divided mandatory>
          <v-btn value="5m" size="x-small">5m</v-btn>
          <v-btn value="15m" size="x-small">15m</v-btn>
          <v-btn value="1h" size="x-small">1h</v-btn>
          <v-btn value="24h" size="x-small">24h</v-btn>
        </v-btn-toggle>
      </div>
    </v-card-title>
    <v-divider></v-divider>

    <v-card-text class="pa-3 flex-grow-1 relative">
      <div
        ref="chartContainerRef"
        class="chart-box w-100 relative pa-2 rounded bg-surface cursor-pointer"
        @mousemove="onMouseMove"
        @mouseleave="onMouseLeave"
        @click="onChartClick"
      >
        <svg
          v-if="samples.length > 0"
          class="w-100 chart-svg"
          viewBox="0 0 800 200"
          preserveAspectRatio="none"
        >
          <defs>
            <linearGradient id="latencyGrad" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stop-color="#38bdf8" stop-opacity="0.4" />
              <stop offset="100%" stop-color="#38bdf8" stop-opacity="0.0" />
            </linearGradient>
          </defs>

          <!-- Grid Lines -->
          <line
            x1="60"
            y1="20"
            x2="780"
            y2="20"
            stroke="rgba(148, 163, 184, 0.2)"
            stroke-dasharray="3,3"
          />
          <text x="52" y="24" font-size="10" fill="#94a3b8" text-anchor="end">
            {{ maxLatencyFormatted }}
          </text>

          <line
            x1="60"
            y1="100"
            x2="780"
            y2="100"
            stroke="rgba(148, 163, 184, 0.2)"
            stroke-dasharray="3,3"
          />
          <text x="52" y="104" font-size="10" fill="#94a3b8" text-anchor="end">
            {{ midLatencyFormatted }}
          </text>

          <line
            x1="60"
            y1="180"
            x2="780"
            y2="180"
            stroke="rgba(148, 163, 184, 0.3)"
            stroke-width="1.5"
          />
          <text x="52" y="184" font-size="10" fill="#94a3b8" text-anchor="end">0 ms</text>

          <!-- Crosshair line -->
          <line
            v-if="hoverIndex !== null && crosshairX !== null"
            :x1="crosshairX"
            y1="20"
            :x2="crosshairX"
            y2="180"
            stroke="#0284c7"
            stroke-dasharray="4,4"
            stroke-width="1.5"
          />

          <!-- Gradient Fill Area -->
          <polygon v-if="areaPoints" :points="areaPoints" fill="url(#latencyGrad)" />

          <!-- Main Latency Line -->
          <polyline
            :points="polylinePoints"
            fill="none"
            stroke="#38bdf8"
            stroke-width="2.5"
            stroke-linecap="round"
            stroke-linejoin="round"
          />

          <!-- Data Circles -->
          <circle
            v-for="(pt, idx) in chartPoints"
            :key="idx"
            :cx="pt.x"
            :cy="pt.y"
            :r="hoverIndex === idx ? 7 : 3.5"
            :fill="pt.loss > 0 ? '#ef4444' : '#38bdf8'"
            stroke="#ffffff"
            stroke-width="1.5"
            class="chart-point"
          />
        </svg>

        <div v-else class="pa-8 text-center text-grey">
          <v-icon size="40" color="grey-lighten-1" class="mb-2">
            mdi-chart-timeline-variant-off
          </v-icon>
          <div class="text-subtitle-2 font-weight-medium">Sem dados de telemetria no período</div>
          <div class="text-caption text-grey mt-1">{{ sourceDescription }}</div>
        </div>

        <!-- Tooltip Card -->
        <v-card
          v-if="hoverIndex !== null && currentHoverItem"
          elevation="8"
          class="active-point-tooltip pa-2 rounded border text-white pointer-events-none"
          :style="tooltipStyle"
        >
          <div class="text-caption font-weight-bold text-info">
            Latência: {{ formatLatency(currentHoverItem.latency) }}
          </div>
          <div
            class="text-caption font-weight-medium"
            :class="currentHoverItem.loss > 0 ? 'text-error' : 'text-success'"
          >
            Perda de Pacotes: {{ currentHoverItem.loss }}%
          </div>
          <div class="text-caption text-grey-lighten-1 mt-1">Hora: {{ currentHoverItem.time }}</div>
          <div class="text-caption text-grey mt-1" style="font-size: 10px !important">
            {{ targetLabel }} (Clique para ver itens)
          </div>
        </v-card>
      </div>
    </v-card-text>

    <!-- Legend Footer -->
    <v-divider></v-divider>
    <v-card-actions
      class="px-4 py-2 bg-surface-variant d-flex align-center justify-space-between text-caption flex-wrap ga-2"
    >
      <div class="d-flex align-center ga-3">
        <div class="d-flex align-center ga-1">
          <span class="dot-indicator bg-info"></span>
          <span>Latência (ms)</span>
        </div>
        <div class="d-flex align-center ga-1">
          <span class="dot-indicator bg-error"></span>
          <span>Perda de Pacotes</span>
        </div>
      </div>
      <span class="text-grey font-weight-bold">Média: {{ formatLatency(avgLatency) }}</span>
    </v-card-actions>

    <!-- Diálogo de Detalhes da Amostragem Selecionada -->
    <v-dialog v-model="detailDialog" max-width="700px" scrollable>
      <v-card v-if="selectedSample" class="rounded-lg">
        <v-card-title
          class="d-flex align-center justify-space-between py-3 px-4 bg-surface-variant"
        >
          <div class="d-flex align-center ga-2">
            <v-icon color="info">mdi-chart-bar</v-icon>
            <span class="font-weight-bold text-subtitle-1">
              Detalhes de Telemetria — {{ selectedSample.time }}
            </span>
          </div>
          <v-btn icon size="small" variant="text" @click="detailDialog = false">
            <v-icon>mdi-close</v-icon>
          </v-btn>
        </v-card-title>
        <v-divider></v-divider>

        <v-card-text class="pa-4">
          <!-- Cards Resumo -->
          <v-row class="mb-4">
            <v-col cols="12" sm="4">
              <v-card variant="tonal" color="info" class="pa-3 rounded-lg text-center">
                <div class="text-caption font-weight-medium">Latência Registrada</div>
                <div class="text-h6 font-weight-bold mt-1">
                  {{ formatLatency(selectedSample.latency) }}
                </div>
              </v-card>
            </v-col>
            <v-col cols="12" sm="4">
              <v-card
                variant="tonal"
                :color="selectedSample.loss > 0 ? 'error' : 'success'"
                class="pa-3 rounded-lg text-center"
              >
                <div class="text-caption font-weight-medium">Perda de Pacotes</div>
                <div class="text-h6 font-weight-bold mt-1">{{ selectedSample.loss }}%</div>
              </v-card>
            </v-col>
            <v-col cols="12" sm="4">
              <v-card variant="tonal" color="primary" class="pa-3 rounded-lg text-center">
                <div class="text-caption font-weight-medium">Monitores Analisados</div>
                <div class="text-h6 font-weight-bold mt-1">
                  {{ selectedSample.monitorsDetail.length }}
                </div>
              </v-card>
            </v-col>
          </v-row>

          <!-- Tabela com itens participantes -->
          <div class="text-subtitle-2 font-weight-bold mb-2">Monitores e Alvos nesta Amostra:</div>

          <v-table density="compact" class="border rounded-lg">
            <thead>
              <tr>
                <th class="text-left font-weight-bold">Monitor</th>
                <th class="text-left font-weight-bold">Alvo / Tipo</th>
                <th class="text-left font-weight-bold">Latência</th>
                <th class="text-left font-weight-bold">Perda / Status</th>
                <th class="text-right font-weight-bold">Ação</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="item in selectedSample.monitorsDetail" :key="item.id">
                <td class="py-2">
                  <div class="font-weight-bold text-body-2">{{ item.name }}</div>
                  <div v-if="item.deviceName" class="text-caption text-grey">
                    {{ item.deviceName }}
                  </div>
                </td>
                <td>
                  <div class="text-caption font-weight-medium">{{ item.target }}</div>
                  <v-chip size="x-small" color="primary" variant="tonal" class="mt-1">
                    {{ (item.type || 'ICMP').toUpperCase() }}
                  </v-chip>
                </td>
                <td class="font-weight-bold">
                  {{ formatLatency(item.latencyMs, 'N/D') }}
                </td>
                <td>
                  <v-chip
                    size="small"
                    :color="
                      item.status === 'offline' || item.status === 'down' || item.lossPct > 0
                        ? 'error'
                        : item.status === 'warning'
                          ? 'warning'
                          : 'success'
                    "
                    variant="tonal"
                    class="font-weight-medium"
                  >
                    {{ item.lossPct > 0 ? `Perda: ${item.lossPct}%` : item.status.toUpperCase() }}
                  </v-chip>
                </td>
                <td class="text-right">
                  <v-btn
                    size="small"
                    color="primary"
                    variant="outlined"
                    prepend-icon="mdi-open-in-new"
                    @click="abrirMonitor(item.id)"
                  >
                    Ver Monitor
                  </v-btn>
                </td>
              </tr>
            </tbody>
          </v-table>
        </v-card-text>
      </v-card>
    </v-dialog>
  </v-card>

  <MonitorDetailDialog v-model="detalheAberto" :monitor-id="monitorEmDetalhe" />
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted, type CSSProperties } from 'vue'
import MonitorDetailDialog from '@/components/monitors/MonitorDetailDialog.vue'
import { useMonitorDetail } from '@/composables/useMonitorDetail'
import {
  useMonitorsStore,
  type MonitorTimeSeriesPoint,
  type MonitorTimeSeriesDetailItem,
  type MonitorTimeSeriesResponse,
} from '@/stores/monitors'
import { useEventsStore } from '@/stores/events'
import { formatLatency } from '@/utils/formatters'

const timeframe = ref<'5m' | '15m' | '1h' | '24h'>('15m')
const selectedMonitorId = ref<number | 'all'>('all')
const monitorsStore = useMonitorsStore()
const eventsStore = useEventsStore()
const { detalheAberto, monitorEmDetalhe, abrirDetalhe } = useMonitorDetail()

const loading = ref(false)
const timeSeriesResponse = ref<MonitorTimeSeriesResponse | null>(null)
const localSamples = ref<MonitorTimeSeriesPoint[]>([])

const chartContainerRef = ref<HTMLElement | null>(null)
const mousePos = ref<{ x: number; y: number } | null>(null)
const hoverIndex = ref<number | null>(null)
const detailDialog = ref(false)
const selectedSample = ref<MonitorTimeSeriesPoint | null>(null)

let unbindEvent: (() => void) | null = null

const monitorOptions = computed(() => {
  const options: Array<{ id: number | 'all'; name: string }> = [
    { id: 'all', name: 'Todos os Monitores Ping (Média Global)' },
  ]
  for (const m of monitorsStore.monitors.filter((m) => m.type === 'ping')) {
    options.push({
      id: m.id,
      name: `${m.name} (${m.target})`,
    })
  }
  return options
})

const selectedMonitor = computed(() => {
  if (selectedMonitorId.value === 'all') return null
  return monitorsStore.monitors.find((m) => m.id === selectedMonitorId.value) || null
})

const pingMonitors = computed(() => monitorsStore.monitors.filter((m) => m.type === 'ping'))

const sourceDescription = computed(() => {
  if (selectedMonitor.value) {
    return `${selectedMonitor.value.name} — ${selectedMonitor.value.target} (PING ICMP)`
  }
  const total = pingMonitors.value.length
  return total > 0
    ? `Média consolidada dos ${total} monitores Ping (ICMP) ativos`
    : 'Média consolidada dos monitores Ping (ICMP)'
})

const targetLabel = computed(() => {
  if (selectedMonitor.value) {
    return `Alvo: ${selectedMonitor.value.target}`
  }
  return 'Origem: Média Global dos Monitores Ping'
})

async function loadTimeSeries() {
  loading.value = true
  try {
    const res = await monitorsStore.fetchTimeSeries({
      monitorId: selectedMonitorId.value,
      monitorType: 'ping',
      timeframe: timeframe.value,
    })
    timeSeriesResponse.value = res
    if (res && Array.isArray(res.samples)) {
      localSamples.value = res.samples
    } else {
      localSamples.value = []
    }
  } catch {
    localSamples.value = []
  } finally {
    loading.value = false
  }
}

watch([timeframe, selectedMonitorId], () => {
  loadTimeSeries()
})

onMounted(async () => {
  if (monitorsStore.monitors.length === 0) {
    await monitorsStore.fetchMonitors()
  }
  await loadTimeSeries()

  // Escuta resultados em tempo real para atualizar o último ponto / balde
  unbindEvent = eventsStore.onEvent('monitor:result', (data) => {
    const monId = Number(data.monitorId ?? data.id)
    if (!monId) return

    const isMatch = selectedMonitorId.value === 'all' || selectedMonitorId.value === monId

    if (!isMatch) return

    const mon = monitorsStore.monitors.find((m) => m.id === monId)
    if (!mon || mon.type !== 'ping') return

    const latency = typeof data.latencyMs === 'number' ? data.latencyMs : null
    const isDown = data.status === 'down' || data.status === 'offline'
    const now = new Date()

    if (localSamples.value.length > 0) {
      const lastPoint = localSamples.value[localSamples.value.length - 1]
      const diffMs = now.getTime() - lastPoint.timestamp

      // Se a última amostra for recente (< 45s), atualiza o ponto; senão anexa novo
      if (diffMs < 45000 && latency !== null) {
        lastPoint.latency = Number(((lastPoint.latency + latency) / 2).toFixed(1))
        lastPoint.loss = isDown ? 100 : 0

        const detailIdx = lastPoint.monitorsDetail.findIndex((d) => d.id === monId)
        const detailItem: MonitorTimeSeriesDetailItem = {
          id: monId,
          name: mon.name,
          target: mon.target,
          type: mon.type,
          deviceName: mon.device?.name,
          status: String(data.status ?? 'up'),
          latencyMs: latency,
          lossPct: isDown ? 100 : 0,
        }

        if (detailIdx !== -1) {
          lastPoint.monitorsDetail[detailIdx] = detailItem
        } else {
          lastPoint.monitorsDetail.push(detailItem)
        }
      } else if (latency !== null) {
        const timeStr = now.toLocaleTimeString([], {
          hour: '2-digit',
          minute: '2-digit',
          second: '2-digit',
        })
        localSamples.value.push({
          time: timeStr,
          timestamp: now.getTime(),
          latency,
          loss: isDown ? 100 : 0,
          monitorsDetail: [
            {
              id: monId,
              name: mon.name,
              target: mon.target,
              type: mon.type,
              deviceName: mon.device?.name,
              status: String(data.status ?? 'up'),
              latencyMs: latency,
              lossPct: isDown ? 100 : 0,
            },
          ],
        })

        if (localSamples.value.length > 50) {
          localSamples.value.shift()
        }
      }
    }
  })
})

onUnmounted(() => {
  if (unbindEvent) {
    unbindEvent()
    unbindEvent = null
  }
})

const samples = computed(() => localSamples.value)

const maxLatency = computed(() => {
  if (samples.value.length === 0) return 100
  const max = Math.max(...samples.value.map((s) => s.latency))
  return max > 0 ? Math.ceil(max * 1.2) : 50
})

const avgLatency = computed(() => {
  if (timeSeriesResponse.value && timeSeriesResponse.value.avgLatency > 0) {
    return timeSeriesResponse.value.avgLatency
  }
  const valid = samples.value.filter((s) => s.latency > 0)
  if (valid.length === 0) return 0
  const sum = valid.reduce((acc, s) => acc + s.latency, 0)
  return Math.round(sum / valid.length)
})

const maxLatencyFormatted = computed(() => formatLatency(maxLatency.value))
const midLatencyFormatted = computed(() => formatLatency(maxLatency.value / 2))

const chartPoints = computed(() => {
  const left = 60
  const right = 780
  const top = 20
  const bottom = 180
  const height = bottom - top
  const count = samples.value.length

  if (count === 0) return []

  const step = count > 1 ? (right - left) / (count - 1) : 0

  return samples.value.map((s, idx) => {
    const x = count === 1 ? (left + right) / 2 : left + idx * step
    const ratio = maxLatency.value > 0 ? Math.min(1, s.latency / maxLatency.value) : 0
    const y = bottom - ratio * height
    return {
      x,
      y,
      latency: s.latency,
      loss: s.loss,
      time: s.time,
      monitorsDetail: s.monitorsDetail,
    }
  })
})

const polylinePoints = computed(() => {
  return chartPoints.value.map((pt) => `${pt.x.toFixed(1)},${pt.y.toFixed(1)}`).join(' ')
})

const areaPoints = computed(() => {
  if (chartPoints.value.length === 0) return ''
  const firstX = chartPoints.value[0].x.toFixed(1)
  const lastX = chartPoints.value[chartPoints.value.length - 1].x.toFixed(1)
  return `${firstX},180 ${polylinePoints.value} ${lastX},180`
})

const crosshairX = computed(() => {
  if (hoverIndex.value === null || !chartPoints.value[hoverIndex.value]) return null
  return chartPoints.value[hoverIndex.value].x
})

const currentHoverItem = computed(() => {
  if (hoverIndex.value === null) return null
  return chartPoints.value[hoverIndex.value] || null
})

function onMouseMove(e: MouseEvent) {
  if (!chartContainerRef.value || samples.value.length === 0) return
  const rect = chartContainerRef.value.getBoundingClientRect()
  const mouseX = e.clientX - rect.left
  const mouseY = e.clientY - rect.top
  mousePos.value = { x: mouseX, y: mouseY }

  const marginX = (60 / 800) * rect.width
  const contentW = ((780 - 60) / 800) * rect.width
  const count = samples.value.length

  let relX = mouseX - marginX
  if (relX < 0) relX = 0
  if (relX > contentW) relX = contentW

  const step = count > 1 ? contentW / (count - 1) : 0
  const idx = step > 0 ? Math.round(relX / step) : 0
  hoverIndex.value = Math.min(count - 1, Math.max(0, idx))
}

function onMouseLeave() {
  mousePos.value = null
  hoverIndex.value = null
}

function onChartClick() {
  if (hoverIndex.value !== null && samples.value[hoverIndex.value]) {
    selectedSample.value = samples.value[hoverIndex.value]
    detailDialog.value = true
  }
}

/**
 * O monitor abre no **mesmo** diálogo das demais listas, e não em outra tela.
 * Ver `useMonitorDetail`: a regra de "como se abre um monitor" é uma só.
 */
function abrirMonitor(id: number) {
  detailDialog.value = false
  abrirDetalhe(id)
}

const tooltipStyle = computed<CSSProperties>(() => {
  if (!mousePos.value || !chartContainerRef.value) return {}
  const { x, y } = mousePos.value
  const rect = chartContainerRef.value.getBoundingClientRect()

  const cardW = 190
  const cardH = 80

  let left = x + 12
  if (x > rect.width - cardW - 10) left = x - cardW - 12
  let top = y - cardH - 12
  if (y < cardH + 10) top = y + 12

  return {
    position: 'absolute',
    left: `${Math.max(4, Math.min(rect.width - cardW - 4, left))}px`,
    top: `${Math.max(4, Math.min(rect.height - cardH - 4, top))}px`,
    zIndex: 20,
    background: '#0F172A',
    borderColor: '#0284C7',
    boxShadow: '0 8px 20px rgba(0,0,0,0.4)',
  }
})
</script>

<style scoped>
.chart-box {
  min-height: 210px;
  position: relative;
}

.chart-svg {
  height: 200px;
  overflow: visible;
}

.dot-indicator {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  display: inline-block;
}

.pointer-events-none {
  pointer-events: none;
}

.ga-1 {
  gap: 4px;
}
.ga-2 {
  gap: 8px;
}
.ga-3 {
  gap: 12px;
}
</style>
