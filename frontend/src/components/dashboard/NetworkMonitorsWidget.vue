<template>
  <v-card elevation="2" class="rounded-lg">
    <v-card-title class="d-flex align-center justify-space-between py-3 px-4 flex-wrap ga-2">
      <div class="d-flex align-center">
        <v-icon start color="primary">mdi-chart-timeline-variant</v-icon>
        <span class="font-weight-bold text-h6">Monitores de Rede</span>
        <v-chip size="x-small" color="primary" class="ml-2" variant="tonal">
          {{ monitorsStore.activeMonitors.length }}
        </v-chip>
      </div>
      <v-btn
        variant="text"
        color="primary"
        size="small"
        append-icon="mdi-arrow-right"
        to="/monitors"
      >
        Ver Todos os Monitores
      </v-btn>
    </v-card-title>
    <v-divider></v-divider>

    <v-card-text class="pa-0">
      <div v-if="monitorsStore.activeMonitors.length > 0">
        <v-list class="monitors-scroll-container pa-0">
          <v-list-item
            v-for="monitor in monitorsStore.activeMonitors"
            :key="monitor.id"
            class="px-4 py-3 border-b cursor-pointer"
            @click="abrirMonitor(monitor.id)"
          >
            <div
              class="d-flex flex-column flex-md-row align-start align-md-center justify-space-between ga-3 w-100"
            >
              <div class="monitor-info d-flex align-center ga-3" style="min-width: 220px; flex: 1">
                <v-avatar :color="getStatusColor(monitor.status)" size="10" class="mr-1"></v-avatar>
                <div>
                  <a
                    class="text-subtitle-1 font-weight-bold text-decoration-none text-primary hover-underline d-block cursor-pointer"
                    role="button"
                    tabindex="0"
                    :href="'/monitors/' + monitor.id"
                    @click.prevent.stop="abrirMonitor(monitor.id)"
                    @keydown.enter.prevent="abrirMonitor(monitor.id)"
                  >
                    {{ monitor.name }}
                  </a>

                  <div class="d-flex align-center ga-2 mt-1">
                    <v-chip size="x-small" color="info" variant="tonal">
                      {{ (monitor.type || 'N/A').toUpperCase() }}
                    </v-chip>
                    <span class="text-caption text-grey-darken-1">{{ monitor.target }}</span>
                  </div>
                </div>
              </div>

              <div
                class="monitor-timeline d-flex align-center justify-start justify-md-center monitor-timeline-scroll"
                style="flex: 2; min-width: 280px"
              >
                <a
                  class="text-decoration-none d-flex align-center ga-2 cursor-pointer"
                  :href="'/monitors/' + monitor.id"
                  @click.prevent.stop="abrirMonitor(monitor.id)"
                >
                  <template v-if="isGaugeMonitor(monitor)">
                    <MonitorSparkline
                      :data="monitor.gaugeHistory || []"
                      :color="gaugeSparklineColor(monitor)"
                      :width="189"
                      :height="28"
                      :unit="gaugeDisplayUnit(monitor)"
                    />
                    <span class="text-caption font-weight-medium text-high-emphasis text-no-wrap">
                      {{ formatGaugeShortValue(monitor) }}
                    </span>
                  </template>
                  <MonitorTimelineBar
                    v-else
                    :results="monitor.recentResults"
                    :max-blocks="24"
                    :height="20"
                    :width="5"
                  ></MonitorTimelineBar>
                </a>
              </div>

              <div
                class="monitor-actions d-flex align-center ga-2 justify-start justify-md-end"
                style="min-width: 140px"
              >
                <v-chip
                  :color="getStatusColor(monitor.status)"
                  size="small"
                  variant="tonal"
                  class="font-weight-medium"
                >
                  {{ (monitor.status || 'UNKNOWN').toUpperCase() }}
                </v-chip>
                <v-btn
                  size="small"
                  color="primary"
                  variant="outlined"
                  prepend-icon="mdi-play"
                  :loading="monitorsStore.runningId === monitor.id"
                  @click.stop="monitorsStore.runMonitor(monitor.id)"
                >
                  Testar
                </v-btn>
              </div>
            </div>
          </v-list-item>
        </v-list>
      </div>
      <div v-else class="pa-8 text-center text-grey">
        <v-icon size="48" color="grey-lighten-1" class="mb-2">
          mdi-chart-timeline-variant-off
        </v-icon>
        <div class="text-subtitle-1 font-weight-medium">
          {{
            monitorsStore.monitors.length > 0
              ? 'Nenhum monitor ativo no momento'
              : 'Nenhum monitor cadastrado'
          }}
        </div>
        <p class="text-caption text-grey-darken-1 mb-4">
          {{
            monitorsStore.monitors.length > 0
              ? 'Todos os monitores cadastrados estão desativados.'
              : 'Cadastre monitores ICMP, HTTP ou TCP para visualizar os gráficos em barras.'
          }}
        </p>
        <v-btn color="primary" prepend-icon="mdi-plus" to="/monitors">
          {{ monitorsStore.monitors.length > 0 ? 'Gerenciar Monitores' : 'Cadastrar Monitor' }}
        </v-btn>
      </div>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import { useMonitorsStore, type Monitor } from '@/stores/monitors'
import { useMonitorDetail } from '@/composables/useMonitorDetail'
import MonitorTimelineBar from '@/components/MonitorTimelineBar.vue'
import MonitorSparkline from '@/components/MonitorSparkline.vue'
import {
  getStatusColor,
  isGaugeMonitor,
  gaugeMetricName,
  gaugeDisplayUnit,
  gaugeUsagePercent,
  formatGaugeValue,
  gaugeHexColor,
} from '@/utils/monitorPresentation'

const monitorsStore = useMonitorsStore()
const { abrirDetalhe: abrirMonitor } = useMonitorDetail()

function gaugeSparklineColor(monitor: Monitor): string {
  return gaugeHexColor(gaugeUsagePercent(monitor), gaugeMetricName(monitor))
}

function formatGaugeShortValue(item: Monitor): string {
  return formatGaugeValue(item, true)
}
</script>

<style scoped>
.hover-underline:hover {
  text-decoration: underline !important;
}

.cursor-pointer {
  cursor: pointer;
}

.monitors-scroll-container {
  max-height: 420px;
  overflow-y: auto;
}

.monitors-scroll-container::-webkit-scrollbar {
  width: 6px;
}

.monitors-scroll-container::-webkit-scrollbar-thumb {
  background: rgba(148, 163, 184, 0.4);
  border-radius: 4px;
}

.monitors-scroll-container::-webkit-scrollbar-thumb:hover {
  background: rgba(148, 163, 184, 0.7);
}
</style>
