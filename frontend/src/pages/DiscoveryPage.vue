<template>
  <div>
    <PageHeader
      title="Central de Descoberta (Discovery)"
      subtitle="Encontre e aprove novos equipamentos na rede"
    >
      <template #actions>
        <v-btn color="primary" prepend-icon="mdi-refresh" @click="refreshData">
          <span class="hidden-sm-and-down">Atualizar</span>
          <span class="hidden-md-and-up">Atualizar</span>
        </v-btn>
      </template>
    </PageHeader>

    <!-- Disparo de varredura por faixa cadastrada em /networks -->
    <v-card elevation="2" class="mobile-full-bleed mb-4 mb-md-6 pa-2 pa-md-4">
      <div class="d-flex flex-column flex-md-row align-start align-md-center ga-3">
        <v-icon color="secondary" size="28" class="hidden-sm-and-down">mdi-radar</v-icon>
        <div class="flex-grow-1 w-100" style="min-width: 260px">
          <v-select
            v-model="selectedNetworkId"
            :items="scannableNetworks"
            item-title="label"
            item-value="id"
            label="Varrer o bloco de IP de uma rede cadastrada"
            variant="outlined"
            density="compact"
            hide-details
            :no-data-text="
              networksStore.networks.length === 0
                ? 'Nenhuma rede cadastrada — cadastre uma em Redes'
                : 'Nenhuma rede com faixa CIDR válida'
            "
          ></v-select>
        </div>
        <div class="d-flex ga-2 w-100 w-md-auto">
          <v-btn
            v-if="!scanning"
            color="secondary"
            prepend-icon="mdi-radar"
            :disabled="selectedNetworkId === null"
            class="flex-grow-1 flex-md-grow-0"
            @click="scanSelectedNetwork"
          >
            <span class="hidden-sm-and-down">Escanear bloco</span>
            <span class="hidden-md-and-up">Escanear</span>
          </v-btn>
          <v-btn
            v-else
            color="error"
            variant="tonal"
            prepend-icon="mdi-stop-circle-outline"
            class="flex-grow-1 flex-md-grow-0"
            @click="cancelScan"
          >
            <span class="hidden-sm-and-down">Cancelar varredura</span>
            <span class="hidden-md-and-up">Cancelar</span>
          </v-btn>
          <v-btn
            variant="text"
            prepend-icon="mdi-lan"
            to="/networks"
            class="flex-grow-1 flex-md-grow-0"
          >
            <span class="hidden-sm-and-down">Gerenciar redes</span>
            <span class="hidden-md-and-up">Redes</span>
          </v-btn>
        </div>
      </div>
    </v-card>

    <!-- Abas: Resultados & Execuções -->
    <v-card elevation="2" class="mobile-full-bleed">
      <v-tabs v-model="tab" color="primary">
        <v-tab value="results">Resultados Encontrados</v-tab>
        <v-tab value="runs">Histórico de Escaneamento</v-tab>
      </v-tabs>
      <v-divider></v-divider>

      <v-card-text class="pa-3 pa-md-4">
        <v-window v-model="tab">
          <!-- Resultados Encontrados -->
          <v-window-item value="results">
            <!-- Card de progresso durante a varredura -->
            <v-card v-if="scanning" variant="outlined" class="rounded-0 pa-3 mb-3">
              <div
                class="d-flex align-start align-md-center justify-space-between flex-column flex-md-row ga-2 mb-2"
              >
                <div>
                  <div class="text-subtitle-2 font-weight-medium">
                    <v-icon color="primary" class="mr-2">mdi-radar</v-icon>
                    {{ PHASE_LABELS[scanPhase] }}
                  </div>
                  <div class="text-caption text-grey">{{ PHASE_SUBTITLES[scanPhase] }}</div>
                </div>
                <div class="text-caption text-grey text-md-right">
                  <div class="font-weight-medium">{{ streamedHosts.length }} encontrado(s)</div>
                  <div>{{ scanProgressText }}</div>
                </div>
              </div>

              <v-progress-linear
                :model-value="scanProgressPercent"
                color="primary"
                height="8"
                rounded
              ></v-progress-linear>

              <div
                v-if="scanLog.length > 0"
                class="mt-2 pt-2 border-t thin text-caption text-grey font-mono"
              >
                <div v-for="(log, idx) in scanLog" :key="idx" class="scan-log-line">
                  {{ log }}
                </div>
              </div>
            </v-card>

            <!-- Estado vazio -->
            <v-card
              v-if="streamedHosts.length === 0 && !scanning"
              variant="outlined"
              class="rounded-0 pa-6 text-center text-grey"
            >
              <v-icon size="40" color="grey-lighten-1" class="mb-2">mdi-radar</v-icon>
              <div class="text-subtitle-2 font-weight-medium">Nenhum dispositivo encontrado.</div>
              <div class="text-caption text-grey mb-3">
                Selecione uma rede cadastrada e inicie uma varredura.
              </div>
              <v-btn
                color="secondary"
                prepend-icon="mdi-radar"
                :disabled="selectedNetworkId === null"
                @click="scanSelectedNetwork"
              >
                Escanear agora
              </v-btn>
            </v-card>

            <!-- Lista de resultados -->
            <div v-if="streamedHosts.length > 0" class="d-flex flex-column ga-2">
              <div v-if="!scanning" class="d-flex align-center justify-space-between mb-2">
                <div class="text-subtitle-2 font-weight-medium">
                  Última varredura: {{ streamedHosts.length }} dispositivo(s)
                </div>
                <v-btn
                  size="small"
                  color="primary"
                  variant="tonal"
                  prepend-icon="mdi-refresh"
                  @click="startNewScan"
                >
                  Nova varredura
                </v-btn>
              </div>

              <v-card
                v-for="item in streamedHosts"
                :key="item.ipAddress"
                variant="outlined"
                class="rounded-0 pa-3 cursor-pointer"
                @click="openDetailDialog(item)"
              >
                <div class="d-flex align-start justify-space-between ga-2">
                  <div class="flex-grow-1 text-break">
                    <div class="text-subtitle-1 font-weight-bold text-primary">
                      {{ item.ipAddress }}
                    </div>
                    <div class="text-caption text-grey-darken-1">
                      {{ item.mdnsName || item.hostname || 'Dispositivo sem nome' }}
                    </div>
                    <div class="d-flex flex-wrap align-center ga-2 mt-1">
                      <v-chip size="x-small" color="info" variant="tonal">
                        {{ item.vendor || 'Fabricante desconhecido' }}
                      </v-chip>
                      <v-chip
                        v-if="isIpAdded(item.ipAddress)"
                        size="x-small"
                        color="success"
                        variant="tonal"
                      >
                        JÁ ADICIONADO
                      </v-chip>
                      <v-chip
                        v-if="item.openPorts && item.openPorts.length > 0"
                        size="x-small"
                        color="success"
                        variant="tonal"
                      >
                        {{ item.openPorts.length }} porta(s)
                      </v-chip>
                    </div>
                    <div v-if="selectedNetwork" class="text-caption text-grey mt-1">
                      {{ selectedNetwork.name }} — {{ selectedNetwork.cidr }}
                    </div>
                  </div>
                  <div class="text-caption font-weight-medium text-success">
                    {{ Math.ceil(item.confidence) }}%
                  </div>
                </div>

                <div v-if="!isIpAdded(item.ipAddress)" class="d-flex ga-2 mt-2">
                  <v-btn
                    size="small"
                    color="success"
                    prepend-icon="mdi-plus"
                    variant="flat"
                    block
                    @click.stop="handleAdd(item)"
                  >
                    Adicionar
                  </v-btn>
                </div>
                <div v-else class="mt-2">
                  <v-chip size="small" color="success" variant="tonal">Já adicionado</v-chip>
                </div>
              </v-card>
            </div>
          </v-window-item>

          <!-- Histórico de Varreduras -->
          <v-window-item value="runs">
            <div class="d-flex align-center justify-space-between mb-4">
              <div class="text-subtitle-2 text-grey">
                Varreduras anteriores ficam aqui para auditoria.
              </div>
              <v-btn
                size="small"
                color="error"
                variant="outlined"
                prepend-icon="mdi-delete-sweep"
                :loading="cleanupLoading"
                @click="handleCleanup"
              >
                Limpar histórico antigo
              </v-btn>
            </div>

            <div v-if="loadingRuns" class="pa-4 text-center">
              <v-progress-circular indeterminate color="primary"></v-progress-circular>
            </div>

            <ResponsiveDataTable
              v-else
              :headers="runHeaders"
              :items="discoveryRuns"
              :items-per-page="-1"
              hide-default-footer
              no-data-text="Nenhuma varredura registrada."
              :clickable="false"
            >
              <template #item.id="{ item }">
                <span class="font-weight-medium">#{{ item.id }}</span>
              </template>

              <template #item.network="{ item }">
                <span>{{ item.networkName || `Rede #${item.networkId}` }}</span>
              </template>

              <template #item.status="{ item }">
                <v-chip :color="runStatusColor(item.status)" size="small">
                  {{ runStatusLabel(item.status) }}
                </v-chip>
              </template>

              <template #item.startedAt="{ item }">
                <span class="text-body-2">{{ formatDateTime(item.startedAt) }}</span>
              </template>

              <template #mobile-item="{ item }">
                <div class="d-flex flex-column ga-2">
                  <div class="d-flex align-start justify-space-between ga-2">
                    <div class="flex-grow-1 text-break">
                      <div class="text-subtitle-1 font-weight-bold">
                        {{ item.networkName || `Rede #${item.networkId}` }}
                      </div>
                      <div class="text-caption text-grey-darken-1">
                        {{ item.cidr || '—' }}
                      </div>
                      <div class="d-flex flex-wrap align-center ga-2 mt-1">
                        <v-chip :color="runStatusColor(item.status)" size="x-small" variant="tonal">
                          {{ runStatusLabel(item.status) }}
                        </v-chip>
                        <span class="text-caption text-grey">
                          {{ formatDateTime(item.startedAt) }}
                        </span>
                      </div>
                    </div>
                    <div class="text-caption font-weight-medium">
                      {{ item.devicesFound }} encontrados
                    </div>
                  </div>
                </div>
              </template>
            </ResponsiveDataTable>
          </v-window-item>
        </v-window>
      </v-card-text>
    </v-card>

    <v-snackbar v-model="feedback.visible" :color="feedback.color" timeout="7000">
      {{ feedback.message }}
    </v-snackbar>

    <DeviceDialog v-model="deviceDialogOpen" :prefill-data="dialogPrefill" @saved="onDeviceSaved" />

    <DiscoveryResultDialog
      v-model="resultDialogOpen"
      :result="selectedDetailResult"
      @add="handleDetailAdd"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, ref, onMounted, onUnmounted, reactive } from 'vue'
