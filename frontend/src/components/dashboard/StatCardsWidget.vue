<template>
  <v-row>
    <v-col cols="12" sm="6" md="3">
      <v-card elevation="2" class="pa-4 rounded-lg stat-card" :to="statCardLink('/devices')">
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
      <v-card elevation="2" class="pa-4 rounded-lg stat-card" :to="statCardLink('/monitors')">
        <div class="d-flex align-center justify-space-between mb-2">
          <span class="text-subtitle-2 text-grey-darken-1 font-weight-bold">Monitores de Rede</span>
          <v-avatar color="info" variant="tonal" size="36">
            <v-icon color="info">mdi-chart-timeline-variant</v-icon>
          </v-avatar>
        </div>
        <div class="text-h4 font-weight-bold">{{ monitorsStore.monitors.length }}</div>
        <div class="text-caption text-info font-weight-medium mt-1">
          {{ healthCounts.up }} operacionais
          <template v-if="healthCounts.disabled > 0">
            · {{ healthCounts.disabled }} desativado(s)
          </template>
        </div>
      </v-card>
    </v-col>

    <v-col cols="12" sm="6" md="3">
      <v-card
        elevation="2"
        class="pa-4 rounded-lg stat-card"
        :to="statCardLink({ path: '/monitors', query: { status: 'down' } })"
      >
        <div class="d-flex align-center justify-space-between mb-2">
          <span class="text-subtitle-2 text-grey-darken-1 font-weight-bold">Disponibilidade</span>
          <v-avatar color="success" variant="tonal" size="36">
            <v-icon color="success">mdi-check-circle-outline</v-icon>
          </v-avatar>
        </div>
        <div class="text-h4 font-weight-bold text-success">{{ globalUptime }}%</div>
        <div class="text-caption text-grey mt-1">
          {{ healthCounts.up }} de {{ healthCounts.monitored }} monitor(es) ativo(s) no ar
        </div>
      </v-card>
    </v-col>

    <v-col cols="12" sm="6" md="3">
      <v-card elevation="2" class="pa-4 rounded-lg stat-card" :to="statCardLink('/alerts')">
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
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { type RouteLocationRaw } from 'vue-router'
import { useDashboardStore } from '@/stores/dashboard'
import { useDevicesStore } from '@/stores/devices'
import { useAlertsStore } from '@/stores/alerts'
import { useMonitorsStore } from '@/stores/monitors'
import { monitorHealthCounts, monitorUptimePercent } from '@/utils/monitorPresentation'

const dashboardStore = useDashboardStore()
const devicesStore = useDevicesStore()
const alertsStore = useAlertsStore()
const monitorsStore = useMonitorsStore()

const healthCounts = computed(() => monitorHealthCounts(monitorsStore.monitors))
const globalUptime = computed(() => monitorUptimePercent(healthCounts.value))

function statCardLink(target: string | RouteLocationRaw): RouteLocationRaw | undefined {
  return dashboardStore.isEditMode ? undefined : target
}
</script>

<style scoped>
.stat-card {
  transition:
    transform 0.15s ease,
    box-shadow 0.15s ease;
}

.stat-card:hover {
  transform: translateY(-2px);
}
</style>
