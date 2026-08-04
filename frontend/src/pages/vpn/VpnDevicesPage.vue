<template>
  <div>
    <div class="d-flex align-center justify-space-between mb-6">
      <div>
        <h1 class="text-h4 font-weight-bold">Dispositivos VPN</h1>
        <p class="text-subtitle-1 text-grey-darken-1">
          Roteadores e clientes conectados ao NetMonitor pelo túnel WireGuard
        </p>
      </div>
      <div class="d-flex" style="gap: 8px">
        <v-btn variant="tonal" prepend-icon="mdi-cog-outline" :to="{ name: 'vpn-server' }">
          Servidor VPN
        </v-btn>
        <v-btn
          color="primary"
          prepend-icon="mdi-plus"
          :disabled="!vpnStore.isConfigured"
          @click="wizardOpen = true"
        >
          Adicionar dispositivo
        </v-btn>
      </div>
    </div>

    <v-alert
      v-if="!vpnStore.isConfigured && !vpnStore.loading"
      type="info"
      variant="tonal"
      class="mb-4"
      density="comfortable"
    >
      Configure o servidor VPN antes de adicionar dispositivos.
    </v-alert>

    <v-card elevation="2" class="rounded-lg">
      <v-data-table
        :headers="headers"
        :items="vpnStore.peers"
        :loading="vpnStore.loading"
        no-data-text="Nenhum dispositivo VPN cadastrado"
      >
        <template #item.name="{ item }">
          <div class="font-weight-medium">{{ item.device?.name || '—' }}</div>
          <div v-if="item.needsFirewallHint" class="text-caption text-warning d-flex align-center">
            <v-icon size="14" start>mdi-alert</v-icon>
            Túnel conectado, mas não responde a ping
            <v-btn
              size="x-small"
              variant="text"
              color="warning"
              class="ml-1"
              @click="showFirewallHints(item)"
            >
              Copiar regras de firewall
            </v-btn>
          </div>
        </template>

        <template #item.deviceProfile="{ item }">
          <v-chip size="small" variant="tonal">
            <v-icon start size="14">{{ profileIcon(item.deviceProfile) }}</v-icon>
            {{ profileLabel(item.deviceProfile) }}
          </v-chip>
        </template>

        <template #item.ipAddress="{ item }">
          {{ item.device?.ipAddress || '—' }}
        </template>

        <template #item.connectionStatus="{ item }">
          <v-chip :color="statusColor(item.connectionStatus)" size="small" variant="flat">
            {{ statusLabel(item.connectionStatus) }}
          </v-chip>
        </template>

        <template #item.lastHandshakeAt="{ item }">
          {{ relativeTime(item.lastHandshakeAt) }}
        </template>

        <template #item.traffic="{ item }">
          {{ formatBytes(item.bytesRx) }} ↓ / {{ formatBytes(item.bytesTx) }} ↑
        </template>

        <template #item.actions="{ item }">
          <div class="d-flex" style="gap: 4px">
            <v-tooltip text="Ver histórico de conectividade (ping)">
              <template #activator="{ props }">
                <v-btn
                  v-bind="props"
                  size="small"
                  icon="mdi-heart-pulse"
                  variant="text"
                  :disabled="!item.pingMonitorId"
                  :to="
                    item.pingMonitorId
                      ? { name: 'monitor-detail', params: { id: item.pingMonitorId } }
                      : undefined
                  "
                ></v-btn>
              </template>
            </v-tooltip>

            <v-tooltip text="Copiar script / configuração">
              <template #activator="{ props }">
                <v-btn
                  v-bind="props"
                  size="small"
                  icon="mdi-content-copy"
                  variant="text"
                  @click="openConfig(item)"
                ></v-btn>
              </template>
            </v-tooltip>

            <v-tooltip v-if="isMobile(item)" text="QR Code">
              <template #activator="{ props }">
                <v-btn
                  v-bind="props"
                  size="small"
                  icon="mdi-qrcode"
                  variant="text"
                  @click="openQrCode(item)"
                ></v-btn>
              </template>
            </v-tooltip>

            <v-tooltip text="Rotacionar chaves">
              <template #activator="{ props }">
                <v-btn
                  v-bind="props"
                  size="small"
                  icon="mdi-key-change"
                  variant="text"
                  color="warning"
                  @click="rotate(item)"
                ></v-btn>
              </template>
            </v-tooltip>

            <v-tooltip text="Revogar acesso">
              <template #activator="{ props }">
                <v-btn
                  v-bind="props"
                  size="small"
                  icon="mdi-cancel"
                  variant="text"
                  color="error"
                  @click="revoke(item)"
                ></v-btn>
              </template>
            </v-tooltip>
          </div>
        </template>
      </v-data-table>
    </v-card>

    <v-alert v-if="vpnStore.error" type="error" variant="tonal" class="mt-4" density="comfortable">
      {{ vpnStore.error }}
    </v-alert>

    <VpnPeerWizard v-model="wizardOpen" @created="onPeerCreated" />

    <VpnScriptViewer v-model="viewerOpen" :artifact="vpnStore.lastArtifact" :qr-svg="qrSvg" />

    <VpnFirewallHintsDialog v-model="firewallOpen" :content="firewallContent" />
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue'
import VpnPeerWizard from '@/components/VpnPeerWizard.vue'
import VpnScriptViewer from '@/components/VpnScriptViewer.vue'
import VpnFirewallHintsDialog from '@/components/VpnFirewallHintsDialog.vue'
import {
  useVpnStore,
  vpnProfileIcon,
  vpnProfileLabel,
  vpnStatusColor,
  vpnStatusLabel,
  vpnRelativeTime,
  type VpnPeer,
} from '@/stores/vpn'

