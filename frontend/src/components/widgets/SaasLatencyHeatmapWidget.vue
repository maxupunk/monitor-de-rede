<template>
  <v-card elevation="2" class="rounded-lg fill-height d-flex flex-column">
    <!-- Cabeçalho do Card -->
    <v-card-title
      class="d-flex align-center justify-space-between py-3 px-4 flex-wrap ga-2 border-b"
    >
      <div class="d-flex align-center ga-3">
        <v-avatar color="primary" variant="tonal" size="38" rounded="lg">
          <v-icon color="primary" size="22">mdi-chart-scatter-plot-hexbin</v-icon>
        </v-avatar>
        <div>
          <div class="text-subtitle-1 font-weight-bold">Mapa de Calor de Latência</div>
          <div class="text-caption text-medium-emphasis">
            Variação horária (00h-23h) e identificação de períodos de lentidão na rede
          </div>
        </div>
      </div>

      <!-- Filtros e Seletores -->
      <div class="d-flex align-center ga-2 flex-wrap">
        <v-select
          v-model="selectedScope"
          :items="scopeOptions"
          item-title="title"
          item-value="value"
          density="compact"
          variant="outlined"
          hide-details
          style="min-width: 190px; max-width: 260px"
          class="text-caption"
        ></v-select>

        <v-btn-toggle
          v-model="timeframeDays"
          density="compact"
          variant="outlined"
          divided
          mandatory
        >
          <v-btn :value="1" size="x-small">24h</v-btn>
          <v-btn :value="7" size="x-small">7d</v-btn>
          <v-btn :value="14" size="x-small">14d</v-btn>
          <v-btn :value="30" size="x-small">30d</v-btn>
        </v-btn-toggle>

        <v-btn icon="mdi-refresh" variant="text" size="small" :loading="loading" @click="loadData">
          <v-tooltip activator="parent" location="bottom">Atualizar mapa de calor</v-tooltip>
        </v-btn>
      </div>
    </v-card-title>

    <!-- Indicador de Carregamento -->
    <div v-if="loading && !heatmapData" class="d-flex align-center justify-center flex-grow-1 pa-8">
      <v-progress-circular indeterminate color="primary" size="42"></v-progress-circular>
      <span class="text-caption text-medium-emphasis ml-3">Calculando matriz horária...</span>
    </div>

    <!-- Estado Vazio / Sem Monitores -->
    <div
      v-else-if="
        !loading &&
        (!heatmapData || heatmapData.monitors.length === 0 || heatmapData.totalChecks === 0)
      "
      class="d-flex flex-column align-center justify-center flex-grow-1 pa-8 text-center"
    >
      <v-avatar color="info" variant="tonal" size="56" class="mb-3">
        <v-icon size="32">mdi-cloud-search-outline</v-icon>
      </v-avatar>
      <div class="text-subtitle-1 font-weight-bold">Nenhum dado de latência no período</div>
      <div class="text-caption text-medium-emphasis mb-4" style="max-width: 420px">
        Provisione alvos de monitoramento SaaS (Google, Cloudflare, Microsoft, etc.) para mapear a
        qualidade da sua conexão.
      </div>
      <v-btn
        color="primary"
        prepend-icon="mdi-plus-box-multiple"
        size="small"
        @click="saasCatalogDialog = true"
      >
        Abrir Catálogo SaaS
      </v-btn>
    </div>

    <!-- Conteúdo Principal do Heatmap -->
    <v-card-text v-else class="pa-4 flex-grow-1 d-flex flex-column ga-4">
      <!-- Destaques e KPIs de Análise -->
      <v-row dense>
        <v-col cols="12" sm="4">
          <v-card variant="tonal" color="warning" class="pa-3 rounded-lg h-100">
            <div class="d-flex align-center justify-space-between">
              <div class="text-caption font-weight-medium">Horário de Maior Latência</div>
              <v-icon size="18">mdi-clock-alert-outline</v-icon>
            </div>
            <div class="text-h6 font-weight-bold mt-1">
              {{
                heatmapData?.peakHour !== null && heatmapData?.peakHour !== undefined
                  ? `${String(heatmapData.peakHour).padStart(2, '0')}:00h`
                  : 'Estável'
              }}
            </div>
            <div class="text-caption text-medium-emphasis">
              Pico médio de latência no período analisado
            </div>
          </v-card>
        </v-col>

        <v-col cols="12" sm="4">
          <v-card variant="tonal" color="success" class="pa-3 rounded-lg h-100">
            <div class="d-flex align-center justify-space-between">
              <div class="text-caption font-weight-medium">Melhor Horário (Mais Rápido)</div>
              <v-icon size="18">mdi-lightning-bolt-outline</v-icon>
            </div>
            <div class="text-h6 font-weight-bold mt-1">
              {{
                heatmapData?.bestHour !== null && heatmapData?.bestHour !== undefined
                  ? `${String(heatmapData.bestHour).padStart(2, '0')}:00h`
                  : 'Estável'
              }}
            </div>
            <div class="text-caption text-medium-emphasis">Menor latência média observada</div>
          </v-card>
        </v-col>

        <v-col cols="12" sm="4">
          <v-card variant="tonal" color="primary" class="pa-3 rounded-lg h-100">
            <div class="d-flex align-center justify-space-between">
              <div class="text-caption font-weight-medium">Latência Global / Uptime</div>
              <v-icon size="18">mdi-pulse</v-icon>
            </div>
            <div class="text-h6 font-weight-bold mt-1">
              {{
                heatmapData?.overallAvgLatencyMs
                  ? `${heatmapData.overallAvgLatencyMs.toFixed(1)} ms`
                  : '--'
              }}
              <span class="text-caption font-weight-regular text-medium-emphasis">
                · {{ heatmapData?.overallUptimePercentage.toFixed(1) }}% up
              </span>
            </div>
            <div class="text-caption text-medium-emphasis">
              {{ heatmapData?.totalChecks.toLocaleString('pt-BR') }} checagens consolidadas
            </div>
          </v-card>
        </v-col>
      </v-row>

      <!-- Grade do Mapa de Calor (Heatmap Grid) -->
      <div class="heatmap-wrapper border rounded-lg pa-3 bg-surface overflow-x-auto">
        <!-- Cabeçalho das Horas (00h às 23h) -->
        <div class="heatmap-grid-header mb-1">
          <div class="row-label-header">Data</div>
          <div
            v-for="h in 24"
            :key="h - 1"
            class="hour-col-header text-caption text-medium-emphasis"
            :title="`Hora ${h - 1}:00`"
          >
            {{ (h - 1) % 3 === 0 ? `${h - 1}h` : '' }}
          </div>
        </div>

        <!-- Linhas por Dia -->
        <div v-for="dayRow in groupedByDay" :key="dayRow.date" class="heatmap-grid-row">
          <!-- Rótulo do Dia -->
          <div class="row-label text-caption font-weight-medium text-truncate" :title="dayRow.date">
            {{ formatDayLabel(dayRow.date, dayRow.dayOfWeek) }}
          </div>

          <!-- Células das 24 Horas -->
          <div
            v-for="cell in dayRow.cells"
            :key="cell.hour"
            class="heatmap-cell rounded-sm"
            :style="{ backgroundColor: getCellColor(cell) }"
          >
            <v-tooltip activator="parent" location="top" density="compact">
              <div class="pa-1 text-caption">
                <div class="font-weight-bold mb-1">
                  {{ formatDateFull(cell.date) }} · {{ String(cell.hour).padStart(2, '0') }}:00 -
                  {{ String(cell.hour).padStart(2, '0') }}:59
                </div>
                <div v-if="cell.totalChecks > 0">
                  <div>
                    <strong>Latência Média:</strong>
                    {{ cell.avgLatencyMs ? `${cell.avgLatencyMs.toFixed(1)} ms` : '--' }}
                  </div>
                  <div v-if="cell.minLatencyMs !== null && cell.maxLatencyMs !== null">
                    <strong>Variação:</strong> {{ cell.minLatencyMs.toFixed(1) }}ms ~
                    {{ cell.maxLatencyMs.toFixed(1) }}ms
                  </div>
                  <div>
                    <strong>Disponibilidade:</strong> {{ cell.uptimePercentage.toFixed(1) }}%
                  </div>
                  <div>
                    <strong>Checagens:</strong> {{ cell.totalChecks }} ({{ cell.upChecks }} up,
                    {{ cell.downChecks }} down)
                  </div>
                </div>
                <div v-else class="text-grey-lighten-1">
                  Nenhuma medição registrada neste intervalo
                </div>
              </div>
            </v-tooltip>
          </div>
        </div>
      </div>

      <!-- Resumo Consolidado do Perfil Diário (24 Horas) -->
      <div class="mt-2">
        <div class="d-flex align-center justify-space-between mb-2">
          <div class="text-caption font-weight-bold text-medium-emphasis d-flex align-center ga-1">
            <v-icon size="14">mdi-chart-line</v-icon>
            Perfil Médio Consolidado por Hora do Dia (00h - 23h)
          </div>
          <!-- Legenda de Cores -->
          <div class="d-flex align-center ga-2 text-caption">
            <span class="legend-dot" style="background-color: #10b981"></span> &lt;30ms
            <span class="legend-dot" style="background-color: #84cc16"></span> &lt;60ms
            <span class="legend-dot" style="background-color: #f59e0b"></span> &lt;100ms
            <span class="legend-dot" style="background-color: #f97316"></span> &lt;200ms
            <span class="legend-dot" style="background-color: #ef4444"></span> &gt;200ms
          </div>
        </div>

        <div class="hourly-summary-bar d-flex ga-1">
          <div
            v-for="h in heatmapData?.byHourOfDay || []"
            :key="h.hour"
            class="hourly-summary-pill flex-grow-1 text-center rounded-sm pa-1"
            :style="{ backgroundColor: getHourSummaryColor(h) }"
          >
            <div class="text-overline" style="font-size: 8px; line-height: 10px">{{ h.hour }}h</div>
            <div
              class="text-caption font-weight-bold text-truncate"
              style="font-size: 10px; line-height: 12px"
            >
              {{ h.avgLatencyMs ? `${Math.round(h.avgLatencyMs)}` : '-' }}
            </div>
            <v-tooltip activator="parent" location="bottom" density="compact">
              <div class="pa-1 text-caption">
                <div class="font-weight-bold">{{ String(h.hour).padStart(2, '0') }}:00h</div>
                <div>
                  Latência Média: {{ h.avgLatencyMs ? `${h.avgLatencyMs.toFixed(1)} ms` : '--' }}
                </div>
                <div>Disponibilidade: {{ h.uptimePercentage.toFixed(1) }}%</div>
                <div>Total de Amostras: {{ h.totalChecks }}</div>
              </div>
            </v-tooltip>
          </div>
        </div>
      </div>
    </v-card-text>

    <!-- Dialog de Catálogo SaaS -->
    <SaasPresetsDialog v-model="saasCatalogDialog" @provisioned="loadData"></SaasPresetsDialog>
  </v-card>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue'
