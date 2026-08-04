<template>
  <div>
    <!-- Botão de Voltar -->
    <v-btn variant="text" prepend-icon="mdi-arrow-left" class="mb-4" to="/monitors">
      Voltar para Monitores
    </v-btn>

    <!-- Loading State -->
    <v-card
      v-if="monitorsStore.loading && !monitorsStore.currentMonitor"
      elevation="2"
      class="pa-8 text-center rounded-lg"
    >
      <v-progress-circular indeterminate color="primary" size="48"></v-progress-circular>
      <div class="mt-4 text-subtitle-1 text-grey">
        Carregando métricas e histórico do monitor...
      </div>
    </v-card>

    <div v-else-if="monitorsStore.currentMonitor">
      <!-- Header do Monitor -->
      <v-card elevation="2" class="rounded-lg pa-6 mb-6">
        <div class="d-flex align-center justify-space-between flex-wrap ga-4">
          <div class="d-flex align-center ga-4" style="gap: 16px">
            <v-avatar :color="getStatusColor(monitor.status)" size="56" class="text-white mr-2">
              <v-icon size="32">{{ getTypeIcon(monitor.type) }}</v-icon>
            </v-avatar>
            <div>
              <div class="d-flex align-center ga-3 flex-wrap" style="gap: 12px">
                <h1 class="text-h4 font-weight-bold mr-3">{{ monitor.name }}</h1>
                <v-chip
                  :color="getStatusColor(monitor.status)"
                  size="small"
                  variant="flat"
                  class="font-weight-bold px-3"
                >
                  <v-icon start size="14">mdi-circle</v-icon>
                  {{ statusText }}
                </v-chip>
                <v-chip size="small" color="info" variant="tonal" class="px-3">
                  {{ typeText }}
                </v-chip>
              </div>
              <div class="text-subtitle-1 text-grey-darken-1 mt-1">
                Alvo: <strong class="text-high-emphasis">{{ formattedTarget }}</strong> | Intervalo:
                {{ monitor.intervalSeconds }}s | Timeout: {{ monitor.timeoutSeconds }}s
                <span v-if="monitor.device">
                  | Dispositivo: <strong>{{ monitor.device.name }}</strong></span
                >
              </div>
            </div>
          </div>

          <div class="d-flex align-center ga-3" style="gap: 12px">
            <v-btn
              v-if="monitor.device"
              variant="tonal"
              prepend-icon="mdi-router-network"
              :to="{ name: 'device-detail', params: { id: monitor.device.id } }"
            >
              Ver dispositivo
            </v-btn>
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
            <div
              class="text-h4 font-weight-bold my-1"
              :class="stats.lastLatency !== null ? 'text-primary' : 'text-grey'"
            >
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
              <span class="text-subtitle-2 text-grey-darken-1 font-weight-medium"
              >Ping Mín / Máx</span
              >
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
              <span class="text-subtitle-2 text-grey-darken-1 font-weight-medium"
              >Taxa de Uptime</span
              >
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
        <div class="d-flex align-center justify-space-between mb-4 flex-wrap ga-2">
          <div>
            <h2 class="text-h6 font-weight-bold d-flex align-center ga-2">
              <v-icon color="primary">mdi-chart-timeline-variant</v-icon>
              Linha do Tempo de Disponibilidade
            </h2>
            <div class="text-subtitle-2 text-grey">Histórico recente de verificações de status</div>
          </div>
          <div class="d-flex align-center ga-4 text-caption">
            <span class="d-flex align-center ga-1">
              <span class="status-indicator-dot bg-success"></span> UP ({{ stats.upChecks }})
            </span>
            <span class="d-flex align-center ga-1">
              <span class="status-indicator-dot bg-error"></span> DOWN ({{
                stats.totalChecks - stats.upChecks
              }})
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

      <!-- Gráfico Unificado de Latência / Tempo de Resposta -->
      <v-card elevation="2" class="rounded-lg pa-6 mb-6">
        <div class="d-flex align-center justify-space-between mb-4">
          <div>
            <h2 class="text-h6 font-weight-bold d-flex align-center ga-2">
              <v-icon color="info">mdi-sine-wave</v-icon>
              Gráfico de Tempo de Resposta (Ping Latency)
            </h2>
            <div class="text-subtitle-2 text-grey">
              Variação da latência em milissegundos (ms) ao longo do tempo
            </div>
          </div>
          <v-chip v-if="stats.avgLatency" color="info" size="small" variant="outlined">
            Média: {{ stats.avgLatency }} ms
          </v-chip>
        </div>

        <!-- Renderização do Gráfico Unificado BaseMetricChart -->
        <BaseMetricChart
          v-if="latencySeries.length > 0 && latencySeries[0].data.length > 0"
          :series="latencySeries"
          unit-type="latency"
          :show-avg-line="true"
          :avg-value="stats.avgLatency || undefined"
        />

        <div v-else class="text-center text-grey py-8 border rounded-lg bg-grey-lighten-5">
          <v-icon size="40" color="grey-lighten-1">mdi-chart-line-variant</v-icon>
          <div class="mt-2 text-subtitle-2">
            Histórico insuficiente para gerar o gráfico de latência.
          </div>
          <div class="text-caption">Execute mais verificações clicando em "Testar Agora".</div>
        </div>
      </v-card>

      <!-- Tabela com Histórico de Verificações Recentes -->
      <v-card elevation="2" class="rounded-lg pa-6">
        <div class="d-flex align-center justify-space-between mb-4">
          <div>
            <h2 class="text-h6 font-weight-bold d-flex align-center ga-2">
              <v-icon color="primary">mdi-history</v-icon>
              Histórico de Execuções Recentes
            </h2>
            <div class="text-subtitle-2 text-grey">Resultados das últimas verificações</div>
          </div>
          <v-btn
            size="small"
            variant="text"
            prepend-icon="mdi-refresh"
            :loading="monitorsStore.loading"
            @click="refreshData"
          >
            Atualizar
          </v-btn>
        </div>

        <v-data-table
          :headers="historyHeaders"
          :items="formattedHistory"
          density="comfortable"
          hover
          class="rounded-lg border"
        >
          <template #item.status="{ item }">
            <v-chip :color="getStatusColor(item.status)" size="x-small" variant="flat">
              {{ item.status ? item.status.toUpperCase() : 'UNKNOWN' }}
            </v-chip>
          </template>

          <template #item.latencyMs="{ item }">
            <span
              v-if="item.latencyMs !== undefined && item.latencyMs !== null"
              class="font-weight-medium"
            >
              {{ item.latencyMs }} ms
            </span>
            <span v-else class="text-grey">-</span>
          </template>

          <template #item.durationMs="{ item }">
            <span class="text-grey">{{ item.durationMs }} ms</span>
          </template>

          <template #item.finishedAt="{ item }">
            <span>{{ formatDate(item.finishedAt) }}</span>
          </template>

          <template #item.message="{ item }">
            <span :class="item.status === 'down' ? 'text-error font-weight-medium' : 'text-body-2'">
              {{ item.message || '-' }}
            </span>
          </template>
        </v-data-table>
      </v-card>
    </div>

    <!-- State de Erro / Não Encontrado -->
    <v-card v-else elevation="2" class="pa-8 text-center rounded-lg">
      <v-icon size="64" color="error" class="mb-4">mdi-alert-circle-outline</v-icon>
      <div class="text-h6 text-error">Monitor não encontrado</div>
      <div class="text-body-2 text-grey mt-1">O monitor solicitado não existe ou foi removido.</div>
      <v-btn color="primary" class="mt-4" to="/monitors">Voltar para Monitores</v-btn>
    </v-card>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useMonitorsStore } from '@/stores/monitors'
