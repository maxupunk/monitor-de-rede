<template>
  <div>
    <!-- Botão de Voltar -->
    <v-btn variant="text" prepend-icon="mdi-arrow-left" class="mb-2 mb-md-4" to="/devices">
      Voltar para Dispositivos
    </v-btn>

    <DeviceDetailHeader
      :device="detailStore.device"
      :device-id="deviceId"
      :tool-groups="toolGroups"
      :busy="{ snmpScan: detailStore.scanningSnmp, snmpCollect: detailStore.pollingSnmp }"
      :can-create-monitor="can.createMonitor"
      :can-edit="can.editIdentity"
      :ai-prompt="aiDevicePrompt"
      :ai-context="aiDeviceContext"
      @tool="runTool"
      @create-monitor="openMonitorDialog()"
      @edit="editDeviceDialog = true"
    />

    <!-- Abas Interativas -->
    <v-card elevation="2" class="rounded-lg">
      <v-tabs
        v-model="activeTab"
        color="primary"
        align-tabs="title"
        show-arrows
        density="comfortable"
      >
        <v-tab value="overview" prepend-icon="mdi-information-outline">Visão Geral</v-tab>
        <v-tab value="monitors" prepend-icon="mdi-heart-pulse">
          Monitores ({{ detailStore.monitors.length }})
        </v-tab>
        <v-tab value="rules" prepend-icon="mdi-bell-cog-outline">Regras</v-tab>
        <v-tab v-if="can.interfaces" value="interfaces" prepend-icon="mdi-expansion-card">
          Interfaces SNMP ({{ detailStore.interfaces.length }})
        </v-tab>
        <v-tab v-if="can.events" value="events" prepend-icon="mdi-history">
          Histórico de Eventos
        </v-tab>
        <v-tab v-if="can.logs" value="logs" prepend-icon="mdi-text-box-search-outline">Logs</v-tab>
        <v-tab v-if="can.vpn" value="vpn" prepend-icon="mdi-shield-lock-outline">VPN</v-tab>
        <!-- Abas que vêm de plugin: sempre na cor PLUGIN_COLOR (AGENTS.md §12) -->
        <v-tab
          v-if="can.wifi"
          value="wifi"
          prepend-icon="mdi-wifi"
          :base-color="PLUGIN_COLOR"
          :color="PLUGIN_COLOR"
          :slider-color="PLUGIN_COLOR"
        >
          Clientes Wi-Fi
        </v-tab>
        <v-tab
          v-if="can.plugins"
          value="plugins"
          prepend-icon="mdi-puzzle-outline"
          :base-color="PLUGIN_COLOR"
          :color="PLUGIN_COLOR"
          :slider-color="PLUGIN_COLOR"
        >
          Plugins
          <v-badge
            v-if="installedPluginCount"
            :content="installedPluginCount"
            :color="PLUGIN_COLOR"
            inline
          />
        </v-tab>
      </v-tabs>

      <v-divider></v-divider>

      <v-card-text class="pa-2 pa-sm-4 pa-md-6">
        <v-window v-model="activeTab" :touch="false">
          <!-- Aba Visão Geral -->
          <v-window-item value="overview">
            <DeviceOverviewTab
              :device-id="deviceId"
              :can-health="can.health"
              :can-snmp-configured="can.snmpConfigured"
              :can-snmp-connected="can.snmpConnected"
              @open-scan-modal="scanModalOpen = true"
              @open-interface-chart="openInterfaceChart"
            />
          </v-window-item>

          <!-- Aba Monitores -->
          <v-window-item value="monitors">
            <DeviceMonitorsTab
              @open-monitor-dialog="openMonitorDialog"
              @reload-monitors="reloadMonitors"
            />
          </v-window-item>

          <!-- Aba Regras -->
          <v-window-item value="rules">
            <DeviceRulesTab
              :device-id="deviceId"
              :device-name="detailStore.device?.name"
              :monitor-names="monitorNames"
              :available-fields="detailStore.capabilities?.alertFields"
            />
          </v-window-item>

          <!-- Aba Interfaces SNMP -->
          <v-window-item value="interfaces">
            <DeviceInterfacesTab @open-interface-chart="openInterfaceChart" />
          </v-window-item>

          <!-- Aba Eventos -->
          <v-window-item value="events">
            <DeviceEventsTab :device-id="deviceId" />
          </v-window-item>

          <!-- Aba Logs: syslog recebido deste dispositivo -->
          <v-window-item value="logs">
            <DeviceLogsTab
              v-if="detailStore.device"
              :device-id="deviceId"
              :device="detailStore.device"
            />
          </v-window-item>

          <!-- Aba VPN -->
          <v-window-item value="vpn">
            <DeviceVpnTab
              @open-vpn-config="openVpnConfig"
              @rotate-vpn-keys="rotateVpnKeys"
              @revoke-vpn-access="revokeVpnAccess"
              @show-vpn-firewall-hints="showVpnFirewallHints"
            />
          </v-window-item>

          <!-- Aba Clientes Wi-Fi: as estações chegam pelo SSE -->
          <v-window-item value="wifi">
            <DeviceWifiClients v-if="can.wifi" :device-id="deviceId" />
          </v-window-item>

          <!-- Aba Plugins: os instalados e o gerenciamento (catálogo, credenciais, histórico) -->
          <v-window-item value="plugins">
            <DevicePluginsTab
              v-if="activeTab === 'plugins' && detailStore.device"
              v-model:section="pluginSection"
              :device-id="deviceId"
              :device-name="detailStore.device.name"
            />
          </v-window-item>
        </v-window>
      </v-card-text>
    </v-card>

    <SnmpScanDialog v-model="scanModalOpen" :device-id="deviceId" />

    <!-- Modal de Gráfico de Tráfego de Interface -->
    <TrafficChartDialog
      v-model="chartDialogOpen"
      :interface-id="selectedInterface?.id ?? null"
      :interface-name="selectedInterface ? interfaceLabel(selectedInterface) : ''"
      :initial-metric="selectedChartMetricType"
      :metrics="detailStore.metrics"
      can-manage-monitoring
      :is-monitored="selectedInterface?.isMonitored === true"
      :busy="detailStore.updatingInterfaceId === selectedInterface?.id"
      @toggle-monitoring="toggleInterfaceMonitoring"
    />

    <!-- Modais da aba VPN -->
    <VpnScriptViewer v-model="vpnViewerOpen" :artifact="vpnStore.lastArtifact" :qr-svg="null" />
    <VpnFirewallHintsDialog v-model="vpnFirewallOpen" :content="vpnFirewallContent" />

    <!-- Modal de Scanner de Portas TCP/UDP -->
    <PortScanDialog
      v-model="portScanOpen"
      :host="detailStore.device?.ipAddress"
      :device-name="detailStore.device?.name"
    />

    <!-- Modal de Traceroute ICMP -->
    <TracerouteDialog
      v-model="tracerouteOpen"
      :host="detailStore.device?.ipAddress"
      :device-name="detailStore.device?.name"
    />

    <QuickPingDialog
      v-model="quickPingOpen"
      :device-id="Number(deviceId)"
      :device-name="detailStore.device?.name"
      :ip-address="detailStore.device?.ipAddress"
    />

    <!-- Modal de Playbook de Diagnóstico -->
    <DiagnosticPlaybookDialog
      v-model="playbookOpen"
      :device-id="Number(deviceId)"
      :device-name="detailStore.device?.name"
      :device-ip="detailStore.device?.ipAddress"
      initial-playbook-type="device_reachability"
    />

    <!-- Monitor deste equipamento: o vínculo já vem definido e travado -->
    <MonitorFormDialog
      v-model="monitorDialog"
      :monitor="editingMonitor"
      :default-device-id="deviceId"
      lock-device
      @saved="onMonitorSaved"
    />

    <!-- Modal de Edição do Equipamento -->
    <DeviceDialog
      v-model="editDeviceDialog"
      :device-to-edit="detailStore.device"
      @saved="onDeviceSaved"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, computed, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import {
  interfaceLabel,
  useDeviceDetailStore,
  type DeviceInterface,
  type DeviceMonitor,
} from '@/stores/deviceDetail'
import TrafficChartDialog from '@/components/TrafficChartDialog.vue'
import VpnScriptViewer from '@/components/VpnScriptViewer.vue'
import VpnFirewallHintsDialog from '@/components/VpnFirewallHintsDialog.vue'
import PortScanDialog from '@/components/PortScanDialog.vue'
import TracerouteDialog from '@/components/TracerouteDialog.vue'
import DiagnosticPlaybookDialog from '@/components/DiagnosticPlaybookDialog.vue'
import QuickPingDialog from '@/components/diagnostics/QuickPingDialog.vue'
import { useAuthStore } from '@/stores/auth'
import MonitorFormDialog from '@/components/MonitorFormDialog.vue'
import DeviceDialog from '@/components/DeviceDialog.vue'
import DeviceDetailHeader from '@/components/devices/DeviceDetailHeader.vue'
import SnmpScanDialog from '@/components/devices/SnmpScanDialog.vue'
import DeviceRulesTab from '@/components/devices/DeviceRulesTab.vue'
import DeviceOverviewTab from '@/components/devices/tabs/DeviceOverviewTab.vue'
import DeviceMonitorsTab from '@/components/devices/tabs/DeviceMonitorsTab.vue'
import DeviceInterfacesTab from '@/components/devices/tabs/DeviceInterfacesTab.vue'
import DeviceEventsTab from '@/components/devices/tabs/DeviceEventsTab.vue'
import DeviceLogsTab from '@/components/devices/tabs/DeviceLogsTab.vue'
import DeviceVpnTab from '@/components/devices/tabs/DeviceVpnTab.vue'
import DeviceWifiClients from '@/components/wifi/DeviceWifiClients.vue'
import DevicePluginsTab from '@/components/devices/tabs/DevicePluginsTab.vue'
import { usePluginsStore } from '@/stores/plugins'
import { installedPluginTabs, PLUGIN_COLOR } from '@/utils/pluginPresentation'
import { deviceToolGroups, type DeviceToolKey } from '@/utils/deviceTools'
import { useVpnStore } from '@/stores/vpn'
import { useAreasStore } from '@/stores/areas'
import { useLogsStore } from '@/stores/logs'
import { confirm } from '@/composables/useConfirm'

