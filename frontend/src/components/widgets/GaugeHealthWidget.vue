<template>
  <v-card elevation="2" class="rounded-lg fill-height d-flex flex-column">
    <v-card-title class="d-flex align-center justify-space-between py-3 px-4">
      <div class="d-flex align-center ga-2">
        <v-icon color="success">mdi-gauge</v-icon>
        <span class="font-weight-bold text-h6">Saúde Global da Rede</span>
      </div>
      <v-chip color="success" size="x-small" variant="tonal" class="font-weight-medium">
        Tempo Real
      </v-chip>
    </v-card-title>
    <v-divider></v-divider>

    <v-card-text class="pa-4 flex-grow-1 d-flex align-center justify-center">
      <div class="w-100 d-flex flex-column flex-sm-row align-center justify-space-around ga-4">
        <!-- SVG Donut Chart -->
        <div class="relative gauge-container d-flex align-center justify-center">
          <svg viewBox="0 0 160 160" class="gauge-svg">
            <!-- Background Ring -->
            <circle
              cx="80"
              cy="80"
              r="62"
              fill="none"
              stroke="rgba(148, 163, 184, 0.2)"
              stroke-width="14"
            />
            <!-- Status Segments -->
            <circle
              v-for="(seg, idx) in strokeSegments"
              :key="idx"
              cx="80"
              cy="80"
              r="62"
              fill="none"
              :stroke="seg.color"
              stroke-width="14"
              :stroke-dasharray="`${seg.dash} ${seg.gap}`"
              :stroke-dashoffset="seg.offset"
              stroke-linecap="round"
              class="gauge-circle"
            />
          </svg>

          <!-- Central Text -->
          <div class="absolute-center text-center">
            <div class="text-h4 font-weight-bold text-success">{{ globalUptime }}%</div>
            <div class="text-caption text-grey font-weight-medium">Disponibilidade</div>
          </div>
        </div>

        <!-- Legend / Stats breakdown -->
        <div class="legend-box d-flex flex-column ga-2 w-100 w-sm-auto">
          <div
            class="d-flex align-center justify-space-between ga-4 pa-2 rounded bg-surface-variant"
          >
            <div class="d-flex align-center ga-2">
              <v-avatar color="success" size="10"></v-avatar>
              <span class="text-caption font-weight-bold">Operacionais (Up)</span>
            </div>
            <span class="text-caption font-weight-bold text-success">{{ counts.up }}</span>
          </div>

          <div
            class="d-flex align-center justify-space-between ga-4 pa-2 rounded bg-surface-variant"
          >
            <div class="d-flex align-center ga-2">
              <v-avatar color="warning" size="10"></v-avatar>
              <span class="text-caption font-weight-bold">Em Alerta (Warning)</span>
            </div>
            <span class="text-caption font-weight-bold text-warning">{{ counts.warning }}</span>
          </div>

          <div
            class="d-flex align-center justify-space-between ga-4 pa-2 rounded bg-surface-variant"
          >
            <div class="d-flex align-center ga-2">
              <v-avatar color="error" size="10"></v-avatar>
              <span class="text-caption font-weight-bold">Fora do Ar (Down)</span>
            </div>
            <span class="text-caption font-weight-bold text-error">{{ counts.down }}</span>
          </div>

          <div
            class="d-flex align-center justify-space-between ga-4 pa-2 rounded bg-surface-variant"
          >
            <div class="d-flex align-center ga-2">
              <v-avatar color="grey-darken-1" size="10"></v-avatar>
              <span class="text-caption font-weight-bold">Desconhecido</span>
            </div>
            <span class="text-caption font-weight-bold text-grey">{{ counts.unknown }}</span>
          </div>
        </div>
      </div>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useMonitorsStore } from '@/stores/monitors'

const monitorsStore = useMonitorsStore()

const counts = computed(() => {
  let up = 0
  let warning = 0
  let down = 0
  let unknown = 0

  for (const m of monitorsStore.monitors) {
    if (m.status === 'online' || m.status === 'up') up++
    else if (m.status === 'warning') warning++
    else if (m.status === 'offline' || m.status === 'down') down++
    else unknown++
  }

  return { up, warning, down, unknown, total: monitorsStore.monitors.length }
})

const globalUptime = computed(() => {
  if (counts.value.total === 0) return 100
  return Math.round((counts.value.up / counts.value.total) * 100)
})

/**
 * Calculates SVG stroke-dasharray and stroke-dashoffset for each arc segment on a circle with radius 62.
 * Circumference = 2 * PI * 62 ≈ 389.56
 */
const strokeSegments = computed(() => {
  const R = 62
  const C = 2 * Math.PI * R
  const total = counts.value.total || 1

  const items = [
    { count: counts.value.up, color: '#22c55e' },
    { count: counts.value.warning, color: '#f59e0b' },
    { count: counts.value.down, color: '#ef4444' },
    { count: counts.value.unknown, color: '#64748b' },
  ]

  let currentOffset = 0
  const result = []

  for (const item of items) {
    if (item.count <= 0) continue
    const ratio = item.count / total
    const dash = Math.max(0, ratio * C - 4) // 4px gap between segments
    const gap = C - dash
    const offset = -currentOffset

    result.push({
      color: item.color,
      dash: dash.toFixed(1),
      gap: gap.toFixed(1),
      offset: offset.toFixed(1),
    })

    currentOffset += ratio * C
  }

  return result
})
</script>

<style scoped>
.gauge-container {
  width: 170px;
  height: 170px;
  position: relative;
}

.gauge-svg {
  width: 100%;
  height: 100%;
  transform: rotate(-90deg);
}

.gauge-circle {
  transition:
    stroke-dasharray 0.5s ease,
    stroke-dashoffset 0.5s ease;
}

.absolute-center {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  pointer-events: none;
}

.legend-box {
  min-width: 200px;
}

.ga-2 {
  gap: 8px;
}
.ga-4 {
  gap: 16px;
}
</style>
