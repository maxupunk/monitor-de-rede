<template>
  <v-row dense>
    <v-col v-for="tile in tiles" :key="tile.label" cols="12" sm="6" lg="3">
      <v-card
        variant="flat"
        rounded="lg"
        class="stat-tile dashboard-surface"
        :to="dashboardStore.isEditMode ? undefined : tile.to"
      >
        <div class="d-flex align-center justify-space-between ga-3">
          <span class="stat-tile__label">{{ tile.label }}</span>
          <v-avatar :color="tile.color" variant="tonal" size="36" rounded="lg">
            <v-icon size="20">{{ tile.icon }}</v-icon>
          </v-avatar>
        </div>
        <div class="stat-tile__value" :class="`text-${tile.valueColor}`">{{ tile.value }}</div>
        <div class="stat-tile__detail" :class="`text-${tile.detailColor}`">{{ tile.detail }}</div>
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

interface StatTile {
  label: string
  icon: string
  color: string
  value: string
  valueColor: string
  detail: string
  detailColor: string
  to: RouteLocationRaw
}

const dashboardStore = useDashboardStore()
const devicesStore = useDevicesStore()
const alertsStore = useAlertsStore()
const monitorsStore = useMonitorsStore()

const healthCounts = computed(() => monitorHealthCounts(monitorsStore.monitors))
const globalUptime = computed(() => monitorUptimePercent(healthCounts.value))

/**
 * Quatro números que respondem "está tudo bem?". A cor do valor só muda
 * quando há algo a fazer — verde/primário é calma, laranja/vermelho é ação.
 */
const tiles = computed<StatTile[]>(() => {
  const offline = devicesStore.offlineCount
  const down = healthCounts.value.monitored - healthCounts.value.up
  const alerts = alertsStore.activeAlerts.length
  return [
    {
      label: 'Dispositivos',
      icon: 'mdi-devices',
      color: 'primary',
      value: String(devicesStore.totalCount),
      valueColor: 'high-emphasis',
      detail: `${devicesStore.onlineCount} online · ${offline} offline`,
      detailColor: offline > 0 ? 'error' : 'success',
      to: '/devices',
    },
    {
      label: 'Monitores',
      icon: 'mdi-heart-pulse',
      color: 'info',
      value: String(monitorsStore.monitors.length),
      valueColor: 'high-emphasis',
      detail:
        `${healthCounts.value.up} operacionais` +
        (healthCounts.value.disabled > 0 ? ` · ${healthCounts.value.disabled} desativados` : ''),
      detailColor: 'info',
      to: '/monitors',
    },
    {
      label: 'Disponibilidade',
      icon: 'mdi-check-circle-outline',
      color: down > 0 ? 'warning' : 'success',
      value: `${globalUptime.value}%`,
      valueColor: down > 0 ? 'warning' : 'success',
      detail:
        down > 0
          ? `${down} de ${healthCounts.value.monitored} monitores fora do ar`
          : `${healthCounts.value.monitored} monitores no ar`,
      detailColor: down > 0 ? 'warning' : 'success',
      to: { path: '/monitors', query: { status: 'down' } },
    },
    {
      label: 'Alertas ativos',
      icon: 'mdi-bell-ring-outline',
      color: alerts > 0 ? 'error' : 'success',
      value: String(alerts),
      valueColor: alerts > 0 ? 'error' : 'success',
      detail: alerts > 0 ? 'Requerem atenção' : 'Nenhum alerta aberto',
      detailColor: alerts > 0 ? 'error' : 'success',
      to: '/alerts',
    },
  ]
})
</script>

<style scoped>
.stat-tile {
  height: 100%;
  padding: 16px 20px;
  transition: transform 0.15s ease;
}

.stat-tile:hover {
  transform: translateY(-2px);
}

.stat-tile__label {
  font-size: 0.85rem;
  font-weight: 700;
}

.stat-tile__value {
  margin-top: 4px;
  font-size: 2rem;
  font-weight: 700;
  line-height: 1.2;
  font-variant-numeric: tabular-nums;
}

.stat-tile__detail {
  margin-top: 2px;
  font-size: 0.8rem;
  font-weight: 500;
}
</style>
