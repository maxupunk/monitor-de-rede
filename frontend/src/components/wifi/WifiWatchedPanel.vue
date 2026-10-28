<template>
  <v-card rounded="lg" border>
    <v-card-item>
      <v-card-title class="d-flex align-center ga-2">
        <v-icon color="primary">mdi-eye</v-icon>
        Aparelhos vigiados
      </v-card-title>
      <v-card-subtitle class="text-wrap">
        Câmeras, IoT e o que deveria estar sempre conectado: sumiu do Wi-Fi por 10 minutos, vira
        alerta. Celulares e notebooks ficam de fora.
      </v-card-subtitle>
      <template #append>
        <v-btn color="primary" variant="tonal" prepend-icon="mdi-bell-cog-outline" to="/alerts">
          Alertas
        </v-btn>
      </template>
    </v-card-item>

    <v-card-text>
      <v-alert v-if="store.watched.length === 0" type="info" variant="tonal" density="compact">
        Nenhum aparelho vigiado. Na tabela abaixo, clique em <strong>Vigiar</strong> na linha da
        câmera ou do IoT.
      </v-alert>

      <v-list v-else lines="three" class="pa-0" aria-label="Aparelhos vigiados">
        <v-list-item
          v-for="client in store.watched"
          :key="client.id"
          :title="client.name"
          :subtitle="where(client)"
          class="px-0"
        >
          <template #prepend>
            <v-avatar :color="WATCH_STATE[client.state].color" variant="tonal">
              <v-icon>{{ WATCH_STATE[client.state].icon }}</v-icon>
            </v-avatar>
          </template>
          <template #append>
            <div class="d-flex flex-wrap align-center justify-end ga-1">
              <v-chip
                size="small"
                variant="flat"
                :color="WATCH_STATE[client.state].color"
                class="font-weight-bold d-none d-sm-flex"
              >
                {{ WATCH_STATE[client.state].label }}
              </v-chip>
              <template v-if="auth.canWrite">
                <v-btn
                  icon="mdi-pencil"
                  variant="text"
                  color="primary"
                  size="small"
                  :aria-label="`Renomear ${client.name}`"
                  @click="rename(client)"
                ></v-btn>
                <v-btn
                  icon="mdi-eye-off-outline"
                  variant="text"
                  color="warning"
                  size="small"
                  :aria-label="`Deixar de vigiar ${client.name}`"
                  @click="stopWatching(client)"
                ></v-btn>
              </template>
            </div>
          </template>
        </v-list-item>
      </v-list>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import type { WifiWatchedClient } from '@/bindings/WifiWatchedClient'
import { useWifiWatch } from '@/composables/useWifiWatch'
import { useAuthStore } from '@/stores/auth'
import { useWifiClientsStore } from '@/stores/wifiClients'
import { formatRelativeTime } from '@/utils/formatters'
import { WATCH_STATE } from '@/utils/wifiPresentation'

const store = useWifiClientsStore()
const auth = useAuthStore()
const { rename, stopWatching } = useWifiWatch()

/** Onde está, ou desde quando sumiu — o que o técnico precisa para agir. */
function where(client: WifiWatchedClient): string {
  const parts = [client.mac]
  const router = client.deviceName ? ` em ${client.deviceName}` : ''
  if (client.state === 'connected') {
    parts.push(`conectado${router}, visto ${formatRelativeTime(client.lastSeenAt)}`)
  } else if (client.state === 'offline') {
    parts.push(`visto por último ${formatRelativeTime(client.lastSeenAt)}${router}`)
  } else {
    parts.push('confere na próxima leitura (até 5 min)')
  }
  if (client.randomizedMac) parts.push('MAC privado: pode mudar')
  return parts.join(' · ')
}
</script>