import { useMonitorsStore } from '@/stores/monitors'
import type { HourlyHeatmapResponse } from '@/bindings/HourlyHeatmapResponse'
import type { HourlyHeatmapCell } from '@/bindings/HourlyHeatmapCell'
import type { HourOfDaySummary } from '@/bindings/HourOfDaySummary'
import SaasPresetsDialog from '@/components/monitors/SaasPresetsDialog.vue'

const props = defineProps<{
  monitorId?: number
}>()

const monitorsStore = useMonitorsStore()

const loading = ref(false)
const timeframeDays = ref(7)
const selectedScope = ref<string>('saas')
const heatmapData = ref<HourlyHeatmapResponse | null>(null)
const saasCatalogDialog = ref(false)

const scopeOptions = computed(() => {
  const list = [
    { title: 'Todos os Alvos SaaS', value: 'saas' },
    { title: 'Todos os Monitores', value: 'all' },
  ]
  if (heatmapData.value?.monitors) {
    for (const m of heatmapData.value.monitors) {
      list.push({
        title: `${m.name} (${m.target})`,
        value: `mon:${m.id}`,
      })
    }
  }
  return list
})

async function loadData() {
  loading.value = true
  try {
    let monId: number | undefined = props.monitorId
    let isSaas: boolean | undefined = true

    if (props.monitorId) {
      monId = props.monitorId
      isSaas = undefined
    } else if (selectedScope.value === 'all') {
      monId = undefined
      isSaas = undefined
    } else if (selectedScope.value.startsWith('mon:')) {
      monId = Number(selectedScope.value.replace('mon:', ''))
      isSaas = undefined
    } else {
      monId = undefined
      isSaas = true
    }

    const res = await monitorsStore.fetchHourlyHeatmap({
      monitorId: monId,
      isSaas,
      days: timeframeDays.value,
    })
    heatmapData.value = res
  } catch {
    heatmapData.value = null
  } finally {
    loading.value = false
  }
}

