<template>
  <v-card elevation="2" class="rounded-lg fill-height d-flex flex-column">
    <v-card-title class="d-flex align-center justify-space-between py-3 px-4">
      <div class="d-flex align-center ga-2">
        <v-icon color="deep-purple">mdi-chart-bar</v-icon>
        <span class="font-weight-bold text-h6">Distribuição de Eventos por Hora</span>
      </div>
      <v-chip color="deep-purple" size="x-small" variant="tonal" class="font-weight-medium">
        Últimas 6 Horas
      </v-chip>
    </v-card-title>
    <v-divider></v-divider>

    <v-card-text class="pa-4 flex-grow-1 d-flex flex-column justify-center">
      <div v-if="hourlyBins.length > 0" class="w-100">
        <svg viewBox="0 0 500 160" class="w-100 bar-svg">
          <!-- Horizontal Grid Lines -->
          <line
            x1="40"
            y1="20"
            x2="480"
            y2="20"
            stroke="rgba(148, 163, 184, 0.2)"
            stroke-dasharray="3,3"
          />
          <text x="32" y="24" font-size="10" fill="#94a3b8" text-anchor="end">{{ maxCount }}</text>

          <line
            x1="40"
            y1="80"
            x2="480"
            y2="80"
            stroke="rgba(148, 163, 184, 0.2)"
            stroke-dasharray="3,3"
          />
          <text x="32" y="84" font-size="10" fill="#94a3b8" text-anchor="end">
            {{ Math.round(maxCount / 2) }}
          </text>

          <line
            x1="40"
            y1="140"
            x2="480"
            y2="140"
            stroke="rgba(148, 163, 184, 0.3)"
            stroke-width="1.5"
          />
          <text x="32" y="144" font-size="10" fill="#94a3b8" text-anchor="end">0</text>

          <!-- Grouped Bars per hour -->
          <g v-for="(bin, bIdx) in hourlyBins" :key="bIdx">
            <!-- Critical Bar -->
            <rect
              v-if="bin.critical > 0"
              :x="barPosition(bIdx, 0)"
              :y="barY(bin.critical)"
              :width="barWidth"
              :height="barHeight(bin.critical)"
              fill="#ef4444"
              rx="2"
            />
            <!-- Warning Bar -->
            <rect
              v-if="bin.warning > 0"
              :x="barPosition(bIdx, 1)"
              :y="barY(bin.warning)"
              :width="barWidth"
              :height="barHeight(bin.warning)"
              fill="#f59e0b"
              rx="2"
            />
            <!-- Info Bar -->
            <rect
              v-if="bin.info > 0"
              :x="barPosition(bIdx, 2)"
              :y="barY(bin.info)"
              :width="barWidth"
              :height="barHeight(bin.info)"
              fill="#3b82f6"
              rx="2"
            />

            <!-- Hour Label below axis -->
            <text :x="groupCenter(bIdx)" y="156" font-size="10" fill="#94a3b8" text-anchor="middle">
              {{ bin.label }}
            </text>
          </g>
        </svg>
      </div>

      <div v-else class="pa-8 text-center text-grey">
        <v-icon size="40" color="grey-lighten-1" class="mb-2">mdi-chart-bar-stacked</v-icon>
        <div class="text-subtitle-2 font-weight-medium">Nenhum evento registrado no período</div>
      </div>
    </v-card-text>

    <!-- Legend Footer -->
    <v-divider></v-divider>
    <v-card-actions
      class="px-4 py-2 bg-surface-variant d-flex align-center justify-space-between text-caption flex-wrap ga-2"
    >
      <div class="d-flex align-center ga-3">
        <div class="d-flex align-center ga-1">
          <span class="dot-indicator bg-error"></span>
          <span>Crítico ({{ totals.critical }})</span>
        </div>
        <div class="d-flex align-center ga-1">
          <span class="dot-indicator bg-warning"></span>
          <span>Alerta ({{ totals.warning }})</span>
        </div>
        <div class="d-flex align-center ga-1">
          <span class="dot-indicator bg-info"></span>
          <span>Info ({{ totals.info }})</span>
        </div>
      </div>
      <span class="text-grey font-weight-bold">Total: {{ totals.total }}</span>
    </v-card-actions>
  </v-card>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useEventsStore, type HourlyDistributionBin } from '@/stores/events'

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

const maxCount = computed(() => {
  let max = 1
  for (const b of hourlyBins.value) {
    const localMax = Math.max(b.critical, b.warning, b.info)
    if (localMax > max) max = localMax
  }
  return Math.ceil(max * 1.2)
})

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

const barWidth = 14
const groupGap = 70

function groupCenter(bIdx: number): number {
  return 55 + bIdx * groupGap + 21
}

function barPosition(bIdx: number, subIdx: number): number {
  return 55 + bIdx * groupGap + subIdx * 15
}

function barY(count: number): number {
  const top = 20
  const bottom = 140
  const height = bottom - top
  const ratio = Math.min(1, count / maxCount.value)
  return bottom - ratio * height
}

function barHeight(count: number): number {
  const top = 20
  const bottom = 140
  const height = bottom - top
  const ratio = Math.min(1, count / maxCount.value)
  return ratio * height
}
</script>

<style scoped>
.bar-svg {
  height: 160px;
}

.dot-indicator {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  display: inline-block;
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
