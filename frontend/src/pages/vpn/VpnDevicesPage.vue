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

    <v-dialog v-model="firewallOpen" max-width="680">
      <v-card class="rounded-lg">
        <v-card-title class="font-weight-bold">Regras de firewall</v-card-title>
        <v-card-subtitle>
          Aplique no equipamento para liberar ICMP e SNMP na interface WireGuard.
        </v-card-subtitle>
        <v-card-text>
          <v-sheet class="rounded-lg pa-4" color="grey-darken-4">
            <pre class="script-content">{{ firewallContent }}</pre>
          </v-sheet>
        </v-card-text>
        <v-card-actions class="px-4 pb-4">
          <v-spacer></v-spacer>
          <v-btn variant="text" @click="firewallOpen = false">Fechar</v-btn>
          <v-btn
            color="primary"
            variant="flat"
            prepend-icon="mdi-content-copy"
            @click="copyFirewall"
          >
            Copiar regras
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue'
import VpnPeerWizard from '@/components/VpnPeerWizard.vue'
import VpnScriptViewer from '@/components/VpnScriptViewer.vue'
import {
  useVpnStore,
  type VpnConnectionStatus,
  type VpnDeviceProfile,
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

const profileLabels: Record<VpnDeviceProfile, string> = {
  mikrotik: 'MikroTik',
  openwrt: 'OpenWrt',
  linux: 'Linux',
  windows: 'Windows',
  mobile: 'Celular',
}

const profileIcons: Record<VpnDeviceProfile, string> = {
  mikrotik: 'mdi-router-network',
  openwrt: 'mdi-router-wireless',
  linux: 'mdi-linux',
  windows: 'mdi-microsoft-windows',
  mobile: 'mdi-cellphone',
}

const statusLabels: Record<VpnConnectionStatus, string> = {
  connected: 'Conectado',
  unstable: 'Instável',
  disconnected: 'Desconectado',
  awaiting: 'Aguardando',
}

const statusColors: Record<VpnConnectionStatus, string> = {
  connected: 'success',
  unstable: 'warning',
  disconnected: 'error',
  awaiting: 'grey',
}

onMounted(async () => {
  await Promise.all([vpnStore.fetchServer(), vpnStore.fetchPeers()])
})

function profileLabel(profile: VpnDeviceProfile): string {
  return profileLabels[profile] || profile
}

function profileIcon(profile: VpnDeviceProfile): string {
  return profileIcons[profile] || 'mdi-devices'
}

function statusLabel(status: VpnConnectionStatus): string {
  return statusLabels[status] || status
}

function statusColor(status: VpnConnectionStatus): string {
  return statusColors[status] || 'grey'
}

function isMobile(peer: VpnPeer): boolean {
  return peer.deviceProfile === 'mobile'
}

function formatBytes(value: number): string {
  if (!value) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const index = Math.min(Math.floor(Math.log(value) / Math.log(1024)), units.length - 1)
  return `${(value / 1024 ** index).toFixed(index === 0 ? 0 : 1)} ${units[index]}`
}

function relativeTime(value: string | null): string {
  if (!value) return 'nunca'

  const elapsedSeconds = Math.floor((Date.now() - new Date(value).getTime()) / 1000)
  if (elapsedSeconds < 60) return `há ${elapsedSeconds}s`
  if (elapsedSeconds < 3600) return `há ${Math.floor(elapsedSeconds / 60)} min`
  if (elapsedSeconds < 86400) return `há ${Math.floor(elapsedSeconds / 3600)} h`
  return `há ${Math.floor(elapsedSeconds / 86400)} dias`
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

async function copyFirewall() {
  try {
    await navigator.clipboard.writeText(firewallContent.value)
  } catch {
    // navegador sem permissão de área de transferência
  }
}
</script>

<style scoped>
.script-content {
  color: #e0e0e0;
  font-family: 'Consolas', 'Monaco', monospace;
  font-size: 12px;
  line-height: 1.6;
  margin: 0;
  white-space: pre-wrap;
}
</style>
