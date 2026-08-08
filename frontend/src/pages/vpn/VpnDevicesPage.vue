<template>
  <div>
    <PageHeader
      title="Dispositivos VPN"
      subtitle="Roteadores e clientes conectados ao NetMonitor pelo túnel WireGuard"
    >
      <template #actions>
        <v-btn variant="tonal" prepend-icon="mdi-cog-outline" :to="{ name: 'vpn-server' }">
          <span class="hidden-sm-and-down">Servidor VPN</span>
          <span class="hidden-md-and-up">Servidor</span>
        </v-btn>
        <v-btn
          color="primary"
          prepend-icon="mdi-plus"
          :disabled="!vpnStore.isConfigured"
          @click="wizardOpen = true"
        >
          <span class="hidden-sm-and-down">Adicionar dispositivo</span>
          <span class="hidden-md-and-up">Novo</span>
        </v-btn>
      </template>
    </PageHeader>

    <v-alert
      v-if="!vpnStore.isConfigured && !vpnStore.loading"
      type="info"
      variant="tonal"
      class="mb-4"
      density="comfortable"
    >
      Configure o servidor VPN antes de adicionar dispositivos.
    </v-alert>

    <v-card elevation="2" class="mobile-full-bleed">
      <ResponsiveDataTable
        :headers="headers"
        :items="vpnStore.peers"
        :loading="vpnStore.loading"
        :items-per-page="-1"
        hide-default-footer
        no-data-text="Nenhum dispositivo VPN cadastrado"
        :clickable="false"
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
          <!-- O ICMP sai da máquina da API, que não tem rota para a VPN: o
               equipamento nunca recebe o pacote, então não é caso de firewall. -->
          <div
            v-else-if="item.pingOutsideTunnel"
            class="text-caption text-info d-flex align-center"
          >
            <v-icon size="14" start>mdi-lan-disconnect</v-icon>
            O ping não sai pelo túnel — registre o vpn-probe
            <v-tooltip
              text="Este monitor está sendo executado pela API, fora da VPN. Defina VPN_PROBE_TOKEN e suba o serviço vpn-probe, depois recrie o dispositivo."
              max-width="360"
            >
              <template #activator="{ props }">
                <v-icon v-bind="props" size="14" class="ml-1">mdi-help-circle-outline</v-icon>
              </template>
            </v-tooltip>
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

        <!-- O handshake é renegociado só quando há o que enviar: num túnel
             ocioso ele fica minutos parado sem que nada esteja errado. O que
             realmente indica vida é o keepalive, então é ele que aparece aqui. -->
        <template #item.lastActivity="{ item }">
          <v-tooltip :text="`Último handshake: ${relativeTime(item.lastHandshakeAt)}`">
            <template #activator="{ props }">
              <span v-bind="props">{{
                relativeTime(item.lastSeenAt || item.lastHandshakeAt)
              }}</span>
            </template>
          </v-tooltip>
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

            <v-tooltip text="Renomear dispositivo">
              <template #activator="{ props }">
                <v-btn
                  v-bind="props"
                  size="small"
                  icon="mdi-pencil"
                  variant="text"
                  @click="openRename(item)"
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
                  @click="openConfig(item)"
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

        <template #mobile-item="{ item }">
          <div class="d-flex flex-column ga-2">
            <div class="d-flex align-start justify-space-between ga-2">
              <div class="flex-grow-1 text-break">
                <div class="text-subtitle-2 font-weight-bold">
                  {{ item.device?.name || `Peer #${item.id}` }}
                </div>
                <div
                  v-if="item.needsFirewallHint"
                  class="text-caption text-warning d-flex align-center ga-1"
                >
                  <v-icon size="12">mdi-alert</v-icon>
                  Túnel conectado, mas não responde a ping
                </div>
                <div
                  v-else-if="item.pingOutsideTunnel"
                  class="text-caption text-info d-flex align-center ga-1"
                >
                  <v-icon size="12">mdi-lan-disconnect</v-icon>
                  Ping fora do túnel
                </div>
                <div class="d-flex flex-wrap align-center ga-2 mt-1">
                  <v-chip size="x-small" variant="tonal">
                    <v-icon start size="12">{{ profileIcon(item.deviceProfile) }}</v-icon>
                    {{ profileLabel(item.deviceProfile) }}
                  </v-chip>
                  <span class="text-caption text-grey-darken-1">{{
                    item.device?.ipAddress || '—'
                  }}</span>
                  <v-chip :color="statusColor(item.connectionStatus)" size="x-small" variant="flat">
                    {{ statusLabel(item.connectionStatus) }}
                  </v-chip>
                </div>
                <div class="text-caption text-grey mt-1">
                  {{ formatBytes(item.bytesRx) }} ↓ / {{ formatBytes(item.bytesTx) }} ↑ ·
                  {{ relativeTime(item.lastSeenAt || item.lastHandshakeAt) }}
                </div>
              </div>
            </div>
            <div class="d-flex flex-wrap justify-end ga-1 mt-1">
              <v-btn
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
              <v-btn
                size="small"
                icon="mdi-pencil"
                variant="text"
                @click="openRename(item)"
              ></v-btn>
              <v-btn
                size="small"
                icon="mdi-content-copy"
                variant="text"
                @click="openConfig(item)"
              ></v-btn>
              <v-btn
                v-if="isMobile(item)"
                size="small"
                icon="mdi-qrcode"
                variant="text"
                @click="openConfig(item)"
              ></v-btn>
              <v-btn
                size="small"
                icon="mdi-key-change"
                variant="text"
                color="warning"
                @click="rotate(item)"
              ></v-btn>
              <v-btn
                size="small"
                icon="mdi-cancel"
                variant="text"
                color="error"
                @click="revoke(item)"
              ></v-btn>
            </div>
          </div>
        </template>
      </ResponsiveDataTable>
    </v-card>

    <v-alert v-if="vpnStore.error" type="error" variant="tonal" class="mt-4" density="comfortable">
      {{ vpnStore.error }}
    </v-alert>

    <v-dialog
      v-model="renameOpen"
      :max-width="$vuetify.display.xs ? undefined : 460"
      :fullscreen="$vuetify.display.xs"
    >
      <v-card class="rounded-lg">
        <v-card-title class="font-weight-bold d-flex align-center">
          <v-icon start color="primary">mdi-pencil</v-icon>
          Renomear dispositivo
        </v-card-title>

        <v-card-text>
          <v-text-field
            v-model="renameValue"
            label="Nome do dispositivo *"
            variant="outlined"
            density="comfortable"
            autofocus
            :error-messages="renameError"
            @keyup.enter="submitRename"
          ></v-text-field>

          <div class="text-caption text-medium-emphasis">
            O IP na VPN e as chaves não mudam — o túnel continua no ar. Os monitores de ping e SNMP
            acompanham o novo nome.
          </div>
        </v-card-text>

        <v-card-actions class="px-4 pb-4">
          <v-spacer></v-spacer>
          <v-btn variant="text" @click="renameOpen = false">Cancelar</v-btn>
          <v-btn
            color="primary"
            variant="flat"
            :loading="vpnStore.saving"
            :disabled="!renameValue.trim()"
            @click="submitRename"
          >
            Salvar
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <VpnPeerWizard v-model="wizardOpen" @created="onPeerCreated" />

    <VpnScriptViewer v-model="viewerOpen" :artifact="vpnStore.lastArtifact" />

    <VpnFirewallHintsDialog v-model="firewallOpen" :content="firewallContent" />
  </div>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import VpnPeerWizard from '@/components/VpnPeerWizard.vue'
