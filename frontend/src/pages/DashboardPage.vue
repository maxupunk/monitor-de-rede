<template>
  <div>
    <div class="d-flex align-center justify-space-between mb-6 flex-wrap ga-4">
      <div>
        <h1 class="text-h4 font-weight-bold">Dashboard</h1>
        <p class="text-subtitle-1 text-grey-darken-1">
          Visão geral do monitoramento e status em tempo real
        </p>
      </div>
      <div class="d-flex align-center ga-3">
        <v-chip
          :color="eventsStore.isConnected ? 'success' : 'warning'"
          variant="tonal"
          size="small"
          class="font-weight-medium"
        >
          <v-icon start size="12" :color="eventsStore.isConnected ? 'success' : 'warning'">
            mdi-circle
          </v-icon>
          {{ eventsStore.isConnected ? 'SSE Conectado' : 'SSE Reconectando...' }}
        </v-chip>
        <v-btn color="primary" prepend-icon="mdi-refresh" :loading="loading" @click="refreshData">
          Atualizar Dados
        </v-btn>
      </div>
    </div>

    <v-row class="mb-6">
      <v-col cols="12" sm="6" md="3">
        <v-card elevation="2" class="pa-4 rounded-lg">
          <div class="d-flex align-center justify-space-between mb-2">
            <span class="text-subtitle-2 text-grey-darken-1 font-weight-bold">Dispositivos</span>
            <v-avatar color="primary" variant="tonal" size="36">
              <v-icon color="primary">mdi-devices</v-icon>
            </v-avatar>
          </div>
          <div class="text-h4 font-weight-bold">{{ devicesStore.totalCount }}</div>
          <div class="text-caption text-success font-weight-medium mt-1">
            {{ devicesStore.onlineCount }} online / {{ devicesStore.offlineCount }} offline
          </div>
        </v-card>
      </v-col>

      <v-col cols="12" sm="6" md="3">
        <v-card elevation="2" class="pa-4 rounded-lg">
          <div class="d-flex align-center justify-space-between mb-2">
            <span class="text-subtitle-2 text-grey-darken-1 font-weight-bold"
            >Monitores de Rede</span
            >
            <v-avatar color="info" variant="tonal" size="36">
              <v-icon color="info">mdi-chart-timeline-variant</v-icon>
            </v-avatar>
          </div>
          <div class="text-h4 font-weight-bold">{{ monitorsStore.monitors.length }}</div>
          <div class="text-caption text-info font-weight-medium mt-1">
            {{ monitorsOnlineCount }} operacionais
          </div>
        </v-card>
      </v-col>

      <v-col cols="12" sm="6" md="3">
        <v-card elevation="2" class="pa-4 rounded-lg">
          <div class="d-flex align-center justify-space-between mb-2">
            <span class="text-subtitle-2 text-grey-darken-1 font-weight-bold">Disponibilidade</span>
            <v-avatar color="success" variant="tonal" size="36">
              <v-icon color="success">mdi-check-circle-outline</v-icon>
            </v-avatar>
          </div>
          <div class="text-h4 font-weight-bold text-success">{{ globalUptime }}%</div>
          <div class="text-caption text-grey mt-1">Taxa de sucesso nas checagens</div>
        </v-card>
      </v-col>

      <v-col cols="12" sm="6" md="3">
        <v-card elevation="2" class="pa-4 rounded-lg">
          <div class="d-flex align-center justify-space-between mb-2">
            <span class="text-subtitle-2 text-grey-darken-1 font-weight-bold">Alertas Ativos</span>
            <v-avatar color="warning" variant="tonal" size="36">
              <v-icon color="warning">mdi-bell-ring-outline</v-icon>
            </v-avatar>
          </div>
          <div class="text-h4 font-weight-bold text-warning">
            {{ alertsStore.activeAlerts.length }}
          </div>
          <div class="text-caption text-warning mt-1">Requerem atenção</div>
        </v-card>
      </v-col>
    </v-row>

    <v-row class="mb-6">
      <v-col cols="12" md="6">
        <v-card elevation="2" class="rounded-lg fill-height">
          <v-card-title class="d-flex align-center py-3 px-4">
            <v-icon start color="warning">mdi-bell-outline</v-icon>
            <span class="font-weight-bold text-h6">Alertas Críticos Ativos</span>
          </v-card-title>
          <v-divider></v-divider>
          <v-card-text class="pa-0">
            <div v-if="alertsStore.activeAlerts.length > 0">
              <v-list lines="two">
                <v-list-item
                  v-for="alert in alertsStore.activeAlerts.slice(0, 5)"
                  :key="alert.id"
                  :title="alert.title"
                  :subtitle="alert.message"
                  :to="getAlertLink(alert)"
                  class="px-4 py-2 border-b"
                >
                  <template #prepend>
                    <v-avatar
                      :color="
                        alert.severity === 'critical' || alert.severity === 'error'
                          ? 'error'
                          : 'warning'
                      "
                      size="36"
                    >
                      <v-icon color="white">mdi-alert</v-icon>
                    </v-avatar>
                  </template>
                  <template #append>
                    <div class="d-flex align-center ga-2">
                      <v-chip variant="outlined" size="x-small" class="font-weight-medium">
                        {{ statusLabel(alert.status) }}
                      </v-chip>
                      <v-tooltip text="Reconhecer alerta">
                        <template #activator="{ props: tooltipProps }">
                          <v-btn
                            v-bind="tooltipProps"
                            icon
                            size="small"
                            variant="text"
                            color="primary"
                            :disabled="alert.status === 'acknowledged'"
                            @click.stop="alertsStore.acknowledgeAlert(alert.id)"
                          >
                            <v-icon>mdi-check-circle-outline</v-icon>
                          </v-btn>
                        </template>
                      </v-tooltip>
                      <v-tooltip text="Silenciar alerta">
                        <template #activator="{ props: tooltipProps }">
                          <v-btn
                            v-bind="tooltipProps"
                            icon
                            size="small"
                            variant="text"
                            color="warning"
                            :disabled="alert.status === 'silenced'"
                            @click.stop="openSilenceDialog(alert.id)"
                          >
                            <v-icon>mdi-bell-off-outline</v-icon>
                          </v-btn>
                        </template>
                      </v-tooltip>
                    </div>
                  </template>
                </v-list-item>
              </v-list>
            </div>
            <div v-else class="pa-6 text-center text-grey">
              <v-icon size="44" color="success" class="mb-2">mdi-check-all</v-icon>
              <div class="text-subtitle-2 font-weight-medium">Nenhum alerta ativo!</div>
              <div class="text-caption">Todos os sistemas estão funcionando normalmente.</div>
            </div>
          </v-card-text>
        </v-card>
      </v-col>

      <v-col cols="12" md="6">
        <v-card elevation="2" class="rounded-lg fill-height">
          <v-card-title class="d-flex align-center justify-space-between py-3 px-4">
            <div class="d-flex align-center">
              <v-icon start color="info">mdi-pulse</v-icon>
              <span class="font-weight-bold text-h6">Feed de Eventos em Tempo Real</span>
            </div>
            <v-chip
              :color="eventsStore.isConnected ? 'success' : 'error'"
              size="x-small"
              variant="flat"
            >
              {{ eventsStore.isConnected ? 'Ao Vivo' : 'Desconectado' }}
            </v-chip>
          </v-card-title>
          <v-divider></v-divider>
          <v-card-text class="pa-0">
            <div v-if="eventsStore.recentEvents.length > 0">
              <v-list max-height="360" class="overflow-y-auto pa-0">
                <v-list-item
                  v-for="(evt, idx) in eventsStore.recentEvents.slice(0, 10)"
                  :key="idx"
                  :title="formatEventDetails(evt).title"
                  :subtitle="formatEventDetails(evt).message"
                  class="px-4 py-2 border-b cursor-pointer"
                  @click="openEventDetail(evt)"
                >
                  <template #prepend>
                    <v-avatar
                      :color="formatEventDetails(evt).color"
                      size="32"
                      variant="tonal"
                      class="mr-3"
                    >
                      <v-icon size="18">{{ formatEventDetails(evt).icon }}</v-icon>
                    </v-avatar>
                  </template>
                  <template #append>
                    <span class="text-caption text-grey">{{ formatEventDetails(evt).time }}</span>
                  </template>
                </v-list-item>
              </v-list>
            </div>
            <div v-else class="pa-6 text-center text-grey">
              <v-icon size="44" color="grey-lighten-1" class="mb-2">
                mdi-access-point-network
              </v-icon>
              <div class="text-subtitle-2 font-weight-medium">
                Aguardando eventos em tempo real...
              </div>
              <div class="text-caption">
                Mudanças de status e verificações aparecerão aqui automaticamente.
              </div>
            </div>
          </v-card-text>
        </v-card>
      </v-col>
    </v-row>

    <v-row class="mb-6">
      <v-col cols="12">
        <v-card elevation="2" class="rounded-lg">
          <v-card-title class="d-flex align-center justify-space-between py-3 px-4 flex-wrap ga-2">
            <div class="d-flex align-center">
              <v-icon start color="primary">mdi-chart-timeline-variant</v-icon>
              <span class="font-weight-bold text-h6">Monitores de Rede</span>
              <v-chip size="x-small" color="primary" class="ml-2" variant="tonal">
                {{ monitorsStore.monitors.length }}
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
            <div v-if="monitorsStore.monitors.length > 0">
              <v-list>
                <v-list-item
                  v-for="monitor in monitorsStore.monitors"
                  :key="monitor.id"
                  :title="undefined"
                  class="px-4 py-3 border-b"
                >
                  <div class="d-flex align-center justify-space-between flex-wrap ga-3 w-100">
                    <div
                      class="monitor-info d-flex align-center ga-3"
                      style="min-width: 220px; flex: 1"
                    >
                      <v-avatar
                        :color="getStatusColor(monitor.status)"
                        size="10"
                        class="mr-1"
                      ></v-avatar>
                      <div>
                        <router-link
                          :to="'/monitors/' + monitor.id"
                          class="text-subtitle-1 font-weight-bold text-decoration-none text-primary hover-underline d-block"
                        >
                          {{ monitor.name }}
                        </router-link>

                        <div class="d-flex align-center ga-2 mt-1">
                          <v-chip size="x-small" color="info" variant="tonal">
                            {{ (monitor.type || 'N/A').toUpperCase() }}
                          </v-chip>
                          <span class="text-caption text-grey-darken-1">{{ monitor.target }}</span>
                        </div>
                      </div>
                    </div>

                    <div
                      class="monitor-timeline d-flex align-center justify-center"
                      style="flex: 2; min-width: 280px"
                    >
                      <router-link
                        :to="'/monitors/' + monitor.id"
                        class="text-decoration-none d-flex align-center ga-2"
                      >
                        <template v-if="isGaugeMonitor(monitor)">
                          <!-- Largura igual à da MonitorTimelineBar abaixo (24 blocos de 5px + 23 gaps de 3px = 189px),
                               para os dois estilos de linha ficarem visualmente alinhados na mesma coluna. -->
                          <MonitorSparkline
                            :data="monitor.gaugeHistory || []"
                            :color="gaugeSparklineColor(monitor)"
                            :width="189"
                            :height="28"
                          />
                          <span class="text-caption font-weight-medium text-high-emphasis">
                            {{
                              monitor.gaugeMetric
                                ? `${Math.round(monitor.gaugeMetric.value)}%`
                                : 'N/D'
                            }}
                          </span>
                        </template>
                        <MonitorTimelineBar
                          v-else
                          :results="monitor.recentResults"
                          :max-blocks="24"
                          :height="20"
                          :width="5"
                        ></MonitorTimelineBar>
                      </router-link>
                    </div>

                    <div
                      class="monitor-actions d-flex align-center ga-2 justify-end"
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
                        @click="monitorsStore.runMonitor(monitor.id)"
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
              <div class="text-subtitle-1 font-weight-medium">Nenhum monitor cadastrado</div>
              <p class="text-caption text-grey-darken-1 mb-4">
                Cadastre monitores ICMP, HTTP ou TCP para visualizar os gráficos em barras.
              </p>
              <v-btn color="primary" prepend-icon="mdi-plus" to="/monitors">
                Cadastrar Monitor
              </v-btn>
            </div>
          </v-card-text>
        </v-card>
      </v-col>
    </v-row>

    <v-row class="mb-6">
      <v-col cols="12">
        <DnsLatencyCard></DnsLatencyCard>
      </v-col>
    </v-row>

    <!-- Modal Silenciar Alerta -->
    <AlertSilenceDialog v-model="silenceDialog" :alert-id="silenceTargetId" />

    <!-- Modal Detalhes do Evento -->
    <EventDetailDialog v-model="eventDetailDialog" :event="selectedEventPayload" />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useDevicesStore } from '@/stores/devices'
