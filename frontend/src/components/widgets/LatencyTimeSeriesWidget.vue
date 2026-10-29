<template>
  <DashboardCard
    title="Latência & Perda de Pacotes"
    icon="mdi-chart-timeline-variant"
    icon-color="info"
    :subtitle="sourceDescription"
    :loading="loading"
  >
    <template #actions>
      <v-select
        v-model="selectedMonitorId"
        :items="monitorOptions"
        item-title="name"
        item-value="id"
        density="compact"
        variant="outlined"
        hide-details
        aria-label="Monitor"
        class="latency-widget__select"
      ></v-select>
      <TimeframeToggle v-model="timeframe" />
    </template>

    <BaseMetricChart
      :series="chartSeries"
      unit-type="latency"
      :height="220"
      :show-avg-line="avgLatency > 0"
      :avg-value="avgLatency"
      clickable
      empty-text="Sem dados de telemetria no período."
      aria-label="Latência e perda de pacotes no período"
      @point-click="openSample"
    />

    <template #footer>
      <div class="text-caption">
        Pontos vermelhos marcam amostras com perda de pacotes · clique no gráfico para ver os
        monitores da amostra.
      </div>
    </template>

    <!-- Diálogo de Detalhes da Amostragem Selecionada -->
    <v-dialog v-model="detailDialog" :fullscreen="$vuetify.display.xs" max-width="700px" scrollable>
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
  </DashboardCard>

  <MonitorDetailDialog v-model="detalheAberto" :monitor-id="monitorEmDetalhe" />
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import MonitorDetailDialog from '@/components/monitors/MonitorDetailDialog.vue'
import { useMonitorDetail } from '@/composables/useMonitorDetail'
import {
  useMonitorsStore,
  type MonitorTimeSeriesPoint,
  type MonitorTimeSeriesDetailItem,
  type MonitorTimeSeriesResponse,
} from '@/stores/monitors'
import { useEventsStore } from '@/stores/events'
import type { WidgetConfig } from '@/stores/dashboard'
import { formatLatency, formatPercent } from '@/utils/formatters'
import { CHART_STATUS_COLORS, paletteColor } from '@/utils/charts/scale'
import BaseMetricChart, { type ChartSeriesInput } from '@/components/BaseMetricChart.vue'
import DashboardCard from '@/components/dashboard/DashboardCard.vue'
import TimeframeToggle from '@/components/dashboard/TimeframeToggle.vue'
import { useWidgetTimeframe } from '@/composables/useWidgetTimeframe'

const props = defineProps<{ widget?: WidgetConfig }>()

const timeframe = useWidgetTimeframe(() => props.widget, '15m')
const configuredMonitor = props.widget?.config?.monitorId
const selectedMonitorId = ref<number | 'all'>(
  typeof configuredMonitor === 'number' ? configuredMonitor : 'all'
)
const monitorsStore = useMonitorsStore()
const eventsStore = useEventsStore()
const { detalheAberto, monitorEmDetalhe, abrirDetalhe } = useMonitorDetail()

const loading = ref(false)
const timeSeriesResponse = ref<MonitorTimeSeriesResponse | null>(null)
const localSamples = ref<MonitorTimeSeriesPoint[]>([])

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

const avgLatency = computed(() => {
  if (timeSeriesResponse.value && timeSeriesResponse.value.avgLatency > 0) {
    return timeSeriesResponse.value.avgLatency
  }
  const valid = samples.value.filter((s) => s.latency > 0)
  if (valid.length === 0) return 0
  const sum = valid.reduce((acc, s) => acc + s.latency, 0)
  return Math.round(sum / valid.length)
})

/** Uma série só; a amostra com perda ganha marcador vermelho e o valor no tooltip. */
const chartSeries = computed<ChartSeriesInput[]>(() => [
  {
    id: 'latency',
    label: 'Latência',
    color: paletteColor(0),
    fillArea: true,
    data: samples.value.map((sample) => ({
      time: new Date(sample.timestamp).toISOString(),
      value: sample.latency,
      formattedValue:
        sample.loss > 0
          ? `${formatLatency(sample.latency)} · perda ${formatPercent(sample.loss, 0)}`
          : formatLatency(sample.latency),
      color: sample.loss > 0 ? CHART_STATUS_COLORS.critical : undefined,
    })),
  },
])

function openSample(index: number) {
  const sample = samples.value[index]
  if (!sample) return
  selectedSample.value = sample
  detailDialog.value = true
}

/**
 * O monitor abre no **mesmo** diálogo das demais listas, e não em outra tela.
 * Ver `useMonitorDetail`: a regra de "como se abre um monitor" é uma só.
 */
function abrirMonitor(id: number) {
  detailDialog.value = false
  abrirDetalhe(id)
}
</script>

<style scoped>
.latency-widget__select {
  min-width: 200px;
  max-width: 260px;
}
</style>
