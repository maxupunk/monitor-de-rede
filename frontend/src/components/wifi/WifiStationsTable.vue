<template>
  <ResponsiveDataTable
    :headers="headers"
    :items="rows"
    :loading="loading"
    :no-data-text="noDataText"
    :items-per-page="50"
    :item-key="rowKey"
    class="wifi-stations"
  >
    <template #[`item.name`]="{ item }">
      <WifiStationIdentity :row="item" />
    </template>

    <template #[`item.connection`]="{ item }">
      <WifiStationNetwork :row="item" :show-device="showDevice" />
    </template>

    <template #[`item.signalDbm`]="{ item }">
      <WifiStationSignal :row="item" />
    </template>

    <template #[`item.health`]="{ item }">
      <WifiStationHealth :row="item" />
    </template>

    <template #[`item.txRateBps`]="{ item }">
      <WifiStationRates :row="item" />
    </template>

    <template #[`item.latency`]="{ item }">
      <div class="d-flex align-center ga-2 text-no-wrap">
        <WifiPingResult :row="item" />
        <WifiPingButton :row="item" compact />
      </div>
    </template>

    <template #[`item.watch`]="{ item }">
      <WifiWatchButton :row="item" />
    </template>

    <template #mobile-item="{ item }">
      <WifiStationCard :row="item" :show-device="showDevice" />
    </template>
  </ResponsiveDataTable>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import ResponsiveDataTable from '@/components/ResponsiveDataTable.vue'
import WifiPingButton from './WifiPingButton.vue'
import WifiPingResult from './WifiPingResult.vue'
import WifiStationCard from './WifiStationCard.vue'
import WifiStationHealth from './WifiStationHealth.vue'
import WifiStationIdentity from './WifiStationIdentity.vue'
import WifiStationNetwork from './WifiStationNetwork.vue'
import WifiStationRates from './WifiStationRates.vue'
import WifiStationSignal from './WifiStationSignal.vue'
import WifiWatchButton from './WifiWatchButton.vue'
import type { WifiStationRow } from '@/utils/wifiPresentation'

const props = defineProps<{
  rows: WifiStationRow[]
  /** O roteador de cada cliente — no painel geral, não na aba do equipamento. */
  showDevice?: boolean
  loading?: boolean
  noDataText: string
}>()

// Sete colunas, na ordem em que o técnico lê: quem é, onde está, como está,
// e o que dá para fazer com ele.
const headers = computed(() => [
  { title: 'Cliente', key: 'name', sortable: false, width: '24%' },
  {
    title: props.showDevice ? 'Conexão' : 'Rede',
    key: 'connection',
    value: props.showDevice ? 'deviceName' : 'ssid',
    width: '16%',
  },
  { title: 'Sinal', key: 'signalDbm' },
  { title: 'Estado', key: 'health', sortable: false, width: '22%' },
  { title: 'Velocidade', key: 'txRateBps' },
  { title: 'Latência', key: 'latency', sortable: false },
  { title: 'Vigiar', key: 'watch', sortable: false, align: 'end' as const },
])

function rowKey(row: WifiStationRow): string {
  return `${row.deviceId}:${row.mac}`
}
</script>

<style scoped>
/* Linhas altas leem melhor com o conteúdo no topo, alinhado ao nome. */
.wifi-stations :deep(tbody td) {
  vertical-align: top;
  padding-top: 12px !important;
  padding-bottom: 12px !important;
}
</style>