import MonitorTimelineBar from '@/components/MonitorTimelineBar.vue'
import BaseMetricChart, { type ChartSeriesInput } from '@/components/BaseMetricChart.vue'

const route = useRoute()
const router = useRouter()
const monitorsStore = useMonitorsStore()

const monitorId = computed(() => Number(route.params.id))

const monitor = computed(
  () =>
    monitorsStore.currentMonitor || {
      id: 0,
      deviceId: 0,
      name: '',
      type: 'ping',
      target: '',
      port: undefined as number | undefined,
      intervalSeconds: 60,
      timeoutSeconds: 5,
      status: 'unknown',
      isEnabled: true,
      device: undefined,
      recentResults: [],
      stats: undefined,
    }
)

const stats = computed(
  () =>
    monitor.value.stats || {
      avgLatency: null,
      minLatency: null,
      maxLatency: null,
      lastLatency: null,
      uptimePercentage: 100,
      totalChecks: 0,
      upChecks: 0,
    }
)

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

// Estrutura unificada de dados para o componente BaseMetricChart
const latencySeries = computed<ChartSeriesInput[]>(() => {
  const results = (monitor.value.recentResults || []).slice().reverse()
  if (results.length === 0) return []

  return [
    {
      id: 'latency',
      label: 'Tempo de Resposta',
      color: '#2196F3',
      fillArea: true,
      data: results.map((r) => {
        const val = r.latencyMs || 0
        const status = r.status || (val > 0 ? 'up' : 'down')
        return {
          time: formatDate(r.finishedAt),
          value: val,
          formattedValue: `${val} ms`,
          status,
          color: status === 'down' ? '#F44336' : '#2196F3',
        }
      }),
    },
  ]
})
</script>

<style scoped>
.status-indicator-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  display: inline-block;
}
</style>
