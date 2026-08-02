<template>
  <div>
    <!-- Botão de Voltar -->
    <v-btn variant="text" prepend-icon="mdi-arrow-left" class="mb-4" to="/monitors">
      Voltar para Monitores
    </v-btn>

    <!-- Loading State -->
    <v-card v-if="monitorsStore.loading && !monitorsStore.currentMonitor" elevation="2" class="pa-8 text-center rounded-lg">
      <v-progress-circular indeterminate color="primary" size="48"></v-progress-circular>
      <div class="mt-4 text-subtitle-1 text-grey">Carregando métricas e histórico do monitor...</div>
    </v-card>

    <div v-else-if="monitorsStore.currentMonitor">
      <!-- Header do Monitor -->
      <v-card elevation="2" class="rounded-lg pa-6 mb-6">
        <div class="d-flex align-center justify-space-between flex-wrap gap-4">
          <div class="d-flex align-center ga-4" style="gap: 16px;">
            <v-avatar :color="getStatusColor(monitor.status)" size="56" class="text-white mr-2">
              <v-icon size="32">{{ getTypeIcon(monitor.type) }}</v-icon>
            </v-avatar>
            <div>
              <div class="d-flex align-center ga-3 flex-wrap" style="gap: 12px;">
                <h1 class="text-h4 font-weight-bold mr-3">{{ monitor.name }}</h1>
                <v-chip :color="getStatusColor(monitor.status)" size="small" variant="flat" class="font-weight-bold px-3">
                  <v-icon start size="14">mdi-circle</v-icon>
                  {{ statusText }}
                </v-chip>
                <v-chip size="small" color="info" variant="tonal" class="px-3">
                  {{ typeText }}
                </v-chip>
              </div>
              <div class="text-subtitle-1 text-grey-darken-1 mt-1">
                Alvo: <strong class="text-high-emphasis">{{ formattedTarget }}</strong> |
                Intervalo: {{ monitor.intervalSeconds }}s |
                Timeout: {{ monitor.timeoutSeconds }}s
                <span v-if="monitor.device"> | Dispositivo: <strong>{{ monitor.device.name }}</strong></span>
              </div>
            </div>
          </div>

          <div class="d-flex align-center ga-3" style="gap: 12px;">
            <v-btn
              color="primary"
              prepend-icon="mdi-play"
              :loading="monitorsStore.runningId === monitor.id"
              @click="monitorsStore.runMonitor(monitor.id)"
            >
              Testar Agora
            </v-btn>
            <v-btn
              :color="monitor.isEnabled ? 'warning' : 'success'"
              variant="outlined"
              :prepend-icon="monitor.isEnabled ? 'mdi-pause' : 'mdi-play-outline'"
              @click="monitorsStore.toggleMonitorEnabled(monitor.id, !monitor.isEnabled)"
            >
              {{ monitor.isEnabled ? 'Pausar' : 'Ativar' }}
            </v-btn>
            <v-btn icon color="error" variant="text" @click="confirmDelete">
              <v-icon>mdi-delete</v-icon>
              <v-tooltip activator="parent" location="top">Excluir Monitor</v-tooltip>
            </v-btn>
          </div>
        </div>
      </v-card>

      <!-- Cards de Métricas KPI (Ping Médio, Ping Atual, Ping Mín/Máx, Uptime %) -->
      <v-row class="mb-6">
        <!-- Ping Atual -->
        <v-col cols="12" sm="6" md="3">
          <v-card elevation="2" class="rounded-lg pa-4 h-100">
            <div class="d-flex align-center justify-space-between mb-2">
              <span class="text-subtitle-2 text-grey-darken-1 font-weight-medium">Ping Atual</span>
              <v-avatar color="primary" variant="tonal" size="36">
                <v-icon size="20">mdi-speedometer</v-icon>
              </v-avatar>
            </div>
            <div class="text-h4 font-weight-bold my-1" :class="stats.lastLatency !== null ? 'text-primary' : 'text-grey'">
              {{ lastLatencyText }}
            </div>
            <div class="text-caption text-grey">Última resposta registrada</div>
          </v-card>
        </v-col>

        <!-- Ping Médio -->
        <v-col cols="12" sm="6" md="3">
          <v-card elevation="2" class="rounded-lg pa-4 h-100">
            <div class="d-flex align-center justify-space-between mb-2">
              <span class="text-subtitle-2 text-grey-darken-1 font-weight-medium">Ping Médio</span>
              <v-avatar color="info" variant="tonal" size="36">
                <v-icon size="20">mdi-chart-line</v-icon>
              </v-avatar>
            </div>
            <div class="text-h4 font-weight-bold my-1 text-info">
              {{ avgLatencyText }}
            </div>
            <div class="text-caption text-grey">Média das verificações recentes</div>
          </v-card>
        </v-col>

        <!-- Ping Mínimo / Máximo -->
        <v-col cols="12" sm="6" md="3">
          <v-card elevation="2" class="rounded-lg pa-4 h-100">
            <div class="d-flex align-center justify-space-between mb-2">
              <span class="text-subtitle-2 text-grey-darken-1 font-weight-medium">Ping Mín / Máx</span>
              <v-avatar color="purple" variant="tonal" size="36">
                <v-icon size="20">mdi-swap-vertical</v-icon>
              </v-avatar>
            </div>
            <div class="text-h5 font-weight-bold my-1 text-purple">
              <span>{{ minLatencyText }}</span>
              <span class="text-grey-darken-1 font-weight-regular text-subtitle-1 mx-1">/</span>
              <span>{{ maxLatencyText }}</span>
            </div>
            <div class="text-caption text-grey">Mínima e máxima de latência</div>
          </v-card>
        </v-col>

        <!-- Uptime / Taxa de Sucesso -->
        <v-col cols="12" sm="6" md="3">
          <v-card elevation="2" class="rounded-lg pa-4 h-100">
            <div class="d-flex align-center justify-space-between mb-2">
              <span class="text-subtitle-2 text-grey-darken-1 font-weight-medium">Taxa de Uptime</span>
              <v-avatar color="success" variant="tonal" size="36">
                <v-icon size="20">mdi-check-decagram</v-icon>
              </v-avatar>
            </div>
            <div class="text-h4 font-weight-bold my-1 text-success">
              {{ stats.uptimePercentage }}%
            </div>
            <v-progress-linear
              :model-value="stats.uptimePercentage"
              color="success"
              height="6"
              rounded
              class="mt-2"
            ></v-progress-linear>
          </v-card>
        </v-col>
      </v-row>

      <!-- Linha do Tempo de Uptime (Bar Timeline - Estilo Uptime Kuma) -->
      <v-card elevation="2" class="rounded-lg pa-6 mb-6">
        <div class="d-flex align-center justify-space-between mb-4 flex-wrap gap-2">
          <div>
            <h2 class="text-h6 font-weight-bold d-flex align-center gap-2">
              <v-icon color="primary">mdi-chart-timeline-variant</v-icon>
              Linha do Tempo de Disponibilidade
            </h2>
            <div class="text-subtitle-2 text-grey">Histórico recente de verificações de status</div>
          </div>
          <div class="d-flex align-center gap-4 text-caption">
            <span class="d-flex align-center gap-1">
              <span class="status-indicator-dot bg-success"></span> UP ({{ stats.upChecks }})
            </span>
            <span class="d-flex align-center gap-1">
              <span class="status-indicator-dot bg-error"></span> DOWN ({{ stats.totalChecks - stats.upChecks }})
            </span>
            <span class="text-grey font-weight-bold">Total: {{ stats.totalChecks }}</span>
          </div>
        </div>

        <div class="pa-3 bg-grey-lighten-4 rounded-lg overflow-x-auto d-flex justify-center">
          <MonitorTimelineBar
            :results="monitor.recentResults"
            :max-blocks="60"
            :height="36"
            :width="10"
          />
        </div>
      </v-card>

      <!-- Gráfico de Latência / Tempo de Resposta (Ping Line Chart) -->
      <v-card elevation="2" class="rounded-lg pa-6 mb-6">
        <div class="d-flex align-center justify-space-between mb-4">
          <div>
            <h2 class="text-h6 font-weight-bold d-flex align-center gap-2">
              <v-icon color="info">mdi-sine-wave</v-icon>
              Gráfico de Tempo de Resposta (Ping Latency)
            </h2>
            <div class="text-subtitle-2 text-grey">Variação da latência em milissegundos (ms) ao longo do tempo</div>
          </div>
          <v-chip v-if="stats.avgLatency" color="info" size="small" variant="outlined">
            Média: {{ stats.avgLatency }} ms
          </v-chip>
        </div>

        <!-- Renderização do Gráfico SVG de Latência -->
        <div v-if="latencyDataPoints.length > 1" class="chart-container relative pa-2">
          <svg class="w-100 latency-svg" viewBox="0 0 800 240" preserveAspectRatio="none">
            <defs>
              <linearGradient id="latencyGradient" x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stop-color="#2196F3" stop-opacity="0.4" />
                <stop offset="100%" stop-color="#2196F3" stop-opacity="0.0" />
              </linearGradient>
            </defs>

            <!-- Linhas de Grade de Fundo (Grid Lines) -->
            <line x1="75" y1="30" x2="785" y2="30" stroke="#E0E0E0" stroke-dasharray="3,3" />
            <text x="68" y="34" font-size="11" fill="#9E9E9E" text-anchor="end">{{ chartMaxMs }}ms</text>

            <line x1="75" y1="110" x2="785" y2="110" stroke="#E0E0E0" stroke-dasharray="3,3" />
            <text x="68" y="114" font-size="11" fill="#9E9E9E" text-anchor="end">{{ halfMaxMs }}ms</text>

            <line x1="75" y1="190" x2="785" y2="190" stroke="#E0E0E0" stroke-width="1.5" />
            <text x="68" y="194" font-size="11" fill="#9E9E9E" text-anchor="end">0ms</text>

            <!-- Linha Média Tracejada -->
            <line
              v-if="stats.avgLatency"
              x1="75"
              :y1="getSvgY(stats.avgLatency)"
              x2="785"
              :y2="getSvgY(stats.avgLatency)"
              stroke="#FF9800"
              stroke-width="1.5"
              stroke-dasharray="4,4"
            />

            <!-- Área sob a Curva com Preenchimento Gradiente -->
            <polygon :points="svgAreaPoints" fill="url(#latencyGradient)" />

            <!-- Linha principal do gráfico -->
            <polyline :points="svgPolylinePoints" fill="none" stroke="#2196F3" stroke-width="2.5" stroke-linecap="round" />

            <!-- Pontos (Data Circles com Tooltip) -->
            <g v-for="(pt, idx) in latencyDataPoints" :key="idx">
              <v-tooltip location="top" color="#0F172A">
                <template #activator="{ props }">
                  <circle
                    v-bind="props"
                    :cx="pt.x"
                    :cy="pt.y"
                    r="5"
                    :fill="pt.status === 'up' ? '#2196F3' : '#F44336'"
                    stroke="#FFFFFF"
                    stroke-width="2"
                    class="chart-point"
                  />
                </template>
                <div class="pa-2 text-white" style="font-size: 12px">
                  <div class="font-weight-bold mb-1 d-flex align-center gap-1" style="font-size: 13px; color: #38BDF8">
                    <span class="status-indicator-dot" :style="{ backgroundColor: pt.status === 'up' ? '#4CAF50' : '#F44336' }"></span>
                    Latência: {{ pt.latency }} ms
                  </div>
                  <div style="font-size: 11px; color: #94A3B8">
                    Data: {{ pt.time }}
                  </div>
                  <div style="font-size: 11px; color: #E2E8F0" class="mt-1">
                    Status: {{ pt.status.toUpperCase() }}
                  </div>
                </div>
              </v-tooltip>
            </g>
          </svg>
        </div>

        <div v-else class="text-center text-grey py-8 border rounded-lg bg-grey-lighten-5">
          <v-icon size="40" color="grey-lighten-1">mdi-chart-line-variant</v-icon>
          <div class="mt-2 text-subtitle-2">Histórico insuficiente para gerar o gráfico de latência.</div>
          <div class="text-caption">Execute mais verificações clicando em "Testar Agora".</div>
        </div>
      </v-card>

      <!-- Tabela de Histórico Detalhado de Verificações com Paginação -->
      <v-card elevation="2" class="rounded-lg">
        <v-card-title class="pa-4 d-flex align-center justify-space-between flex-wrap gap-4">
          <div class="d-flex align-center gap-3">
            <span class="font-weight-bold text-h6">Histórico de Execuções</span>
            <v-chip size="small" color="primary" variant="tonal" class="font-weight-bold">
              {{ formattedHistory.length }} registros
            </v-chip>
          </div>

          <div class="d-flex align-center gap-3">
            <v-text-field
              v-model="searchHistory"
              prepend-inner-icon="mdi-magnify"
              label="Buscar mensagem ou status"
              single-line
              hide-details
              variant="outlined"
              density="compact"
              style="width: 240px"
            ></v-text-field>

            <v-btn icon variant="text" size="small" @click="refreshData">
              <v-icon>mdi-refresh</v-icon>
              <v-tooltip activator="parent" location="top">Atualizar Histórico</v-tooltip>
            </v-btn>
          </div>
        </v-card-title>

        <v-divider></v-divider>

        <v-data-table
          :headers="historyHeaders"
          :items="formattedHistory"
          :search="searchHistory"
          :items-per-page="10"
          :items-per-page-options="[10, 25, 50, 100]"
          density="compact"
          no-data-text="Nenhum histórico registrado para este monitor."
        >
          <template #item.status="{ item }">
            <v-chip :color="getStatusColor(item.status)" size="x-small" variant="flat" class="font-weight-bold">
              {{ (item.status || 'UNKNOWN').toUpperCase() }}
            </v-chip>
          </template>

          <template #item.latencyMs="{ item }">
            <span v-if="item.latencyMs !== null" class="font-weight-bold text-primary">{{ item.latencyMs }} ms</span>
            <span v-else class="text-grey-darken-1">N/A</span>
          </template>

          <template #item.durationMs="{ item }">
            {{ item.durationMs }} ms
          </template>

          <template #item.finishedAt="{ item }">
            {{ formatDate(item.finishedAt || item.startedAt) }}
          </template>

          <template #item.message="{ item }">
            <span class="text-caption text-grey-darken-1">{{ item.message || '-' }}</span>
          </template>
        </v-data-table>
      </v-card>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useMonitorsStore, type Monitor } from '@/stores/monitors'