const route = useRoute()
const router = useRouter()
const detailStore = useDeviceDetailStore()
const vpnStore = useVpnStore()
const logsStore = useLogsStore()
const pluginsStore = usePluginsStore()
const areasStore = useAreasStore()
const activeTab = ref('overview')
const abaPedida = ref<string | null>(null)
/** Seção da aba Plugins (um instalado ou o gerenciamento); `null` = a padrão. */
const pluginSection = ref<string | null>(null)

const can = computed(() => {
  const caps = detailStore.capabilities
  const snmpConnected = caps?.snmpConnected ?? false
  const isSystem = caps?.isSystem ?? false
  const snmpScan = caps?.canSnmpScan ?? !isSystem
  const snmpCollect = caps?.canSnmpCollect ?? false
  const scanPorts = caps?.canScanPorts ?? false
  const editIdentity = caps?.canEditIdentity ?? !isSystem
  return {
    isSystem,
    snmpConfigured: caps?.snmpConfigured ?? false,
    snmpConnected,
    interfaces: caps?.interfaces ?? false,
    events: caps?.events ?? false,
    logs: caps?.logs ?? false,
    // Aba de área exige a capacidade **e** a área ligada (ADR 017).
    vpn: areasStore.isVisible('vpn') && (caps?.vpn ?? Boolean(detailStore.device?.vpnPeer)),
    wifi: areasStore.isVisible('network') && (caps?.wifi ?? false),
    health: caps?.health ?? false,
    plugins: caps?.plugins ?? false,
    snmpScan,
    snmpCollect,
    scanPorts,
    editIdentity,
    createMonitor: caps?.canCreateMonitor ?? !isSystem,
    anyHeaderAction: snmpScan || snmpCollect || scanPorts || editIdentity,
  }
})