import {
  useDiscoveryStore,
  type DiscoveryResult,
  type DiscoveryRun,
  type DiscoveryPhase,
  type StreamedDiscoveryHost,
  type ScanSessionState,
} from '@/stores/discovery'
import { useNetworksStore } from '@/stores/networks'
import { useDevicesStore } from '@/stores/devices'
import { formatDateTime } from '@/utils/formatters'
import DeviceDialog from '@/components/DeviceDialog.vue'
import DiscoveryResultDialog from '@/components/DiscoveryResultDialog.vue'
import PageHeader from '@/components/PageHeader.vue'
import ResponsiveDataTable from '@/components/ResponsiveDataTable.vue'
import type { Device } from '@/stores/devices'

/** Espelha `MAX_SCAN_HOSTS` de `modules/discovery/cidr_range.ts` */
const MAX_SCAN_HOSTS = 1024

const PHASE_LABELS: Record<DiscoveryPhase, string> = {
  idle: 'Aguardando',
  icmp: 'Ping (ICMP)',
  discovery: 'Descoberta (ARP/mDNS/SSDP)',
  ports: 'Varredura de portas',
  snmp: 'Consulta SNMP',
}

const PHASE_SUBTITLES: Record<DiscoveryPhase, string> = {
  idle: 'Preparando varredura...',
  icmp: 'Hosts que respondem ao ping aparecem abaixo em tempo real.',
  discovery: 'Descobrindo nome, fabricante e endereço MAC da rede local.',
  ports: 'Testando portas abertas em cada host encontrado.',
  snmp: 'Consultando informações do sistema via SNMP.',
}