watch([timeframeDays, selectedScope, () => props.monitorId], () => {
  loadData()
})

onMounted(() => {
  loadData()
})

const groupedByDay = computed(() => {
  if (!heatmapData.value || heatmapData.value.matrix.length === 0) return []

  const map = new Map<string, { date: string; dayOfWeek: number; cells: HourlyHeatmapCell[] }>()

  for (const cell of heatmapData.value.matrix) {
    if (!map.has(cell.date)) {
      map.set(cell.date, {
        date: cell.date,
        dayOfWeek: cell.dayOfWeek,
        cells: [],
      })
    }
    map.get(cell.date)!.cells.push(cell)
  }

  // Ordena por hora garantindo 0..23
  for (const day of map.values()) {
    day.cells.sort((a, b) => a.hour - b.hour)
  }

  return Array.from(map.values())
})

function getCellColor(cell: HourlyHeatmapCell): string {
  if (cell.totalChecks === 0 || cell.avgLatencyMs === null) {
    return 'rgba(148, 163, 184, 0.12)'
  }

  if (cell.uptimePercentage < 80.0) {
    return '#ef4444' // Vermelho de indisponibilidade
  }

  const lat = cell.avgLatencyMs
  if (lat < 30.0) return '#10b981' // Verde esmeralda
  if (lat < 60.0) return '#84cc16' // Verde claro
  if (lat < 100.0) return '#f59e0b' // Amarelo/Âmbar
  if (lat < 200.0) return '#f97316' // Laranja
  return '#ef4444' // Vermelho
}