import MonitorTimelineBar from '@/components/MonitorTimelineBar.vue'

const route = useRoute()
const router = useRouter()
const monitorsStore = useMonitorsStore()
const searchHistory = ref('')

const monitorId = computed(() => Number(route.params.id))
const monitor = computed<Monitor>(() => monitorsStore.currentMonitor || {
  id: 0,
  deviceId: 0,
  name: '',
  type: 'ping',
  target: '',
  intervalSeconds: 60,
  timeoutSeconds: 5,
  status: 'unknown',
  isEnabled: true,
  device: undefined,
  recentResults: [],
  stats: undefined,
})

const stats = computed(() => monitor.value.stats || {
  avgLatency: null,
  minLatency: null,
  maxLatency: null,
  lastLatency: null,
  uptimePercentage: 100,
  totalChecks: 0,
  upChecks: 0,
})

const historyHeaders = [
  { title: 'Status', key: 'status', width: '110px' },
  { title: 'Latência (Ping)', key: 'latencyMs', width: '140px' },
  { title: 'Duração', key: 'durationMs', width: '120px' },
  { title: 'Data e Hora', key: 'finishedAt', width: '180px' },
  { title: 'Mensagem', key: 'message' },
]

const formattedHistory = computed(() => {
  return (monitor.value.recentResults || []).slice().reverse()
})

