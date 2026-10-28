<template>
  <div>
    <PageHeader
      title="Sinais Wi-Fi"
      subtitle="Os clientes de todos os roteadores, de quem precisa de atenção para quem está bem"
    >
      <template #actions>
        <WifiRefreshButton :reading="store.reading" @refresh="store.refresh()" />
      </template>
    </PageHeader>

    <v-alert v-if="store.error" type="error" variant="tonal" class="mb-4">
      {{ store.error }}
    </v-alert>

    <v-alert v-if="noWifiDevices" type="info" variant="tonal" class="mb-4">
      <div class="font-weight-bold mb-1">Nenhum roteador com plugin de Wi-Fi ativo</div>
      Os clientes vêm do plugin instalado em cada roteador — no OpenWrt, o aplicativo
      <strong>Rede Wi-Fi OpenWrt</strong>. Cadastre o roteador com o sistema OpenWrt ou instale o
      plugin na aba Plugins do equipamento.
      <template #append>
        <v-btn color="info" variant="flat" to="/plugins">Ver plugins</v-btn>
      </template>
    </v-alert>

    <template v-else>
      <v-row dense class="mb-2">
        <v-col v-for="tile in tiles" :key="tile.title" cols="4">
          <v-card rounded="lg" border class="pa-3 pa-sm-4 h-100">
            <div class="d-flex align-center justify-space-between ga-2">
              <div>
                <div class="text-body-2 font-weight-medium">{{ tile.title }}</div>
                <div :class="`text-h5 text-sm-h4 font-weight-bold text-${tile.color}`">
                  {{ tile.value }}
                </div>
                <div class="text-body-2 d-none d-sm-block">{{ tile.hint }}</div>
              </div>
              <v-avatar :color="tile.color" variant="tonal" size="48" class="d-none d-md-flex">
                <v-icon size="28">{{ tile.icon }}</v-icon>
              </v-avatar>
            </div>
          </v-card>
        </v-col>
      </v-row>

      <section v-if="store.devices.length" class="mb-4" aria-label="Roteadores lidos">
        <div class="d-flex flex-wrap ga-2">
          <v-tooltip
            v-for="device in store.devices"
            :key="device.deviceId"
            :text="deviceTip(device)"
            location="bottom"
          >
            <template #activator="{ props: tip }">
              <v-chip
                v-bind="tip"
                :to="{ path: `/devices/${device.deviceId}`, query: { tab: 'wifi' } }"
                :color="DEVICE_STATUS[device.status].color"
                :prepend-icon="DEVICE_STATUS[device.status].icon"
                variant="tonal"
              >
                {{ device.deviceName }} · {{ device.stations.length }}
              </v-chip>
            </template>
          </v-tooltip>
        </div>
      </section>

      <WifiWatchedPanel class="mb-4" />

      <v-card rounded="lg" border>
        <v-card-text class="d-flex flex-column ga-4">
          <WifiStationFilters v-model="filter" :counts="counts" :devices="deviceOptions" />
          <WifiStationsTable
            :rows="filtered"
            show-device
            :loading="store.devices.length === 0 && store.requesting"
            :no-data-text="emptyText"
          />
        </v-card-text>
      </v-card>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import PageHeader from '@/components/PageHeader.vue'
import WifiRefreshButton from '@/components/wifi/WifiRefreshButton.vue'
import WifiStationFilters from '@/components/wifi/WifiStationFilters.vue'
import WifiStationsTable from '@/components/wifi/WifiStationsTable.vue'
import WifiWatchedPanel from '@/components/wifi/WifiWatchedPanel.vue'
import type { WifiDeviceSnapshot } from '@/bindings/WifiDeviceSnapshot'
import type { WifiReadStatus } from '@/bindings/WifiReadStatus'
import { useAuthStore } from '@/stores/auth'
import { useWifiClientsStore } from '@/stores/wifiClients'
import { formatRelativeTime } from '@/utils/formatters'
import { countStations, emptyFilter, filterStations } from '@/utils/wifiPresentation'

const store = useWifiClientsStore()
const auth = useAuthStore()
const filter = ref(emptyFilter())

const DEVICE_STATUS: Record<WifiReadStatus, { color: string; icon: string }> = {
  reading: { color: 'primary', icon: 'mdi-progress-clock' },
  ok: { color: 'success', icon: 'mdi-router-wireless' },
  failed: { color: 'error', icon: 'mdi-alert-circle' },
}

const counts = computed(() => countStations(store.rows))
const filtered = computed(() => filterStations(store.rows, filter.value))

const noWifiDevices = computed(
  () => store.devices.length === 0 && store.lastFleetRefresh?.deviceIds.length === 0
)

const tiles = computed(() => {
  const routers = store.devices.length
  return [
    {
      title: 'Clientes conectados',
      value: counts.value.all,
      hint: routers === 1 ? 'em 1 roteador' : `em ${routers} roteadores`,
      color: 'primary',
      icon: 'mdi-devices',
    },
    {
      title: 'Precisam de atenção',
      value: counts.value.attention,
      hint: 'sinal fraco, ruído ou interferência',
      color: counts.value.attention > 0 ? 'error' : 'success',
      icon: 'mdi-wifi-alert',
    },
    {
      title: 'Em repouso',
      value: counts.value.powerSave,
      hint: 'parados com sinal bom — não é problema',
      color: 'info',
      icon: 'mdi-sleep',
    },
  ]
})

const deviceOptions = computed(() => [
  { title: 'Todos os roteadores', value: null },
  ...store.devices.map((device) => ({ title: device.deviceName, value: device.deviceId })),
])

const emptyText = computed(() => {
  if (store.devices.length === 0) return 'Ainda não houve leitura. Clique em "Ler agora".'
  if (store.rows.length === 0) return 'Nenhum cliente conectado agora.'
  return 'Nenhum cliente com esses filtros.'
})

function deviceTip(device: WifiDeviceSnapshot): string {
  if (device.status === 'reading') return 'Lendo agora…'
  if (device.status === 'failed') return `Falhou: ${device.error ?? 'sem detalhe'}`
  return `Lido ${formatRelativeTime(device.readAt)}`
}

// Sem leitura guardada (servidor recém-iniciado): uma leitura, pedida uma vez
// ao abrir — o resto chega pelo SSE.
onMounted(() => {
  if (store.devices.length === 0 && auth.canWrite) store.refresh()
})
</script>