const discoveryStore = useDiscoveryStore()
const networksStore = useNetworksStore()
const devicesStore = useDevicesStore()
const tab = ref('results')
const selectedNetworkId = ref<number | null>(null)
const feedback = reactive({ visible: false, message: '', color: 'success' })
const deviceDialogOpen = ref(false)
const selectedResult = ref<DiscoveryResult | StreamedDiscoveryHost | null>(null)
const resultDialogOpen = ref(false)
const selectedDetailResult = ref<DiscoveryResult | StreamedDiscoveryHost | null>(null)
const cleanupLoading = ref(false)

const scanning = ref(false)
const scanPhase = ref<DiscoveryPhase>('idle')
const scanProgressCurrent = ref(0)
const scanProgressTotal = ref(100)
const streamedHostMap = ref(new Map<string, StreamedDiscoveryHost>())
const scanError = ref<string | null>(null)
const scanLog = ref<string[]>([])
let unsubscribeScanStream: (() => void) | null = null

const streamedHosts = computed(() =>
  Array.from(streamedHostMap.value.values()).sort((a, b) => a.ipAddress.localeCompare(b.ipAddress))
)

const scanProgressPercent = computed(() =>
  scanProgressTotal.value > 0 ? (scanProgressCurrent.value / scanProgressTotal.value) * 100 : 0
)

