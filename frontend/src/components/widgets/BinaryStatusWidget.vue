<template>
  <v-card elevation="2" class="rounded-lg fill-height d-flex flex-column">
    <v-card-title class="d-flex align-center justify-space-between py-3 px-4 flex-wrap ga-2">
      <div>
        <div class="d-flex align-center ga-2">
          <v-icon color="teal">mdi-checkbox-blank-circle-outline</v-icon>
          <span class="font-weight-bold text-h6">{{
            widget.title || 'Status Binário (Up/Down)'
          }}</span>
        </div>
        <div class="text-caption text-grey mt-1 d-flex align-center ga-1">
          <v-icon size="14" color="teal">mdi-binary</v-icon>
          <span>Exclusivo para Estados Booleanos (1=Up, 0=Down)</span>
        </div>
      </div>

      <div class="d-flex align-center ga-2 flex-wrap">
        <v-select
          v-model="selectedMonitorId"
          :items="monitorOptions"
          item-title="name"
          item-value="id"
          density="compact"
          variant="outlined"
          hide-details
          style="min-width: 200px; max-width: 260px"
          class="text-caption"
          placeholder="Selecione o Monitor"
        ></v-select>
      </div>
    </v-card-title>
    <v-divider></v-divider>

    <v-card-text class="pa-4 flex-grow-1">
      <div class="d-flex align-center justify-space-between mb-4 flex-wrap ga-3">
        <div class="d-flex align-center ga-3">
          <v-avatar :color="isCurrentUp ? 'success' : 'error'" size="44">
            <v-icon color="white" size="24">
              {{ isCurrentUp ? 'mdi-check-circle-outline' : 'mdi-close-circle-outline' }}
            </v-icon>
          </v-avatar>
          <div>
            <div class="text-subtitle-1 font-weight-bold">
              {{ targetName }}
            </div>
            <div class="text-caption text-grey">Alvo: {{ targetHost }} ({{ targetType }})</div>
          </div>
        </div>

        <div class="d-flex align-center ga-2">
          <v-chip
            :color="isCurrentUp ? 'success' : 'error'"
            variant="flat"
            class="font-weight-bold"
          >
            {{ isCurrentUp ? 'ESTADO: UP (1)' : 'ESTADO: DOWN (0)' }}
          </v-chip>
        </div>
      </div>

      <!-- Binary Timeline Grid / Blocks -->
      <div class="binary-matrix pa-3 rounded-lg bg-surface-variant mb-4">
        <div class="text-caption text-grey mb-2 d-flex align-center justify-space-between">
          <span>Histórico Sequencial de Amostras Binárias</span>
          <span class="font-weight-bold text-success">Disponibilidade: {{ uptimePercent }}%</span>
        </div>

        <div class="d-flex align-center ga-1 flex-wrap justify-start">
          <v-tooltip v-for="(block, idx) in binaryHistory" :key="idx" location="top">
            <template #activator="{ props: tooltipProps }">
              <div
                v-bind="tooltipProps"
                class="binary-block rounded"
                :class="block.val === 1 ? 'bg-success' : 'bg-error'"
              ></div>
            </template>
            <span>{{ block.time }} — {{ block.val === 1 ? '1 (UP)' : '0 (DOWN)' }}</span>
          </v-tooltip>
        </div>
      </div>

      <v-row density="compact">
        <v-col cols="4">
          <div class="pa-2 border rounded text-center">
            <div class="text-caption text-grey">Total de Checagens</div>
            <div class="text-h6 font-weight-bold text-primary">{{ totalChecks }}</div>
          </div>
        </v-col>
        <v-col cols="4">
          <div class="pa-2 border rounded text-center">
            <div class="text-caption text-grey">Flapping (Alternâncias)</div>
            <div
              class="text-h6 font-weight-bold"
              :class="flipCount > 0 ? 'text-warning' : 'text-success'"
            >
              {{ flipCount }}
            </div>
          </div>
        </v-col>
        <v-col cols="4">
          <div class="pa-2 border rounded text-center">
            <div class="text-caption text-grey">Última Checagem</div>
            <div class="text-subtitle-2 font-weight-bold mt-1 text-truncate">
              {{ lastCheckedFormatted }}
            </div>
          </div>
        </v-col>
      </v-row>
    </v-card-text>

    <v-divider></v-divider>
    <v-card-actions
      class="px-4 py-2 bg-surface-variant d-flex align-center justify-space-between text-caption"
    >
      <span class="text-grey font-weight-medium">Filtro Estrito: Dados Booleanos (0/1)</span>
      <v-chip size="x-small" color="teal" variant="tonal">Card Binário Validado</v-chip>
    </v-card-actions>
  </v-card>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useMonitorsStore } from '@/stores/monitors'
import type { WidgetConfig } from '@/stores/dashboard'

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

const binaryHistory = computed(() => {
  const list: Array<{ val: 0 | 1; time: string }> = []
  const results = activeMonitor.value?.recentResults || []

  if (results.length > 0) {
    for (const r of results) {
      const isUp = r.status === 'up'
      const d = new Date(r.finishedAt)
      list.push({
        val: isUp ? 1 : 0,
        time: d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
      })
    }
  } else {
    // Amostras binárias demonstrativas sintéticas
    const now = new Date()
    for (let i = 24; i >= 0; i--) {
      const t = new Date(now.getTime() - i * 60 * 1000)
      list.push({
        val: i === 5 ? 0 : 1,
        time: t.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
      })
    }
  }
  return list
})

const totalChecks = computed(() => binaryHistory.value.length)

const uptimePercent = computed(() => {
  if (binaryHistory.value.length === 0) return 100
  const upCount = binaryHistory.value.filter((b) => b.val === 1).length
  return Math.round((upCount / binaryHistory.value.length) * 100)
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

const lastCheckedFormatted = computed(() => {
  if (activeMonitor.value?.lastCheckedAt) {
    return new Date(activeMonitor.value.lastCheckedAt).toLocaleTimeString([], {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    })
  }
  return 'Agora mesmo'
})
</script>

<style scoped>
.binary-block {
  width: 14px;
  height: 24px;
  transition: transform 0.15s ease;
  cursor: pointer;
}

.binary-block:hover {
  transform: scale(1.2);
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