import { useAlertsStore } from '@/stores/alerts'
import { useEventsStore, type RealtimeEventPayload } from '@/stores/events'
import { useMonitorsStore } from '@/stores/monitors'
import MonitorTimelineBar from '@/components/MonitorTimelineBar.vue'
import MonitorSparkline from '@/components/MonitorSparkline.vue'
import DnsLatencyCard from '@/components/DnsLatencyCard.vue'
import AlertSilenceDialog from '@/components/AlertSilenceDialog.vue'
import EventDetailDialog from '@/components/EventDetailDialog.vue'
import { statusLabel } from '@/utils/alertPresentation'
import { formatEventDetails } from '@/utils/eventPresentation'
import {
  getStatusColor,
  isGaugeMonitor,
  gaugeMetricName,
  gaugeHexColor,
} from '@/utils/monitorPresentation'
import type { Monitor } from '@/stores/monitors'

const devicesStore = useDevicesStore()
const alertsStore = useAlertsStore()
const eventsStore = useEventsStore()
const monitorsStore = useMonitorsStore()
const loading = ref(false)

const silenceDialog = ref(false)
const silenceTargetId = ref<number | null>(null)

const eventDetailDialog = ref(false)
const selectedEventPayload = ref<RealtimeEventPayload | null>(null)