const abasAplicaveis = computed(() => {
  const abas = ['overview', 'monitors', 'rules']
  if (can.value.interfaces) abas.push('interfaces')
  if (can.value.events) abas.push('events')
  if (can.value.logs) abas.push('logs')
  if (can.value.vpn) abas.push('vpn')
  if (can.value.wifi) abas.push('wifi')
  if (can.value.plugins) abas.push('plugins')
  return abas
})

/** Quantos plugins estão instalados — o selo da aba Plugins. */
const installedPluginCount = computed(() =>
  can.value.plugins ? installedPluginTabs(pluginsStore.deviceViews[deviceId.value]).length : 0
)

/** Ferramentas do cabeçalho; sem nenhuma ação de cabeçalho, nenhuma ferramenta. */
const toolGroups = computed(() => {
  const c = can.value
  const hasIp = c.anyHeaderAction && Boolean(detailStore.device?.ipAddress)
  return deviceToolGroups({
    snmpScan: c.snmpScan,
    snmpCollect: c.snmpCollect,
    ports: c.scanPorts,
    traceroute: hasIp,
    quickPing: hasIp && !c.isSystem && auth.canWrite,
    playbook: hasIp,
  })
})

const scanModalOpen = ref(false)

function runTool(key: DeviceToolKey) {
  const actions: Record<DeviceToolKey, () => void> = {
    snmpScan: () => (scanModalOpen.value = true),
    snmpCollect: () => void detailStore.triggerSnmpPoll(deviceId.value),
    ports: () => (portScanOpen.value = true),
    traceroute: () => (tracerouteOpen.value = true),
    quickPing: () => (quickPingOpen.value = true),
    playbook: () => (playbookOpen.value = true),
  }
  actions[key]()
}

