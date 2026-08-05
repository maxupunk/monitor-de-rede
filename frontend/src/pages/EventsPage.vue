<template>
  <div>
    <div class="d-flex align-center justify-space-between mb-6">
      <div>
        <h1 class="text-h4 font-weight-bold">Feed de Eventos em Tempo Real</h1>
        <p class="text-subtitle-1 text-grey-darken-1">
          Fluxo contínuo de eventos transmitidos via Server-Sent Events (SSE)
        </p>
      </div>
      <v-chip :color="eventsStore.isConnected ? 'success' : 'grey'" size="large" variant="tonal">
        <v-icon start>mdi-radiobox-marked</v-icon>
        {{ eventsStore.isConnected ? 'SSE Conectado' : 'SSE Desconectado' }}
      </v-chip>
    </div>

    <!-- Feed de Eventos -->
    <v-card elevation="2" class="rounded-lg">
      <v-card-title class="pa-4 font-weight-bold">Transmissão em Tempo Real</v-card-title>
      <v-divider />
      <v-card-text class="pa-0">
        <v-list lines="two">
          <v-list-item
            v-for="(evt, idx) in eventsStore.recentEvents"
            :key="idx"
            class="px-4 py-3 border-b"
          >
            <template #prepend>
              <v-avatar color="primary" variant="tonal" size="36">
                <v-icon color="primary">mdi-lightning-bolt</v-icon>
              </v-avatar>
            </template>
            <v-list-item-title class="font-weight-bold">
              {{ eventTypeLabel(evt.type) }}
              <span class="text-caption text-grey font-weight-regular ml-2">{{ evt.type }}</span>
            </v-list-item-title>
            <v-list-item-subtitle class="font-mono">
              {{ JSON.stringify(evt.data) }}
            </v-list-item-subtitle>
            <template #append>
              <span class="text-caption text-grey">{{ formatTime(evt.timestamp) }}</span>
            </template>
          </v-list-item>
        </v-list>
        <div v-if="eventsStore.recentEvents.length === 0" class="pa-8 text-center text-grey">
          <v-icon size="48" color="grey-lighten-1" class="mb-2">mdi-pulse</v-icon>
          <div>Nenhum evento transmitido até o momento nesta sessão.</div>
        </div>
      </v-card-text>
    </v-card>
  </div>
</template>

<script setup lang="ts">
import { useEventsStore } from '@/stores/events'

const eventsStore = useEventsStore()

const EVENT_LABELS: Record<string, string> = {
  'monitor:result': 'Verificação de monitor concluída',
  'device:status': 'Status de dispositivo alterado',
  'alert:triggered': 'Alerta disparado',
  'alert:resolved': 'Alerta normalizado',
  'alert:acknowledged': 'Alerta reconhecido',
  'alert:silenced': 'Alerta silenciado',
  'alert_rule:created': 'Regra de alerta criada',
  'alert_rule:updated': 'Regra de alerta atualizada',
  'alert_rule:deleted': 'Regra de alerta removida',
  'probe:status': 'Status de probe alterado',
  'discovery:started': 'Varredura de rede iniciada',
  'discovery:completed': 'Varredura de rede concluída',
  'discovery:failed': 'Varredura de rede falhou',
  'interface:status_change': 'Interface mudou de estado',
  'interface:speed_change': 'Interface renegociou velocidade',
  'interface:speed_downgrade': 'Interface sofreu downgrade de velocidade',
  'device:updated': 'Dispositivo atualizado',
  'metric:recorded': 'Métricas coletadas',
  'vpn:peers_updated': 'Peers de VPN atualizados',
  'topology:updated': 'Topologia atualizada',
}

function eventTypeLabel(type: string): string {
  return EVENT_LABELS[type] ?? type
}

function formatTime(timestamp?: string): string {
  if (!timestamp) return 'agora'
  const date = new Date(timestamp)
  return Number.isNaN(date.getTime()) ? timestamp : date.toLocaleTimeString('pt-BR')
}
</script>
