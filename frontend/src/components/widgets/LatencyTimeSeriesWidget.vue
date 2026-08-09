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
            Latência: {{ currentHoverItem.latency.toFixed(1) }} ms
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
      <span class="text-grey font-weight-bold">Média: {{ avgLatency }} ms</span>
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
                <div class="text-h6 font-weight-bold mt-1">{{ selectedSample.latency }} ms</div>
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
                  {{ item.latencyMs !== null ? `${item.latencyMs} ms` : 'N/D' }}
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
                    @click="goToMonitor(item.id)"
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
</template>

<script setup lang="ts">
import { ref, computed, type CSSProperties } from 'vue'
import { useRouter } from 'vue-router'
import { useMonitorsStore } from '@/stores/monitors'

const timeframe = ref<'5m' | '15m' | '1h' | '24h'>('15m')
const selectedMonitorId = ref<number | 'all'>('all')
const monitorsStore = useMonitorsStore()
const router = useRouter()

const chartContainerRef = ref<HTMLElement | null>(null)
const mousePos = ref<{ x: number; y: number } | null>(null)
const hoverIndex = ref<number | null>(null)
const detailDialog = ref(false)
const selectedSample = ref<SamplePoint | null>(null)

interface MonitorDetailItem {
  id: number
  name: string
  target: string
  type: string
  deviceName?: string
  status: string
  latencyMs: number | null
  lossPct: number
}

interface SamplePoint {
  time: string
  latency: number
  loss: number
  monitorsDetail: MonitorDetailItem[]
}

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

// Compila amostras com base no filtro selecionado (específico ou apenas monitores de ping)
const samples = computed<SamplePoint[]>(() => {
  const targetMonitors =
    selectedMonitorId.value === 'all'
      ? pingMonitors.value
      : monitorsStore.monitors.filter((m) => m.id === selectedMonitorId.value)

  const allResults: Array<{
    monitor: (typeof monitorsStore.monitors)[0]
    latency: number | null
    status: string
    finishedAt: string
  }> = []

  for (const m of targetMonitors) {
    if (m.recentResults && m.recentResults.length > 0) {
      for (const r of m.recentResults) {
        allResults.push({
          monitor: m,
          latency: r.latencyMs,
          status: r.status,
          finishedAt: r.finishedAt,
        })
      }
    } else if (typeof m.lastLatencyMs === 'number') {
      allResults.push({
        monitor: m,
        latency: m.lastLatencyMs,
        status: m.status === 'offline' || m.status === 'down' ? 'down' : 'up',
        finishedAt: m.lastCheckedAt || new Date().toISOString(),
      })
    }
  }

  if (allResults.length === 0) {
    // Retorna amostras sintéticas demonstrativas para inicialização visual
    const now = new Date()
    const list: SamplePoint[] = []
    const pointsCount = timeframe.value === '5m' ? 10 : timeframe.value === '15m' ? 15 : 20

    const mockMonitors: MonitorDetailItem[] =
      targetMonitors.length > 0
        ? targetMonitors.map((m) => ({
            id: m.id,
            name: m.name,
            target: m.target,
            type: m.type,
            deviceName: m.device?.name,
            status: m.status || 'up',
            latencyMs: m.lastLatencyMs ?? 15,
            lossPct: m.status === 'down' || m.status === 'offline' ? 100 : 0,
          }))
        : [
            {
              id: 1,
              name: 'Gateway Principal',
              target: '192.168.1.1',
              type: 'ping',
              status: 'up',
              latencyMs: 12,
              lossPct: 0,
            },
          ]

    for (let i = pointsCount - 1; i >= 0; i--) {
      const t = new Date(now.getTime() - i * 60 * 1000)
      const lossVal = i === 3 ? 5 : 0
      list.push({
        time: t.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' }),
        latency: Math.max(5, Math.floor(12 + Math.random() * 8)),
        loss: lossVal,
        monitorsDetail: mockMonitors.map((item) => ({
          ...item,
          lossPct: lossVal > 0 ? lossVal : item.lossPct,
        })),
      })
    }
    return list
  }

  // Agrupa em baldes de tempo
  const sorted = [...allResults].sort(
    (a, b) => new Date(a.finishedAt).getTime() - new Date(b.finishedAt).getTime()
  )
  const sampleMap = new Map<
    string,
    {
      latencies: number[]
      downCount: number
      total: number
      items: MonitorDetailItem[]
    }
  >()

  for (const item of sorted) {
    const d = new Date(item.finishedAt)
    const key = d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
    if (!sampleMap.has(key)) {
      sampleMap.set(key, { latencies: [], downCount: 0, total: 0, items: [] })
    }
    const entry = sampleMap.get(key)!
    entry.total++
    const isDown = item.status === 'down' || item.status === 'offline'
    if (isDown) {
      entry.downCount++
    }
    if (item.latency !== null && item.latency !== undefined) {
      entry.latencies.push(item.latency)
    }

    if (!entry.items.some((i) => i.id === item.monitor.id)) {
      entry.items.push({
        id: item.monitor.id,
        name: item.monitor.name,
        target: item.monitor.target,
        type: item.monitor.type,
        deviceName: item.monitor.device?.name,
        status: item.status,
        latencyMs: item.latency,
        lossPct: isDown ? 100 : 0,
      })
    }
  }

  const result: SamplePoint[] = []
  sampleMap.forEach((val, key) => {
    const avgLat =
      val.latencies.length > 0 ? val.latencies.reduce((a, b) => a + b, 0) / val.latencies.length : 0
    const lossPct = val.total > 0 ? Math.round((val.downCount / val.total) * 100) : 0
    result.push({
      time: key,
      latency: Number(avgLat.toFixed(1)),
      loss: lossPct,
      monitorsDetail: val.items,
    })
  })

  return result.slice(-25)
})

const maxLatency = computed(() => {
  if (samples.value.length === 0) return 100
  const max = Math.max(...samples.value.map((s) => s.latency))
  return max > 0 ? Math.ceil(max * 1.2) : 50
})

const avgLatency = computed(() => {
  if (samples.value.length === 0) return 0
  const sum = samples.value.reduce((acc, s) => acc + s.latency, 0)
  return Math.round(sum / samples.value.length)
})

const maxLatencyFormatted = computed(() => `${maxLatency.value} ms`)
const midLatencyFormatted = computed(() => `${Math.round(maxLatency.value / 2)} ms`)

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

function goToMonitor(id: number) {
  detailDialog.value = false
  router.push({ name: 'monitor-detail', params: { id } })
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