function getHourSummaryColor(h: HourOfDaySummary): string {
  if (h.totalChecks === 0 || h.avgLatencyMs === null) {
    return 'rgba(148, 163, 184, 0.12)'
  }
  const lat = h.avgLatencyMs
  if (lat < 30.0) return 'rgba(16, 185, 129, 0.35)'
  if (lat < 60.0) return 'rgba(132, 204, 22, 0.35)'
  if (lat < 100.0) return 'rgba(245, 158, 11, 0.35)'
  if (lat < 200.0) return 'rgba(249, 115, 22, 0.35)'
  return 'rgba(239, 68, 68, 0.35)'
}

function formatDayLabel(dateStr: string, dayOfWeek: number): string {
  const days = ['Dom', 'Seg', 'Ter', 'Qua', 'Qui', 'Sex', 'Sáb']
  const parts = dateStr.split('-')
  const formattedDate = parts.length === 3 ? `${parts[2]}/${parts[1]}` : dateStr
  return `${formattedDate} (${days[dayOfWeek] ?? ''})`
}

function formatDateFull(dateStr: string): string {
  const parts = dateStr.split('-')
  if (parts.length === 3) {
    return `${parts[2]}/${parts[1]}/${parts[0]}`
  }
  return dateStr
}
</script>

<style scoped>
.heatmap-wrapper {
  min-width: 600px;
}

.heatmap-grid-header {
  display: grid;
  grid-template-columns: 85px repeat(24, minmax(18px, 1fr));
  gap: 3px;
  align-items: center;
}

.row-label-header {
  font-size: 11px;
  font-weight: 600;
  color: rgba(var(--v-theme-on-surface), 0.6);
}

.hour-col-header {
  text-align: center;
  font-size: 10px;
  height: 16px;
  line-height: 16px;
}

.heatmap-grid-row {
  display: grid;
  grid-template-columns: 85px repeat(24, minmax(18px, 1fr));
  gap: 3px;
  align-items: center;
  margin-bottom: 3px;
}

.row-label {
  font-size: 11px;
  color: rgba(var(--v-theme-on-surface), 0.8);
}

.heatmap-cell {
  height: 22px;
  cursor: pointer;
  transition:
    transform 0.15s ease,
    filter 0.15s ease;
}

.heatmap-cell:hover {
  transform: scale(1.18);
  filter: brightness(1.2);
  z-index: 2;
}

.legend-dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 2px;
  margin-right: 2px;
}

.hourly-summary-pill {
  border: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
}
</style>
