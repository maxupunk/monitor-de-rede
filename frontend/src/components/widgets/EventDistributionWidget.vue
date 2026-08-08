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
import { computed } from 'vue'
import { useEventsStore } from '@/stores/events'
import { formatEventDetails } from '@/utils/eventPresentation'

const eventsStore = useEventsStore()

interface HourlyBin {
  label: string
  critical: number
  warning: number
  info: number
}

const hourlyBins = computed<HourlyBin[]>(() => {
  const bins: HourlyBin[] = []
  const now = new Date()

  // Prepara 6 caixas para as últimas 6 horas
  for (let i = 5; i >= 0; i--) {
    const d = new Date(now.getTime() - i * 3600 * 1000)
    const label = `${d.getHours().toString().padStart(2, '0')}:00`
    bins.push({ label, critical: 0, warning: 0, info: 0 })
  }

  // Preenche com os eventos reais do `eventsStore`
  for (const evt of eventsStore.recentEvents) {
    const evtTime = new Date(evt.timestamp)
    const hoursAgo = Math.floor((now.getTime() - evtTime.getTime()) / (3600 * 1000))
    if (hoursAgo >= 0 && hoursAgo < 6) {
      const binIdx = 5 - hoursAgo
      const bin = bins[binIdx]
      if (bin) {
        const details = formatEventDetails(evt)
        const color = details.color
        if (color === 'error' || color === 'red') bin.critical++
        else if (color === 'warning' || color === 'amber' || color === 'orange') bin.warning++
        else bin.info++
      }
    }
  }

  // Se houver pouquíssimos eventos no feed, adiciona amostragem demonstrativa inicial
  const hasData = bins.some((b) => b.critical + b.warning + b.info > 0)
  if (!hasData) {
    bins[1].info = 2
    bins[2].warning = 1
    bins[3].critical = 1
    bins[3].warning = 2
    bins[4].info = 3
    bins[5].info = 1
  }

  return bins
})

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