const scanProgressText = computed(() => {
  if (scanPhase.value === 'icmp') {
    return `${scanProgressCurrent.value} de ${scanProgressTotal.value} endereços testados`
  }
  return `${scanProgressCurrent.value} de ${scanProgressTotal.value} hosts analisados`
})

const discoveryRuns = ref<DiscoveryRun[]>([])
const loadingRuns = ref(false)

const addedIpSet = computed(
  () => new Set(devicesStore.devices.map((d) => d.ipAddress).filter(Boolean))
)

function isIpAdded(ip: string): boolean {
  return addedIpSet.value.has(ip)
}

const selectedNetwork = computed(() =>
  networksStore.networks.find((n) => n.id === selectedNetworkId.value)
)

const dialogPrefill = computed<Partial<Device> | null>(() => {
  if (!selectedResult.value) return null
  const result = selectedResult.value
  return {
    name: result.mdnsName || result.hostname || result.ipAddress,
    ipAddress: result.ipAddress,
    type: result.deviceType || 'other',
    vendor: result.vendor || undefined,
    macAddress: result.macAddress || undefined,
    siteId: selectedNetwork.value?.siteId ?? null,
    networkId: selectedNetwork.value?.id ?? null,
    isMonitored: true,
    snmpEnabled: false,
  }
})

const runHeaders = [
  { title: 'ID Run', key: 'id', width: '80px' },
  { title: 'Rede', key: 'network' },
  { title: 'Faixa', key: 'cidr' },
  { title: 'Dispositivos Encontrados', key: 'devicesFound', width: '120px' },
  { title: 'Status', key: 'status', width: '110px' },
  { title: 'Iniciado em', key: 'startedAt', width: '160px' },
]

/**
 * O tamanho da faixa entra no próprio rótulo da opção: como `hint` renderiza
 * uma linha extra sob o campo, ele desalinhava o select dos botões ao lado.
 */
const scannableNetworks = computed(() =>
  networksStore.networks
    .filter((network) => network.scannable !== false)
    .map((network) => {
      const usableHosts = network.usableHosts ?? 0
      const scope =
        usableHosts > MAX_SCAN_HOSTS
          ? `${usableHosts} endereços (varre os primeiros ${MAX_SCAN_HOSTS})`
          : `${usableHosts} endereço(s)`

      return {
        id: network.id,
        label: `${network.name} — ${network.cidr} · ${scope}`,
      }
    })
)

onMounted(async () => {
  networksStore.fetchNetworks()
  devicesStore.fetchDevices()
  await loadRuns()
  await restoreScanState()
})

onUnmounted(() => {
  unsubscribeScanStream?.()
})

async function refreshData() {
  await Promise.all([devicesStore.fetchDevices(), loadRuns()])
}

async function loadRuns() {
  loadingRuns.value = true
  try {
    discoveryRuns.value = await discoveryStore.fetchDiscoveryRuns()
  } catch (err: unknown) {
    console.error('Erro ao carregar histórico de varreduras:', err)
    discoveryRuns.value = []
  } finally {
    loadingRuns.value = false
  }
}

function resetScanState() {
  scanning.value = false
  scanPhase.value = 'idle'
  scanProgressCurrent.value = 0
  scanProgressTotal.value = 100
  streamedHostMap.value = new Map()
  scanError.value = null
  scanLog.value = []
}

