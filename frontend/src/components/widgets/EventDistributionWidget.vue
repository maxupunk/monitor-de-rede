<template>
  <DashboardCard
    title="Distribuição de Eventos por Hora"
    icon="mdi-chart-bar"
    icon-color="secondary"
    subtitle="Últimas 6 horas, por severidade"
    :loading="loading"
  >
    <template #actions>
      <v-chip color="secondary" size="small" variant="tonal" class="font-weight-bold">
        Total: {{ totals.total }}
      </v-chip>
    </template>

    <StackedBarChart
      v-if="hourlyBins.length > 0"
      :categories="categories"
      :series="BAR_SERIES"
      :height="200"
      aria-label="Eventos por hora e severidade"
    />
    <div v-else class="pa-8 text-center">
      <v-icon size="36" color="secondary" class="mb-2">mdi-chart-bar-stacked</v-icon>
      <div class="text-subtitle-2 font-weight-medium">Nenhum evento registrado no período</div>
    </div>
  </DashboardCard>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useEventsStore, type HourlyDistributionBin } from '@/stores/events'
import DashboardCard from '@/components/dashboard/DashboardCard.vue'
import StackedBarChart, { type BarSeries } from '@/components/charts/StackedBarChart.vue'
import { CHART_STATUS_COLORS } from '@/utils/charts/scale'

const BAR_SERIES: BarSeries[] = [
  { id: 'critical', label: 'Crítico', color: CHART_STATUS_COLORS.critical },
  { id: 'warning', label: 'Alerta', color: CHART_STATUS_COLORS.warning },
  { id: 'info', label: 'Info', color: CHART_STATUS_COLORS.info },
]

const eventsStore = useEventsStore()
const loading = ref(false)
const bins = ref<HourlyDistributionBin[]>([])
let unbindEvent: (() => void) | null = null

async function loadData() {
  loading.value = true
  try {
    const res = await eventsStore.fetchHourlyDistribution(6)
    if (res && Array.isArray(res.bins)) {
      bins.value = res.bins
    } else {
      buildEmptyBins()
    }
  } catch {
    buildEmptyBins()
  } finally {
    loading.value = false
  }
}

function buildEmptyBins() {
  const list: HourlyDistributionBin[] = []
  const now = new Date()
  for (let i = 5; i >= 0; i--) {
    const d = new Date(now.getTime() - i * 3600 * 1000)
    const hour = d.getHours()
    const label = `${hour.toString().padStart(2, '0')}:00`
    list.push({
      label,
      hour,
      timestamp: d.toISOString(),
      critical: 0,
      warning: 0,
      info: 0,
    })
  }
  bins.value = list
}

onMounted(async () => {
  await loadData()

  unbindEvent = eventsStore.onEvent(
    ['alert:triggered', 'alert:updated', 'alert:resolved'],
    (data) => {
      const severity = String(data.severity || 'info').toLowerCase()
      const now = new Date()
      const currentHour = now.getHours()

      if (bins.value.length === 0) {
        buildEmptyBins()
      }

      // Encontra ou atualiza o último balde correspondente à hora corrente
      const lastBin = bins.value[bins.value.length - 1]
      if (lastBin && lastBin.hour === currentHour) {
        if (severity === 'critical' || severity === 'error') {
          lastBin.critical++
        } else if (severity === 'warning') {
          lastBin.warning++
        } else {
          lastBin.info++
        }
      } else {
        // Se mudou de hora, recarrega a distribuição do servidor
        loadData()
      }
    }
  )
})

onUnmounted(() => {
  if (unbindEvent) {
    unbindEvent()
    unbindEvent = null
  }
})

const hourlyBins = computed(() => bins.value)

const categories = computed(() =>
  hourlyBins.value.map((bin) => ({
    label: bin.label,
    values: { critical: bin.critical, warning: bin.warning, info: bin.info },
  }))
)

const totals = computed(() => {
  let critical = 0
  let warning = 0
  let info = 0
  for (const b of hourlyBins.value) {
    critical += b.critical
    warning += b.warning
    info += b.info
  }
  return { critical, warning, info, total: critical + warning + info }
})
</script>