const formattedTarget = computed(() => {
  if (monitor.value.port) {
    return `${monitor.value.target}:${monitor.value.port}`
  }
  return monitor.value.target
})

const statusText = computed(() => (monitor.value.status || 'UNKNOWN').toUpperCase())
const typeText = computed(() => (monitor.value.type || 'PING').toUpperCase())

const lastLatencyText = computed(() => {
  return stats.value.lastLatency !== null ? `${stats.value.lastLatency} ms` : 'N/A'
})
const avgLatencyText = computed(() => {
  return stats.value.avgLatency !== null ? `${stats.value.avgLatency} ms` : 'N/A'
})
const minLatencyText = computed(() => {
  return stats.value.minLatency !== null ? `${stats.value.minLatency}ms` : 'N/A'
})
const maxLatencyText = computed(() => {
  return stats.value.maxLatency !== null ? `${stats.value.maxLatency}ms` : 'N/A'
})

onMounted(async () => {
  if (monitorId.value) {
    await monitorsStore.fetchMonitorById(monitorId.value)
  }
})

async function refreshData() {
  if (monitorId.value) {
    await monitorsStore.fetchMonitorById(monitorId.value)
  }
}

function getStatusColor(status?: string): string {
  switch (status) {
    case 'up':
    case 'online':
      return 'success'
    case 'down':
    case 'offline':
      return 'error'
    case 'warning':
      return 'warning'
    default:
      return 'grey'
  }
}

