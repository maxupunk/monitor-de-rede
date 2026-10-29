<template>
  <DashboardCard
    title="Saúde Global da Rede"
    icon="mdi-gauge"
    icon-color="success"
    subtitle="Disponibilidade dos monitores ativos, em tempo real"
  >
    <div class="gauge-widget">
      <!-- Anel em pixels reais: 1 unidade do SVG = 1 px na tela -->
      <div class="gauge-widget__ring">
        <svg
          :width="SIZE"
          :height="SIZE"
          :viewBox="`0 0 ${SIZE} ${SIZE}`"
          class="gauge-widget__svg"
          role="img"
          :aria-label="`Disponibilidade global de ${globalUptime}%`"
        >
          <circle
            :cx="CENTER"
            :cy="CENTER"
            :r="RADIUS"
            fill="none"
            :stroke-width="RING"
            class="gauge-widget__track"
          />
          <circle
            v-for="seg in strokeSegments"
            :key="seg.status"
            :cx="CENTER"
            :cy="CENTER"
            :r="RADIUS"
            fill="none"
            :stroke="seg.color"
            :stroke-width="RING"
            :stroke-dasharray="`${seg.dash} ${seg.gap}`"
            :stroke-dashoffset="seg.offset"
            class="gauge-widget__segment"
            @click="navigateToStatus(seg.status)"
          >
            <title>Ver monitores: {{ seg.label }}</title>
          </circle>
        </svg>

        <div class="gauge-widget__center">
          <div class="text-h4 font-weight-bold" :class="`text-${uptimeColor}`">
            {{ globalUptime }}%
          </div>
          <div class="text-caption font-weight-medium">Disponibilidade</div>
        </div>
      </div>

      <v-list density="compact" class="gauge-widget__legend pa-0" bg-color="transparent">
        <v-list-item
          v-for="row in legendRows"
          :key="row.status"
          rounded="lg"
          class="px-2"
          @click="navigateToStatus(row.status)"
        >
          <template #prepend>
            <span class="gauge-widget__swatch" :style="{ backgroundColor: row.color }"></span>
          </template>
          <v-list-item-title class="text-body-2 font-weight-medium">
            {{ row.label }}
          </v-list-item-title>
          <template #append>
            <span class="text-body-2 font-weight-bold">{{ row.count }}</span>
          </template>
          <v-tooltip v-if="row.hint" activator="parent" location="top">{{ row.hint }}</v-tooltip>
        </v-list-item>
      </v-list>
    </div>
  </DashboardCard>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import DashboardCard from '@/components/dashboard/DashboardCard.vue'
import { useMonitorsStore } from '@/stores/monitors'
import { CHART_STATUS_COLORS } from '@/utils/charts/scale'
import {
  monitorHealthCounts,
  monitorUptimePercent,
  type MonitorHealthBucket,
} from '@/utils/monitorPresentation'

const SIZE = 168
const CENTER = SIZE / 2
const RING = 14
const RADIUS = CENTER - RING / 2 - 2
/** Folga, em px, entre fatias vizinhas do anel. */
const SEGMENT_GAP = 3

const router = useRouter()
const monitorsStore = useMonitorsStore()

function navigateToStatus(status: MonitorHealthBucket) {
  router.push({ path: '/monitors', query: { status } })
}

const counts = computed(() => monitorHealthCounts(monitorsStore.monitors))

const globalUptime = computed(() => monitorUptimePercent(counts.value))

const uptimeColor = computed(() => {
  const value = Number(globalUptime.value)
  if (value >= 99) return 'success'
  if (value >= 95) return 'warning'
  return 'error'
})

const SEGMENTS: Array<{ status: MonitorHealthBucket; label: string; color: string }> = [
  { status: 'up', label: 'Operacionais', color: CHART_STATUS_COLORS.ok },
  { status: 'warning', label: 'Em alerta', color: CHART_STATUS_COLORS.warning },
  { status: 'down', label: 'Fora do ar', color: CHART_STATUS_COLORS.critical },
  { status: 'unknown', label: 'Desconhecidos', color: CHART_STATUS_COLORS.muted },
]

const legendRows = computed(() => [
  ...SEGMENTS.map((seg) => ({ ...seg, count: counts.value[seg.status], hint: '' })),
  ...(counts.value.disabled > 0
    ? [
        {
          status: 'disabled' as MonitorHealthBucket,
          label: 'Desativados',
          color: CHART_STATUS_COLORS.muted,
          count: counts.value.disabled,
          hint: 'Monitores desligados pelo operador — fora da conta de disponibilidade',
        },
      ]
    : []),
])

/**
 * Fatias do anel por `stroke-dasharray`. O anel mostra o que está sendo
 * verificado: monitor desativado não ocupa fatia, do mesmo jeito que não
 * entra na disponibilidade do centro.
 */
const strokeSegments = computed(() => {
  const circumference = 2 * Math.PI * RADIUS
  const total = counts.value.monitored || 1
  const visible = SEGMENTS.filter((seg) => counts.value[seg.status] > 0)
  const gap = visible.length > 1 ? SEGMENT_GAP : 0

  let consumed = 0
  return visible.map((seg) => {
    const length = (counts.value[seg.status] / total) * circumference
    const dash = Math.max(0, length - gap)
    const result = {
      ...seg,
      dash: dash.toFixed(2),
      gap: (circumference - dash).toFixed(2),
      offset: (-consumed).toFixed(2),
    }
    consumed += length
    return result
  })
})
</script>

<style scoped>
.gauge-widget {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-around;
  gap: 16px;
  height: 100%;
}

.gauge-widget__ring {
  position: relative;
  flex: 0 0 auto;
}

.gauge-widget__svg {
  display: block;
  transform: rotate(-90deg);
}

.gauge-widget__track {
  stroke: rgba(var(--v-theme-on-surface), 0.08);
}

.gauge-widget__segment {
  cursor: pointer;
  transition:
    stroke-dasharray 0.5s ease,
    stroke-dashoffset 0.5s ease,
    opacity 0.15s ease;
}

.gauge-widget__segment:hover {
  opacity: 0.8;
}

.gauge-widget__center {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  pointer-events: none;
}

.gauge-widget__legend {
  flex: 1 1 200px;
  max-width: 280px;
}

.gauge-widget__swatch {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  margin-right: 10px;
}
</style>