const vpnStore = useVpnStore()

const wizardOpen = ref(false)
const viewerOpen = ref(false)
const firewallOpen = ref(false)
const firewallContent = ref('')
const qrSvg = ref<string | null>(null)

const headers = [
  { title: 'Nome', key: 'name' },
  { title: 'Perfil', key: 'deviceProfile', width: '180px' },
  { title: 'IP fixo', key: 'ipAddress', width: '130px' },
  { title: 'Status', key: 'connectionStatus', width: '140px' },
  { title: 'Último handshake', key: 'lastHandshakeAt', width: '180px' },
  { title: 'Tráfego RX/TX', key: 'traffic', width: '180px', sortable: false },
  { title: 'Ações', key: 'actions', sortable: false, width: '200px' },
]

onMounted(async () => {
  await Promise.all([vpnStore.fetchServer(), vpnStore.fetchPeers()])
})

const profileLabel = vpnProfileLabel
const profileIcon = vpnProfileIcon
const statusLabel = vpnStatusLabel
const statusColor = vpnStatusColor
const relativeTime = vpnRelativeTime

function isMobile(peer: VpnPeer): boolean {
  return peer.deviceProfile === 'mobile'
}

function formatBytes(value: number): string {
  if (!value) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const index = Math.min(Math.floor(Math.log(value) / Math.log(1024)), units.length - 1)
  return `${(value / 1024 ** index).toFixed(index === 0 ? 0 : 1)} ${units[index]}`
}

function onPeerCreated() {
  qrSvg.value = null
  viewerOpen.value = true
}

async function openConfig(peer: VpnPeer) {
  qrSvg.value = null
  const artifact = await vpnStore.fetchConfig(peer.id)
  if (artifact) viewerOpen.value = true
}

async function openQrCode(peer: VpnPeer) {
  const artifact = await vpnStore.fetchConfig(peer.id)
  if (!artifact) return

  qrSvg.value = await vpnStore.fetchQrCode(peer.id)
  viewerOpen.value = true
}

async function rotate(peer: VpnPeer) {
  const name = peer.device?.name || `peer #${peer.id}`
  if (!confirm(`Gerar novas chaves para "${name}"? A configuração atual deixará de funcionar.`)) {
    return
  }

  qrSvg.value = null
  const artifact = await vpnStore.rotateKeys(peer.id)
  if (artifact) viewerOpen.value = true
}

async function revoke(peer: VpnPeer) {
  const name = peer.device?.name || `peer #${peer.id}`
  if (!confirm(`Revogar o acesso de "${name}"? O túnel cai imediatamente e o IP é liberado.`)) {
    return
  }

  await vpnStore.revokePeer(peer.id)
}

async function showFirewallHints(peer: VpnPeer) {
  const content = await vpnStore.fetchFirewallHints(peer.id)
  if (!content) return

  firewallContent.value = content
  firewallOpen.value = true
}
</script>