const monitorNames = computed<Record<number, string>>(() =>
  Object.fromEntries(detailStore.monitors.map((monitor) => [monitor.id, monitor.name]))
)
const portScanOpen = ref(false)
const tracerouteOpen = ref(false)
const quickPingOpen = ref(false)
const auth = useAuthStore()
const playbookOpen = ref(false)
const editDeviceDialog = ref(false)

async function onDeviceSaved() {
  if (deviceId.value) {
    await detailStore.loadDeviceDetails(deviceId.value)
  }
}

const chartDialogOpen = ref(false)
const selectedInterface = ref<DeviceInterface | null>(null)
const selectedChartMetricType = ref<'inBps' | 'outBps' | 'inOctets' | 'outOctets' | 'combined'>(
  'inBps'
)

const vpnViewerOpen = ref(false)
const vpnFirewallOpen = ref(false)
const vpnFirewallContent = ref('')

function openInterfaceChart(
  intf: DeviceInterface,
  metricType: 'inBps' | 'outBps' | 'inOctets' | 'outOctets' | 'combined' = 'combined'
) {
  selectedInterface.value = intf
  selectedChartMetricType.value = metricType
  chartDialogOpen.value = true
}

async function toggleInterfaceMonitoring(enabled: boolean) {
  const target = selectedInterface.value
  if (!target) return

  const success = await detailStore.setInterfaceMonitoring(deviceId.value, target.id, enabled)
  if (success) {
    selectedInterface.value =
      detailStore.interfaces.find((intf) => intf.id === target.id) ?? selectedInterface.value
  }
}

const deviceId = computed(() => Number(route.params.id))
// A lista de plugins vem uma vez por equipamento (abertura da tela); instalar e
// desinstalar atualizam a mesma visão na store.
watch(
  () => [deviceId.value, can.value.plugins] as const,
  ([id, enabled]) => {
    if (id && enabled && !pluginsStore.deviceViews[id]) void pluginsStore.loadDevice(id)
  },
  { immediate: true }
)
const aiDevicePrompt = computed(
  () =>
    `Faça um diagnóstico do dispositivo ${detailStore.device?.name || `#${deviceId.value}`}: estado atual, alertas, interfaces com problema e erros recentes nos logs.`
)
const aiDeviceContext = computed(() => ({ deviceId: deviceId.value }))

