<template>
  <div v-if="vpnPeer">
    <v-alert
      v-if="vpnNeedsFirewallHint"
      type="warning"
      variant="tonal"
      class="mb-6"
      density="comfortable"
    >
      <div class="font-weight-bold mb-1">
        Túnel conectado, mas o dispositivo não responde a ping.
      </div>
      <div class="text-body-2 mb-2">
        Provavelmente falta liberar o tráfego na interface WireGuard.
      </div>
      <v-btn size="small" color="warning" variant="flat" @click="emit('showVpnFirewallHints')">
        Copiar regras de firewall
      </v-btn>
    </v-alert>

    <v-row class="mb-2">
      <v-col cols="12" md="6">
        <v-list border class="rounded-lg">
          <v-list-item title="Perfil do equipamento">
            <template #subtitle>
              <v-chip size="small" variant="tonal" class="mt-1">
                <v-icon start size="14">{{ vpnProfileIconValue }}</v-icon>
                {{ vpnProfileLabelValue }}
              </v-chip>
            </template>
          </v-list-item>
          <v-list-item title="Status do túnel">
            <template #subtitle>
              <v-chip :color="vpnStatusColorValue" size="small" variant="flat" class="mt-1">
                {{ vpnStatusLabelValue }}
              </v-chip>
            </template>
          </v-list-item>
          <v-list-item
            title="Endereço na VPN"
            :subtitle="detailStore.device?.ipAddress || 'Não informado'"
          ></v-list-item>
          <v-list-item title="Último handshake" :subtitle="vpnLastHandshakeText"></v-list-item>
        </v-list>
      </v-col>
      <v-col cols="12" md="6">
        <v-list border class="rounded-lg">
          <v-list-item
            title="Keepalive persistente"
            :subtitle="`${vpnPeer.persistentKeepalive}s`"
          ></v-list-item>
          <v-list-item
            title="Chave pública do peer"
            :subtitle="vpnPeer.publicKey"
            class="text-truncate"
          ></v-list-item>
          <v-list-item
            title="Sub-rede da VPN"
            :subtitle="vpnStore.state?.cidr || 'Não configurada'"
          ></v-list-item>
          <v-list-item
            title="Acesso"
            :subtitle="vpnPeer.enabled ? 'Habilitado' : 'Revogado'"
          ></v-list-item>
        </v-list>
      </v-col>
    </v-row>

    <div
      class="text-subtitle-1 font-weight-bold mb-3 mt-4 d-flex align-center ga-2"
      style="gap: 8px"
    >
      <v-icon color="primary">mdi-swap-horizontal</v-icon>
      Tráfego do Túnel WireGuard
    </div>

    <v-row class="mb-4">
      <v-col cols="12" sm="6">
        <v-card border flat class="pa-4 rounded-lg text-center">
          <div class="text-caption text-grey">Total Recebido (RX)</div>
          <div class="text-h6 font-weight-bold text-success">
            {{ formatBytes(vpnPeer.bytesRx) }}
          </div>
        </v-card>
      </v-col>
      <v-col cols="12" sm="6">
        <v-card border flat class="pa-4 rounded-lg text-center">
          <div class="text-caption text-grey">Total Enviado (TX)</div>
          <div class="text-h6 font-weight-bold text-primary">
            {{ formatBytes(vpnPeer.bytesTx) }}
          </div>
        </v-card>
      </v-col>
    </v-row>

    <BaseMetricChart
      v-if="vpnTrafficSeries.length > 0"
      :series="vpnTrafficSeries"
      unit-type="bandwidth"
    />
    <div v-else class="text-center text-grey py-10 border rounded-lg bg-grey-lighten-5">
      <v-icon size="40" color="grey-lighten-1">mdi-chart-line-variant</v-icon>
      <div class="mt-2 text-subtitle-2">
        Sem amostras de tráfego ainda. O histórico é coletado a cada 30s pelo scheduler.
      </div>
    </div>

    <v-divider class="my-6"></v-divider>

    <div class="d-flex flex-column flex-md-row flex-wrap ga-3">
      <v-btn
        color="primary"
        variant="tonal"
        prepend-icon="mdi-content-copy"
        size="small"
        @click="emit('openVpnConfig')"
      >
        Copiar Configuração
      </v-btn>
      <v-btn
        color="warning"
        variant="tonal"
        prepend-icon="mdi-key-change"
        size="small"
        @click="emit('rotateVpnKeys')"
      >
        Gerar Novas Chaves
      </v-btn>
      <v-btn
        color="error"
        variant="tonal"
        prepend-icon="mdi-shield-off-outline"
        size="small"
        @click="emit('revokeVpnAccess')"
      >
        Revogar Acesso
      </v-btn>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useDeviceDetailStore } from '@/stores/deviceDetail'
import {
  useVpnStore,
  vpnProfileIcon,
  vpnProfileLabel,
  vpnStatusColor,
  vpnStatusLabel,
} from '@/stores/vpn'
import BaseMetricChart, { type ChartSeriesInput } from '@/components/BaseMetricChart.vue'
import { formatBytes, formatRelativeTime } from '@/utils/formatters'

const emit = defineEmits<{
  (e: 'openVpnConfig'): void
  (e: 'rotateVpnKeys'): void
  (e: 'revokeVpnAccess'): void
  (e: 'showVpnFirewallHints'): void
}>()

const detailStore = useDeviceDetailStore()
const vpnStore = useVpnStore()

const vpnPeer = computed(() => detailStore.device?.vpnPeer ?? null)
const vpnProfileLabelValue = computed(() =>
  vpnPeer.value ? vpnProfileLabel(vpnPeer.value.deviceProfile, vpnStore.profiles) : ''
)
const vpnProfileIconValue = computed(() =>
  vpnPeer.value ? vpnProfileIcon(vpnPeer.value.deviceProfile, vpnStore.profiles) : ''
)
const vpnStatusLabelValue = computed(() =>
  vpnPeer.value ? vpnStatusLabel(vpnPeer.value.connectionStatus) : ''
)
const vpnStatusColorValue = computed(() =>
  vpnPeer.value ? vpnStatusColor(vpnPeer.value.connectionStatus) : 'grey'
)
const vpnLastHandshakeText = computed(() =>
  vpnPeer.value ? formatRelativeTime(vpnPeer.value.lastHandshakeAt) : 'nunca'
)

const vpnNeedsFirewallHint = computed(() => {
  const peer = vpnPeer.value
  if (!peer || peer.connectionStatus !== 'connected') return false
  const pingMonitor = detailStore.monitors.find((m) => m.type === 'ping')
  return pingMonitor?.status === 'down'
})

const vpnTrafficSeries = computed<ChartSeriesInput[]>(() => {
  const rx = detailStore.metrics
    .filter((m) => m.metricName === 'vpn_rx_bps')
    .sort((a, b) => new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime())
  const tx = detailStore.metrics
    .filter((m) => m.metricName === 'vpn_tx_bps')
    .sort((a, b) => new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime())

  const series: ChartSeriesInput[] = []
  if (rx.length > 0) {
    series.push({
      id: 'vpn_rx_bps',
      label: 'Recebido (RX)',
      color: '#4CAF50',
      fillArea: false,
      data: rx.map((m) => ({ time: m.createdAt, value: Number(m.metricValue) || 0 })),
    })
  }
  if (tx.length > 0) {
    series.push({
      id: 'vpn_tx_bps',
      label: 'Enviado (TX)',
      color: '#2196F3',
      fillArea: false,
      data: tx.map((m) => ({ time: m.createdAt, value: Number(m.metricValue) || 0 })),
    })
  }
  return series
})
</script>