function openSilenceDialog(id: number) {
  silenceTargetId.value = id
  silenceDialog.value = true
}

function openEventDetail(evt: RealtimeEventPayload) {
  selectedEventPayload.value = evt
  eventDetailDialog.value = true
}

function getAlertLink(alert: { monitorId?: number | null; deviceId?: number | null }): string {
  if (alert.monitorId) return '/monitors/' + alert.monitorId
  if (alert.deviceId) return '/devices/' + alert.deviceId
  return '/alerts'
}

onMounted(async () => {
  await refreshData()
})

async function refreshData() {
  loading.value = true
  await Promise.all([
    devicesStore.fetchDevices(),
    alertsStore.fetchActiveAlerts(),
    monitorsStore.fetchMonitors(),
  ])
  loading.value = false
}

function gaugeSparklineColor(monitor: Monitor): string {
  return gaugeHexColor(monitor.gaugeMetric?.value ?? null, gaugeMetricName(monitor))
}

const monitorsOnlineCount = computed(() => {
  return monitorsStore.monitors.filter((m) => m.status === 'online' || m.status === 'up').length
})

const globalUptime = computed(() => {
  if (monitorsStore.monitors.length === 0) return 100
  const up = monitorsOnlineCount.value
  return Math.round((up / monitorsStore.monitors.length) * 100)
})
</script>

<style scoped>
.hover-underline:hover {
  text-decoration: underline !important;
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
.ga-4 {
  gap: 16px;
}
</style>