import VpnScriptViewer from '@/components/VpnScriptViewer.vue'
import VpnFirewallHintsDialog from '@/components/VpnFirewallHintsDialog.vue'
import PageHeader from '@/components/PageHeader.vue'
import ResponsiveDataTable from '@/components/ResponsiveDataTable.vue'
import {
  useVpnStore,
  vpnProfileIcon,
  vpnProfileLabel,
  vpnStatusColor,
  vpnStatusLabel,
  type VpnPeer,
} from '@/stores/vpn'
import { useEventsStore } from '@/stores/events'
import { formatBytes, formatRelativeTime } from '@/utils/formatters'

const vpnStore = useVpnStore()
const eventsStore = useEventsStore()

const wizardOpen = ref(false)
const viewerOpen = ref(false)
const firewallOpen = ref(false)
const firewallContent = ref('')
const renameOpen = ref(false)
const renameValue = ref('')
const renameError = ref('')
const renamingPeerId = ref<number | null>(null)

const headers = [
  { title: 'Nome', key: 'name' },
  { title: 'Perfil', key: 'deviceProfile', width: '180px' },
  { title: 'IP fixo', key: 'ipAddress', width: '130px' },
  { title: 'Status', key: 'connectionStatus', width: '140px' },
  { title: 'Última atividade', key: 'lastActivity', value: 'lastSeenAt', width: '180px' },
  { title: 'Tráfego RX/TX', key: 'traffic', width: '180px', sortable: false },
  { title: 'Ações', key: 'actions', sortable: false, width: '200px' },
]

/**
 * Rede de segurança: o status vem do SSE (`vpn:peers_updated`), então uma queda
 * silenciosa da conexão congela a tela sem nenhum sinal para o operador. Só
 * busca de novo enquanto o stream estiver fora do ar — com ele de pé, este
 * intervalo não gera request nenhum.
 */
const FALLBACK_REFRESH_MS = 30_000
let fallbackTimer: ReturnType<typeof setInterval> | null = null

onMounted(async () => {
  await Promise.all([vpnStore.fetchServer(), vpnStore.fetchPeers()])

  fallbackTimer = setInterval(() => {
    if (!eventsStore.isConnected) vpnStore.fetchPeers()
  }, FALLBACK_REFRESH_MS)
})

onUnmounted(() => {
  if (fallbackTimer) clearInterval(fallbackTimer)
})

const profileLabel = vpnProfileLabel
const profileIcon = vpnProfileIcon
const statusLabel = vpnStatusLabel
const statusColor = vpnStatusColor
const relativeTime = formatRelativeTime

function isMobile(peer: VpnPeer): boolean {
  return peer.deviceProfile === 'mobile'
}

function onPeerCreated() {
  viewerOpen.value = true
}

function openRename(peer: VpnPeer) {
  renamingPeerId.value = peer.id
  renameValue.value = peer.device?.name || ''
  renameError.value = ''
  renameOpen.value = true
}

async function submitRename() {
  const peerId = renamingPeerId.value
  const name = renameValue.value.trim()
  if (!peerId || !name) return

  renameError.value = ''
  const saved = await vpnStore.renamePeer(peerId, name)

  if (saved) {
    renameOpen.value = false
  } else {
    renameError.value = vpnStore.error || 'Não foi possível renomear'
  }
}

/** O artefato já traz o QR Code dos perfis móveis — não há segunda requisição. */
async function openConfig(peer: VpnPeer) {
  const artifact = await vpnStore.fetchConfig(peer.id)
  if (artifact) viewerOpen.value = true
}

async function rotate(peer: VpnPeer) {
  const name = peer.device?.name || `peer #${peer.id}`
  if (!confirm(`Gerar novas chaves para "${name}"? A configuração atual deixará de funcionar.`)) {
    return
  }

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
