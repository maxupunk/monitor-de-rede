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
                      <router-link :to="'/monitors/' + monitor.id" class="text-decoration-none">
                        <MonitorTimelineBar
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

    <v-row>
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
                    <v-btn
                      size="small"
                      variant="outlined"
                      color="primary"
                      @click="alertsStore.acknowledgeAlert(alert.id)"
                    >
                      Reconhecer
                    </v-btn>
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
                  class="px-4 py-2 border-b"
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
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useDevicesStore } from '@/stores/devices'
import { useAlertsStore } from '@/stores/alerts'
import { useEventsStore, type RealtimeEventPayload } from '@/stores/events'
import { useMonitorsStore } from '@/stores/monitors'
import MonitorTimelineBar from '@/components/MonitorTimelineBar.vue'
import DnsLatencyCard from '@/components/DnsLatencyCard.vue'

const devicesStore = useDevicesStore()
const alertsStore = useAlertsStore()
const eventsStore = useEventsStore()
const monitorsStore = useMonitorsStore()
const loading = ref(false)

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

const monitorsOnlineCount = computed(() => {
  return monitorsStore.monitors.filter((m) => m.status === 'online' || m.status === 'up').length
})

const globalUptime = computed(() => {
  if (monitorsStore.monitors.length === 0) return 100
  const up = monitorsOnlineCount.value
  return Math.round((up / monitorsStore.monitors.length) * 100)
})

function getStatusColor(status?: string): string {
  switch (status) {
    case 'up':
    case 'online':
      return 'success'
    case 'down':
    case 'offline':
      return 'error'
    case 'warning':
      return 'warning'
    case 'disabled':
      return 'grey'
    default:
      return 'blue-grey'
  }
}

function formatEventDetails(evt: RealtimeEventPayload) {
  const d = (evt.data || {}) as Record<string, any>
  const dateStr = evt.timestamp
    ? new Date(evt.timestamp).toLocaleTimeString('pt-BR', {
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit',
      })
    : new Date().toLocaleTimeString('pt-BR', {
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit',
      })

  switch (evt.type) {
    case 'monitor:result': {
      const name = d.name || d.monitorName || `Monitor #${d.monitorId || d.id || ''}`
      const status = String(d.status || '').toLowerCase()
      const isUp = status === 'up' || status === 'online'
      const latency = d.latencyMs !== undefined && d.latencyMs !== null ? `${d.latencyMs} ms` : null
      return {
        title: name,
        message: `${isUp ? 'Verificação OK (ONLINE)' : 'Falha na Verificação (OFFLINE)'}${latency ? ` • Latência: ${latency}` : ''}`,
        icon: isUp ? 'mdi-check-circle' : 'mdi-alert-circle',
        color: isUp ? 'success' : 'error',
        time: dateStr,
      }
    }
    case 'device:status': {
      const name = d.name || d.deviceName || `Dispositivo #${d.id || ''}`
      const status = String(d.status || '').toLowerCase()
      const isOnline = status === 'online'
      return {
        title: name,
        message: `Status do equipamento alterado para ${status.toUpperCase()}`,
        icon: isOnline ? 'mdi-lan-connect' : 'mdi-lan-disconnect',
        color: isOnline ? 'success' : 'error',
        time: dateStr,
      }
    }
    case 'alert:triggered': {
      return {
        title: d.title || 'Novo Alerta Disparado',
        message: d.message || 'Aviso de incidente registrado no sistema',
        icon: 'mdi-bell-ring',
        color: 'warning',
        time: dateStr,
      }
    }
    case 'alert:resolved': {
      return {
        title: d.title || 'Alerta Normalizado',
        message: d.message || 'A condição que gerou o alerta foi restabelecida',
        icon: 'mdi-check-decagram',
        color: 'success',
        time: dateStr,
      }
    }
    case 'interface:status_change':
    case 'interface:speed_change':
    case 'interface:speed_downgrade': {
      return {
        title: `Interface ${d.ifName || ''}`.trim(),
        message: String(d.message || 'Alteração detectada na interface'),
        icon: 'mdi-ethernet-cable',
        color: evt.type === 'interface:speed_change' ? 'info' : 'warning',
        time: dateStr,
      }
    }
    case 'probe:status': {
      return {
        title: String(d.name || `Probe #${d.id || ''}`),
        message: `Probe passou para o estado ${String(d.status || '').toUpperCase()}`,
        icon: 'mdi-router-wireless',
        color: d.status === 'online' ? 'success' : 'warning',
        time: dateStr,
      }
    }
    default: {
      const summary =
        d.name ||
        d.title ||
        d.message ||
        (Object.keys(d).length ? JSON.stringify(d).slice(0, 60) : 'Evento de sistema')
      return {
        title: evt.type || 'Evento do Sistema',
        message: String(summary),
        icon: 'mdi-pulse',
        color: 'info',
        time: dateStr,
      }
    }
  }
}
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
