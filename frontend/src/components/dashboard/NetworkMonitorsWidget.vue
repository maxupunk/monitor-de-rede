<template>
  <DashboardCard
    title="Monitores de rede"
    icon="mdi-heart-pulse"
    :subtitle="`${monitorsStore.activeMonitors.length} ativos`"
    to="/monitors"
    to-label="Ver todos"
    flush
  >
    <div v-if="monitorsStore.activeMonitors.length > 0">
      <v-list class="monitors-scroll-container pa-0">
        <v-list-item
          v-for="monitor in monitorsStore.activeMonitors"
          :key="monitor.id"
          class="px-5 py-3 cursor-pointer"
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
                  <span class="text-caption">{{ monitor.target }}</span>
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
                    :in-data="
                      isTrafficMonitor(monitor) ? getMonitorTrafficInData(monitor) : undefined
                    "
                    :out-data="
                      isTrafficMonitor(monitor) ? getMonitorTrafficOutData(monitor) : undefined
                    "
                    :color="gaugeSparklineColor(monitor)"
                    :width="189"
                    :height="28"
                    :unit="gaugeDisplayUnit(monitor)"
                  />
                  <div
                    v-if="isTrafficMonitor(monitor)"
                    class="d-flex flex-column text-no-wrap"
                    style="min-width: 68px; line-height: 1.15"
                  >
                    <span
                      class="d-flex align-center ga-1 text-success"
                      style="font-size: 11px; font-weight: 600"
                    >
                      <v-icon size="12">mdi-arrow-down</v-icon>
                      {{ formatBps(getMonitorInBps(monitor) ?? 0, { fractionDigits: 1 }) }}
                    </span>
                    <span
                      class="d-flex align-center ga-1 text-info"
                      style="font-size: 11px; font-weight: 600"
                    >
                      <v-icon size="12">mdi-arrow-up</v-icon>
                      {{ formatBps(getMonitorOutBps(monitor) ?? 0, { fractionDigits: 1 }) }}
                    </span>
                  </div>
                  <span
                    v-else
                    class="text-caption font-weight-medium text-high-emphasis text-no-wrap"
                  >
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
    <div v-else class="dashboard-empty">
      <v-icon size="40" color="primary"> mdi-chart-timeline-variant-off </v-icon>
      <div class="text-subtitle-2 font-weight-bold mt-2">
        {{
          monitorsStore.monitors.length > 0
            ? 'Nenhum monitor ativo no momento'
            : 'Nenhum monitor cadastrado'
        }}
      </div>
      <p class="text-body-2 mb-4">
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
    <MonitorDetailDialog v-model="detalheAberto" :monitor-id="monitorEmDetalhe" />
  </DashboardCard>
</template>

<script setup lang="ts">
import { useMonitorsStore, type Monitor } from '@/stores/monitors'
import { useMonitorDetail } from '@/composables/useMonitorDetail'
import DashboardCard from '@/components/dashboard/DashboardCard.vue'
import MonitorDetailDialog from '@/components/monitors/MonitorDetailDialog.vue'
import MonitorTimelineBar from '@/components/MonitorTimelineBar.vue'
import MonitorSparkline from '@/components/MonitorSparkline.vue'
import { formatBps } from '@/utils/formatters'
import {
  getStatusColor,
  isGaugeMonitor,
  isTrafficMonitor,
  getMonitorTrafficInData,
  getMonitorTrafficOutData,
  getMonitorInBps,
  getMonitorOutBps,
  gaugeMetricName,
  gaugeDisplayUnit,
  gaugeUsagePercent,
  formatGaugeValue,
  gaugeHexColor,
} from '@/utils/monitorPresentation'

const monitorsStore = useMonitorsStore()
const { detalheAberto, monitorEmDetalhe, abrirDetalhe: abrirMonitor } = useMonitorDetail()

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
