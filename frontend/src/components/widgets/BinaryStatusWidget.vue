<template>
  <DashboardCard
    :title="widget.title || 'Status Binário (Up/Down)'"
    icon="mdi-checkbox-blank-circle-outline"
    icon-color="teal"
    subtitle="Estado booleano do monitor (1 = Up, 0 = Down)"
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
        class="binary-widget__select"
        placeholder="Selecione o Monitor"
      ></v-select>
    </template>

    <div class="d-flex align-center justify-space-between mb-4 flex-wrap ga-3">
      <div class="d-flex align-center ga-3">
        <v-avatar :color="isCurrentUp ? 'success' : 'error'" size="44">
          <v-icon color="white" size="24">
            {{ isCurrentUp ? 'mdi-check-circle-outline' : 'mdi-close-circle-outline' }}
          </v-icon>
        </v-avatar>
        <div>
          <div class="text-subtitle-1 font-weight-bold">{{ targetName }}</div>
          <div class="text-caption">Alvo: {{ targetHost }} ({{ targetType }})</div>
        </div>
      </div>

      <v-chip :color="isCurrentUp ? 'success' : 'error'" variant="flat" class="font-weight-bold">
        {{ isCurrentUp ? 'ESTADO: UP (1)' : 'ESTADO: DOWN (0)' }}
      </v-chip>
    </div>

    <!-- Linha do tempo binária: um bloco por checagem -->
    <div class="binary-widget__matrix mb-4">
      <div class="text-caption mb-2 d-flex align-center justify-space-between">
        <span>Histórico sequencial das checagens</span>
        <span class="font-weight-bold text-success">
          Disponibilidade: {{ formatPercent(uptimePercent, 0) }}
        </span>
      </div>

      <div v-if="binaryHistory.length > 0" class="d-flex align-center ga-1 flex-wrap">
        <v-tooltip v-for="(block, idx) in binaryHistory" :key="idx" location="top">
          <template #activator="{ props: tooltipProps }">
            <div
              v-bind="tooltipProps"
              class="binary-widget__block rounded"
              :class="block.val === 1 ? 'bg-success' : 'bg-error'"
            ></div>
          </template>
          <span>{{ block.time }} — {{ block.val === 1 ? '1 (UP)' : '0 (DOWN)' }}</span>
        </v-tooltip>
      </div>
      <div v-else class="text-body-2">Nenhuma checagem registrada para este monitor.</div>
    </div>

    <v-row dense>
      <v-col cols="4">
        <div class="binary-widget__kpi">
          <div class="binary-widget__kpi-label">Total de checagens</div>
          <div class="text-h6 font-weight-bold text-primary">{{ totalChecks }}</div>
        </div>
      </v-col>
      <v-col cols="4">
        <div class="binary-widget__kpi">
          <div class="binary-widget__kpi-label">Alternâncias (flapping)</div>
          <div
            class="text-h6 font-weight-bold"
            :class="flipCount > 0 ? 'text-warning' : 'text-success'"
          >
            {{ flipCount }}
          </div>
        </div>
      </v-col>
      <v-col cols="4">
        <div class="binary-widget__kpi">
          <div class="binary-widget__kpi-label">Última checagem</div>
          <div class="text-subtitle-2 font-weight-bold mt-1 text-truncate">
            {{ lastCheckedFormatted }}
          </div>
        </div>
      </v-col>
    </v-row>
  </DashboardCard>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useMonitorsStore } from '@/stores/monitors'
import type { WidgetConfig } from '@/stores/dashboard'
import { formatClockTime, formatPercent, formatShortDateTime } from '@/utils/formatters'
import DashboardCard from '@/components/dashboard/DashboardCard.vue'

const props = defineProps<{
  widget: WidgetConfig
}>()

const monitorsStore = useMonitorsStore()
const selectedMonitorId = ref<number | 'first'>((props.widget.config?.monitorId as any) || 'first')

onMounted(async () => {
  if (monitorsStore.monitors.length === 0) {
    await monitorsStore.fetchMonitors()
  }
})

const monitorOptions = computed(() => {
  const options: Array<{ id: number | 'first'; name: string }> = []
  for (const m of monitorsStore.monitors) {
    options.push({
      id: m.id,
      name: `${m.name} (${(m.type || 'icmp').toUpperCase()})`,
    })
  }
  return options
})

const activeMonitor = computed(() => {
  if (selectedMonitorId.value === 'first') {
    return monitorsStore.monitors[0] || null
  }
  return monitorsStore.monitors.find((m) => m.id === selectedMonitorId.value) || null
})

const targetName = computed(() => activeMonitor.value?.name || 'Monitor de Status')
const targetHost = computed(() => activeMonitor.value?.target || '127.0.0.1')
const targetType = computed(() => (activeMonitor.value?.type || 'icmp').toUpperCase())

const isCurrentUp = computed(() => {
  if (!activeMonitor.value) return true
  return activeMonitor.value.status === 'up'
})

/** Uma entrada por checagem real; sem resultados, a linha do tempo fica vazia. */
const binaryHistory = computed(() =>
  (activeMonitor.value?.recentResults || []).map((r) => ({
    val: (r.status === 'up' ? 1 : 0) as 0 | 1,
    time: formatShortDateTime(r.finishedAt),
  }))
)

const totalChecks = computed(() => binaryHistory.value.length)

const uptimePercent = computed(() => {
  if (binaryHistory.value.length === 0) return 100
  const upCount = binaryHistory.value.filter((b) => b.val === 1).length
  return (upCount / binaryHistory.value.length) * 100
})

const flipCount = computed(() => {
  let flips = 0
  for (let i = 1; i < binaryHistory.value.length; i++) {
    if (binaryHistory.value[i].val !== binaryHistory.value[i - 1].val) {
      flips++
    }
  }
  return flips
})

const lastCheckedFormatted = computed(() =>
  activeMonitor.value?.lastCheckedAt ? formatClockTime(activeMonitor.value.lastCheckedAt) : '—'
)
</script>

<style scoped>
.binary-widget__select {
  min-width: 200px;
  max-width: 260px;
}

.binary-widget__matrix {
  padding: 12px;
  border-radius: 8px;
  background: rgba(var(--v-theme-primary), 0.06);
}

.binary-widget__block {
  width: 14px;
  height: 24px;
  transition: transform 0.15s ease;
  cursor: pointer;
}

.binary-widget__block:hover {
  transform: scale(1.2);
}

.binary-widget__kpi {
  padding: 8px;
  border-radius: 8px;
  text-align: center;
  background: rgba(var(--v-theme-primary), 0.06);
}

.binary-widget__kpi-label {
  font-size: 0.75rem;
  color: rgba(var(--v-theme-on-surface), 0.8);
}
</style>