function applyScanState(state: ScanSessionState) {
  scanning.value = state.status === 'running'
  scanPhase.value = state.phase
  scanProgressCurrent.value = state.progressCurrent
  scanProgressTotal.value = state.progressTotal
  scanError.value = state.error
  scanLog.value = state.logs

  if (state.networkId && selectedNetworkId.value !== state.networkId) {
    selectedNetworkId.value = state.networkId
  }

  streamedHostMap.value = new Map(
    state.hosts.map((host) => [host.ipAddress, host as StreamedDiscoveryHost])
  )
}

async function restoreScanState() {
  const state = await discoveryStore.fetchScanState()
  if (!state || state.status === 'idle') {
    resetScanState()
    return
  }

  applyScanState(state)

  if (state.status === 'running') {
    subscribeToScanStream()
  }
}

function subscribeToScanStream() {
  unsubscribeScanStream?.()
  unsubscribeScanStream = discoveryStore.subscribeScanStream({
    onState: (state) => {
      applyScanState(state)
      if (state.status === 'completed') {
        feedback.color = 'success'
        feedback.message = `Varredura concluída: ${streamedHosts.value.length} dispositivo(s) encontrado(s).`
        feedback.visible = true
        loadRuns()
      } else if (state.status === 'failed') {
        feedback.color = 'error'
        feedback.message = state.error || 'Erro durante a varredura.'
        feedback.visible = true
        loadRuns()
      } else if (state.status === 'cancelled') {
        feedback.color = 'warning'
        feedback.message = 'Varredura cancelada.'
        feedback.visible = true
        loadRuns()
      }
    },
    onError: (message) => {
      console.error(message)
    },
  })
}

function startNewScan() {
  resetScanState()
  unsubscribeScanStream?.()
  unsubscribeScanStream = null
  window.scrollTo({ top: 0, behavior: 'smooth' })
}

async function scanSelectedNetwork() {
  if (selectedNetworkId.value === null) return

  startNewScan()
  scanning.value = true
  scanPhase.value = 'idle'
  scanProgressCurrent.value = 0
  scanProgressTotal.value = 100

  const runId = await discoveryStore.startScan(selectedNetworkId.value)
  if (!runId) {
    scanning.value = false
    feedback.color = 'error'
    feedback.message = discoveryStore.error || 'Não foi possível iniciar a varredura.'
    feedback.visible = true
    return
  }

  subscribeToScanStream()
}

async function cancelScan() {
  unsubscribeScanStream?.()
  unsubscribeScanStream = null
  await discoveryStore.cancelScan()
  scanning.value = false
  feedback.color = 'warning'
  feedback.message = 'Varredura cancelada.'
  feedback.visible = true
  await loadRuns()
}

const RUN_STATUS: Record<string, { label: string; color: string }> = {
  pending: { label: 'Na fila', color: 'grey' },
  running: { label: 'Em execução', color: 'info' },
  completed: { label: 'Concluída', color: 'success' },
  failed: { label: 'Falhou', color: 'error' },
}

function runStatusLabel(status: string): string {
  return RUN_STATUS[status]?.label ?? status
}

function runStatusColor(status: string): string {
  return RUN_STATUS[status]?.color ?? 'grey'
}

function handleAdd(item: DiscoveryResult | StreamedDiscoveryHost) {
  selectedResult.value = item
  deviceDialogOpen.value = true
}

function openDetailDialog(item: DiscoveryResult | StreamedDiscoveryHost) {
  selectedDetailResult.value = item
  resultDialogOpen.value = true
}

function handleDetailAdd() {
  resultDialogOpen.value = false
  if (selectedDetailResult.value) {
    handleAdd(selectedDetailResult.value)
  }
  selectedDetailResult.value = null
}

async function onDeviceSaved() {
  selectedResult.value = null
  deviceDialogOpen.value = false
  await devicesStore.fetchDevices()
  feedback.color = 'success'
  feedback.message = 'Dispositivo cadastrado com sucesso.'
  feedback.visible = true
}

async function handleCleanup() {
  if (
    !confirm('Isso apagará varreduras com mais de 7 dias e todos os seus resultados. Continuar?')
  ) {
    return
  }

  cleanupLoading.value = true
  const result = await discoveryStore.cleanup(7)
  cleanupLoading.value = false

  feedback.color = result ? 'success' : 'error'
  feedback.message = result
    ? `${result.removedRuns} varredura(s) antiga(s) removida(s).`
    : (discoveryStore.error ?? 'Não foi possível limpar o histórico.')
  feedback.visible = true

  if (result) {
    await loadRuns()
  }
}
</script>