onMounted(() => {
  if (deviceId.value) {
    detailStore.loadDeviceDetails(deviceId.value)
    if (areasStore.isVisible('vpn')) vpnStore.fetchServer()
  }
})

watch(
  () => route.query.tab,
  (pedida) => {
    abaPedida.value = typeof pedida === 'string' ? pedida : null
  },
  { immediate: true }
)

// `?section=` escolhe a seção da aba Plugins (ex.: `credentials`, o link de
// quem chega da ativação do log para gerenciar o login salvo).
watch(
  () => route.query.section,
  (pedida) => {
    if (typeof pedida === 'string') pluginSection.value = pedida
  },
  { immediate: true }
)

// A aba pedida pela URL espera existir (as capacidades chegam depois); uma aba
// que deixou de existir volta à Visão Geral. `?tab=plugin-<id>` — o link de um
// plugin instalado — abre a aba Plugins já nele.
watch(
  [abasAplicaveis, abaPedida],
  ([abas, pedida]) => {
    if (pedida?.startsWith('plugin-') && abas.includes('plugins')) {
      activeTab.value = 'plugins'
      pluginSection.value = pedida
      abaPedida.value = null
    } else if (pedida && abas.includes(pedida)) {
      activeTab.value = pedida
      abaPedida.value = null
    } else if (!abas.includes(activeTab.value)) {
      activeTab.value = 'overview'
    }
  },
  { immediate: true }
)

watch(activeTab, (aba, anterior) => {
  if (aba === 'logs') {
    void logsStore.fetchSources()
  } else if (anterior === 'logs') {
    logsStore.stopTail()
  }
})

const monitorDialog = ref(false)
const editingMonitor = ref<DeviceMonitor | null>(null)

function openMonitorDialog(monitor?: DeviceMonitor) {
  editingMonitor.value = monitor ?? null
  monitorDialog.value = true
}

async function onMonitorSaved() {
  if (deviceId.value) await detailStore.loadDeviceDetails(deviceId.value)
}

async function reloadMonitors() {
  if (deviceId.value) await detailStore.reloadMonitors(deviceId.value)
}

async function openVpnConfig() {
  const vpnPeer = detailStore.device?.vpnPeer
  if (!vpnPeer) return
  const artifact = await vpnStore.fetchConfig(vpnPeer.id)
  if (artifact) vpnViewerOpen.value = true
}

async function rotateVpnKeys() {
  const vpnPeer = detailStore.device?.vpnPeer
  if (!vpnPeer) return

  const ok = await confirm({
    title: 'Gerar novas chaves VPN',
    message: `Gerar novas chaves para "${detailStore.device?.name}"? A configuração atual deixará de funcionar.`,
    confirmText: 'Gerar novas chaves',
    confirmColor: 'warning',
    icon: 'mdi-key-change',
  })
  if (!ok) return

  const artifact = await vpnStore.rotateKeys(vpnPeer.id)
  if (artifact) {
    vpnViewerOpen.value = true
    await detailStore.loadDeviceDetails(deviceId.value)
  }
}

async function revokeVpnAccess() {
  const vpnPeer = detailStore.device?.vpnPeer
  if (!vpnPeer) return

  const ok = await confirm({
    title: 'Revogar acesso VPN',
    message: `Revogar o acesso VPN de "${detailStore.device?.name}"? O túnel cai imediatamente, o IP é liberado e este dispositivo será removido.`,
    confirmText: 'Revogar acesso',
    confirmColor: 'error',
    icon: 'mdi-shield-remove-outline',
  })
  if (!ok) return

  const success = await vpnStore.revokePeer(vpnPeer.id)
  if (success) {
    router.push({ name: 'vpn-devices' })
  }
}

async function showVpnFirewallHints() {
  const vpnPeer = detailStore.device?.vpnPeer
  if (!vpnPeer) return
  const content = await vpnStore.fetchFirewallHints(vpnPeer.id)
  if (!content) return

  vpnFirewallContent.value = content
  vpnFirewallOpen.value = true
}
</script>