function getTypeIcon(type?: string): string {
  switch (type) {
    case 'http':
    case 'https':
      return 'mdi-web'
    case 'tcp':
      return 'mdi-ethernet-cable'
    case 'dns':
      return 'mdi-dns'
    default:
      return 'mdi-ping'
  }
}

function formatDate(dateStr?: string): string {
  if (!dateStr) return '-'
  try {
    const d = new Date(dateStr)
    return d.toLocaleString('pt-BR')
  } catch {
    return dateStr
  }
}

async function confirmDelete() {
  if (confirm('Tem certeza de que deseja excluir este monitor?')) {
    const success = await monitorsStore.deleteMonitor(monitorId.value)
    if (success) {
      router.push('/monitors')
    }
  }
}

// CÁLCULO E RENDERIZAÇÃO DO GRÁFICO SVG DE LATÊNCIA
const chartMaxMs = computed(() => {
  const max = stats.value.maxLatency || 100
  return Math.max(max + 20, 50)
})

const halfMaxMs = computed(() => Math.round(chartMaxMs.value / 2))

interface DataPoint {
  x: number
  y: number
  latency: number
  time: string
  status: string
}

const latencyDataPoints = computed<DataPoint[]>(() => {
  const results = monitor.value.recentResults || []
  if (results.length === 0) return []

  const maxMs = chartMaxMs.value
  const paddingLeft = 75
  const paddingRight = 785
  const paddingTop = 30
  const paddingBottom = 190

  const width = paddingRight - paddingLeft
  const height = paddingBottom - paddingTop

  const stepX = results.length > 1 ? width / (results.length - 1) : 0

  return results.map((res, idx) => {
    const latency = res.latencyMs ?? 0
    const ratio = Math.min(latency / maxMs, 1)
    const x = paddingLeft + idx * stepX
    const y = paddingBottom - ratio * height

    return {
      x: Math.round(x * 10) / 10,
      y: Math.round(y * 10) / 10,
      latency,
      time: formatDate(res.finishedAt || res.startedAt),
      status: res.status,
    }
  })
})

function getSvgY(latency: number): number {
  const maxMs = chartMaxMs.value
  const paddingTop = 30
  const paddingBottom = 190
  const height = paddingBottom - paddingTop
  const ratio = Math.min(latency / maxMs, 1)
  return Math.round((paddingBottom - ratio * height) * 10) / 10
}

const svgPolylinePoints = computed(() => {
  return latencyDataPoints.value.map((p) => `${p.x},${p.y}`).join(' ')
})

const svgAreaPoints = computed(() => {
  if (latencyDataPoints.value.length === 0) return ''
  const first = latencyDataPoints.value[0]
  const last = latencyDataPoints.value[latencyDataPoints.value.length - 1]
  const line = svgPolylinePoints.value
  return `${first.x},190 ${line} ${last.x},190`
})
</script>

<style scoped>
.status-indicator-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  display: inline-block;
}

.latency-svg {
  width: 100%;
  height: 240px;
}

.chart-point {
  transition: r 0.2s ease, fill 0.2s ease;
  cursor: pointer;
}

.chart-point:hover {
  r: 7;
}
</style>
