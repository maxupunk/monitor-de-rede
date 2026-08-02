<template>
  <div>
    <div class="d-flex align-center justify-space-between mb-6">
      <div>
        <h1 class="text-h4 font-weight-bold">Dashboard</h1>
        <p class="text-subtitle-1 text-grey-darken-1">Visão geral do monitoramento da infraestrutura</p>
      </div>
      <v-btn color="primary" prepend-icon="mdi-refresh" :loading="loading" @click="refreshData">
        Atualizar Dados
      </v-btn>
    </div>

    <!-- Cards de Métricas Principais -->
    <v-row class="mb-6">
      <v-col cols="12" sm="6" md="3">
        <v-card elevation="2" class="pa-4 rounded-lg">
          <div class="d-flex align-center justify-space-between mb-2">
            <span class="text-subtitle-2 text-grey-darken-1 font-weight-bold">Total Dispositivos</span>
            <v-avatar color="primary" variant="tonal" size="36">
              <v-icon color="primary">mdi-devices</v-icon>
            </v-avatar>
          </div>
          <div class="text-h3 font-weight-bold">{{ devicesStore.totalCount }}</div>
          <div class="text-caption text-grey mt-1">Equipamentos cadastrados</div>
        </v-card>
      </v-col>

      <v-col cols="12" sm="6" md="3">
        <v-card elevation="2" class="pa-4 rounded-lg">
          <div class="d-flex align-center justify-space-between mb-2">
            <span class="text-subtitle-2 text-grey-darken-1 font-weight-bold">Dispositivos Online</span>
            <v-avatar color="success" variant="tonal" size="36">
              <v-icon color="success">mdi-check-circle-outline</v-icon>
            </v-avatar>
          </div>
          <div class="text-h3 font-weight-bold text-success">{{ devicesStore.onlineCount }}</div>
          <div class="text-caption text-success mt-1">Disponibilidade OK</div>
        </v-card>
      </v-col>

      <v-col cols="12" sm="6" md="3">
        <v-card elevation="2" class="pa-4 rounded-lg">
          <div class="d-flex align-center justify-space-between mb-2">
            <span class="text-subtitle-2 text-grey-darken-1 font-weight-bold">Dispositivos Offline</span>
            <v-avatar color="error" variant="tonal" size="36">
              <v-icon color="error">mdi-alert-circle-outline</v-icon>
            </v-avatar>
          </div>
          <div class="text-h3 font-weight-bold text-error">{{ devicesStore.offlineCount }}</div>
          <div class="text-caption text-error mt-1">Sem resposta ICMP/TCP</div>
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
          <div class="text-h3 font-weight-bold text-warning">{{ alertsStore.activeAlerts.length }}</div>
          <div class="text-caption text-warning mt-1">Requerem atenção</div>
        </v-card>
      </v-col>
    </v-row>

    <!-- Grid Inferior: Alertas Recentes & Eventos em Tempo Real -->
    <v-row>
      <v-col cols="12" md="7">
        <v-card elevation="2" class="rounded-lg">
          <v-card-title class="d-flex align-center py-3 px-4">
            <v-icon start color="warning">mdi-bell-outline</v-icon>
            Alertas Críticos Ativos
          </v-card-title>
          <v-divider></v-divider>
          <v-card-text class="pa-0">
            <v-list v-if="alertsStore.activeAlerts.length > 0" lines="two">
              <v-list-item
                v-for="alert in alertsStore.activeAlerts.slice(0, 5)"
                :key="alert.id"
                class="px-4 py-2 border-b"
              >
                <template #prepend>
                  <v-avatar :color="alert.severity === 'critical' || alert.severity === 'error' ? 'error' : 'warning'" size="36">
                    <v-icon color="white">mdi-alert</v-icon>
                  </v-avatar>
                </template>
                <v-list-item-title class="font-weight-bold">{{ alert.title }}</v-list-item-title>
                <v-list-item-subtitle>{{ alert.message }}</v-list-item-subtitle>
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
            <div v-else class="pa-6 text-center text-grey">
              <v-icon size="48" color="success" class="mb-2">mdi-check-all</v-icon>
              <div>Nenhum alerta ativo no momento! Todos os sistemas operacionais.</div>
            </div>
          </v-card-text>
        </v-card>
      </v-col>

      <v-col cols="12" md="5">
        <v-card elevation="2" class="rounded-lg">
          <v-card-title class="d-flex align-center py-3 px-4">
            <v-icon start color="info">mdi-pulse</v-icon>
            Feed de Eventos (Tempo Real SSE)
          </v-card-title>
          <v-divider></v-divider>
          <v-card-text class="pa-0">
            <v-list v-if="eventsStore.recentEvents.length > 0" lines="one" max-height="340" class="overflow-y-auto">
              <v-list-item
                v-for="(evt, idx) in eventsStore.recentEvents.slice(0, 10)"
                :key="idx"
                class="px-4 py-2 border-b"
              >
                <template #prepend>
                  <v-icon color="primary" size="20">mdi-lightning-bolt-outline</v-icon>
                </template>
                <v-list-item-title class="font-weight-bold text-caption">
                  {{ evt.event }}
                </v-list-item-title>
                <v-list-item-subtitle class="text-caption">
                  {{ JSON.stringify(evt.data) }}
                </v-list-item-subtitle>
              </v-list-item>
            </v-list>
            <div v-else class="pa-6 text-center text-grey">
              <v-icon size="48" color="grey-lighten-1" class="mb-2">mdi-access-point-network-off</v-icon>
              <div>Aguardando eventos em tempo real do servidor...</div>
            </div>
          </v-card-text>
        </v-card>
      </v-col>
    </v-row>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useDevicesStore } from '@/stores/devices'
import { useAlertsStore } from '@/stores/alerts'
import { useEventsStore } from '@/stores/events'

const devicesStore = useDevicesStore()
const alertsStore = useAlertsStore()
const eventsStore = useEventsStore()
const loading = ref(false)

onMounted(async () => {
  await refreshData()
})

async function refreshData() {
  loading.value = true
  await Promise.all([
    devicesStore.fetchDevices(),
    alertsStore.fetchActiveAlerts(),
  ])
  loading.value = false
}
</script>
