<template>
  <div>
    <PageHeader
      title="Descoberta de rede"
      subtitle="Varra uma rede e cadastre o que for encontrado — o tipo de cada equipamento é identificado automaticamente"
    >
      <template #actions>
        <v-btn color="primary" variant="tonal" prepend-icon="mdi-refresh" @click="refreshData">
          Atualizar
        </v-btn>
        <v-btn color="primary" variant="text" prepend-icon="mdi-lan" to="/networks"> Redes </v-btn>
      </template>
    </PageHeader>

    <!-- Disparo da varredura -->
    <v-card variant="outlined" rounded="lg" class="mb-4 pa-3 pa-md-4">
      <div class="d-flex flex-column flex-md-row align-stretch align-md-center ga-3">
        <v-select
          v-model="selectedNetworkId"
          :items="scannableNetworks"
          item-title="label"
          item-value="id"
          label="Rede a varrer"
          prepend-inner-icon="mdi-lan"
          variant="outlined"
          density="comfortable"
          hide-details
          class="flex-grow-1"
          :disabled="discoveryStore.scanning"
          :no-data-text="
            networksStore.networks.length === 0
              ? 'Nenhuma rede cadastrada — cadastre uma em Redes'
              : 'Nenhuma rede com faixa CIDR válida'
          "
        >
          <template #item="{ props: itemProps, item }">
            <v-list-item v-bind="itemProps" :subtitle="item.detail"></v-list-item>
          </template>
        </v-select>
        <v-btn
          v-if="!discoveryStore.scanning"
          color="primary"
          variant="flat"
          size="large"
          prepend-icon="mdi-radar"
          :disabled="selectedNetworkId === null"
          @click="scanSelectedNetwork"
        >
          Escanear
        </v-btn>
        <v-btn
          v-else
          color="error"
          variant="flat"
          size="large"
          prepend-icon="mdi-stop-circle-outline"
          @click="cancelScan"
        >
          Cancelar varredura
        </v-btn>
      </div>
    </v-card>

    <DiscoveryScanProgress
      v-if="scan.status !== 'idle'"
      :scan="scan"
      :percent="discoveryStore.progressPercent"
      :network-label="scanNetwork?.name ?? null"
      class="mb-4"
    />

    <v-card variant="outlined" rounded="lg">
      <v-tabs v-model="tab" color="primary" show-arrows>
        <v-tab value="results" prepend-icon="mdi-devices">
          Equipamentos
          <v-chip
            v-if="hosts.length > 0"
            size="x-small"
            color="primary"
            variant="flat"
            class="ms-2"
          >
            {{ hosts.length }}
          </v-chip>
        </v-tab>
        <v-tab value="runs" prepend-icon="mdi-history">Histórico</v-tab>
        <v-tab value="conflicts" prepend-icon="mdi-shield-alert-outline">
          Conflitos
          <v-chip
            v-if="discoveryStore.conflicts.length > 0"
            size="x-small"
            color="error"
            variant="flat"
            class="ms-2"
          >
            {{ discoveryStore.conflicts.length }}
          </v-chip>
        </v-tab>
      </v-tabs>
      <v-divider></v-divider>

      <v-card-text class="pa-3 pa-md-4">
        <v-window v-model="tab" :touch="false">
          <v-window-item value="results">
            <DiscoveryResultsTab
              :hosts="hosts"
              :added-ips="addedIps"
              :gateway="scanNetwork?.gateway ?? null"
              :scanning="discoveryStore.scanning"
              :can-scan="selectedNetworkId !== null"
              @open="openDetails"
              @add="openRegistration"
              @scan="scanSelectedNetwork"
            />
          </v-window-item>

          <v-window-item value="runs">
            <DiscoveryRunsTab
              :runs="discoveryStore.runs"
              :loading="discoveryStore.loading"
              :cleaning="cleaning"
              @cleanup="handleCleanup"
            />
          </v-window-item>

          <v-window-item value="conflicts">
            <DiscoveryConflictsTab
              :conflicts="discoveryStore.conflicts"
              :host-mode="discoveryStore.environment?.isHostMode ?? false"
              :loading="discoveryStore.loadingConflicts"
              @check="onCheckConflicts"
            />
          </v-window-item>
        </v-window>
      </v-card-text>
    </v-card>

    <v-snackbar v-model="feedback.visible" :color="feedback.color" timeout="6000">
      {{ feedback.message }}
    </v-snackbar>

    <DiscoveryHostDialog
      v-model="detailsOpen"
      :host="detailsHost"
      :added="detailsHost ? addedIps.has(detailsHost.ipAddress) : false"
      :is-gateway="isGateway(detailsHost)"
      @add="openRegistration"
    />

    <DeviceDialog v-model="registrationOpen" :prefill-data="prefill" @saved="onDeviceSaved" />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { storeToRefs } from 'pinia'
import PageHeader from '@/components/PageHeader.vue'
import DeviceDialog from '@/components/DeviceDialog.vue'
import DiscoveryScanProgress from '@/components/discovery/DiscoveryScanProgress.vue'
import DiscoveryResultsTab from '@/components/discovery/DiscoveryResultsTab.vue'
import DiscoveryRunsTab from '@/components/discovery/DiscoveryRunsTab.vue'
import DiscoveryConflictsTab from '@/components/discovery/DiscoveryConflictsTab.vue'
import DiscoveryHostDialog from '@/components/discovery/DiscoveryHostDialog.vue'
import { confirm } from '@/composables/useConfirm'
import { useDiscoveryStore, type StreamedDiscoveryHost } from '@/stores/discovery'
import { useNetworksStore } from '@/stores/networks'
import { useDevicesStore, type Device } from '@/stores/devices'
import {
  discoveryDeviceName,
  discoveryHasSnmp,
  discoveryIdentity,
  discoveryRegistrationType,
  discoveryTypeMeta,
  discoveryVendor,
} from '@/utils/discoveryPresentation'

const route = useRoute()
const router = useRouter()
const discoveryStore = useDiscoveryStore()
const networksStore = useNetworksStore()
const devicesStore = useDevicesStore()
const { scan, hosts } = storeToRefs(discoveryStore)

const tab = ref('results')
const selectedNetworkId = ref<number | null>(null)
const feedback = reactive({ visible: false, message: '', color: 'success' })
const cleaning = ref(false)
const detailsOpen = ref(false)
const detailsHost = ref<StreamedDiscoveryHost | null>(null)
const registrationOpen = ref(false)
const registrationHost = ref<StreamedDiscoveryHost | null>(null)

function notify(message: string, color = 'success') {
  feedback.message = message
  feedback.color = color
  feedback.visible = true
}

const scannableNetworks = computed(() =>
  networksStore.networks
    .filter((network) => network.scannable !== false)
    .map((network) => ({
      id: network.id,
      label: `${network.name} — ${network.cidr}`,
      detail: `${network.usableHosts ?? 0} endereço(s)${network.probeId ? ' · via probe remoto' : ''}`,
    }))
)

/** A rede da varredura ao vivo; sem ela, a escolhida no seletor. */
const scanNetwork = computed(() => {
  const id = scan.value.networkId ?? selectedNetworkId.value
  return networksStore.networks.find((network) => network.id === id) ?? null
})

const addedIps = computed<ReadonlySet<string>>(
  () =>
    new Set(
      devicesStore.devices
        .map((device) => device.ipAddress)
        .filter((ip): ip is string => Boolean(ip))
    )
)

function isGateway(host: StreamedDiscoveryHost | null): boolean {
  return Boolean(host && scanNetwork.value?.gateway === host.ipAddress)
}

// A tela segue a varredura que está rodando, inclusive a iniciada em outra aba.
watch(
  [() => scan.value.networkId, scannableNetworks],
  ([networkId, networks]) => {
    if (
      networkId !== null &&
      discoveryStore.scanning &&
      networks.some((network) => network.id === networkId)
    ) {
      selectedNetworkId.value = networkId
    }
  },
  { immediate: true }
)

// O desfecho chega pelo stream: o aviso sai daqui, não do clique.
watch(
  () => scan.value.status,
  (status, previous) => {
    if (previous !== 'running' && previous !== 'pending') return
    if (status === 'completed') {
      notify(`Varredura concluída: ${scan.value.hosts.length} dispositivo(s) encontrado(s).`)
    } else if (status === 'failed') {
      notify(scan.value.error || 'Erro durante a varredura.', 'error')
    } else if (status === 'cancelled') {
      notify('Varredura cancelada.', 'warning')
    }
  }
)

const prefill = computed<Partial<Device> | null>(() => {
  const host = registrationHost.value
  if (!host) return null
  const network = scanNetwork.value
  const gateway = isGateway(host)
  const parent =
    !gateway && network?.gateway
      ? devicesStore.devices.find((device) => device.ipAddress === network.gateway)
      : undefined
  const type = discoveryTypeMeta(host)
  return {
    name: discoveryDeviceName(host) || `${type.label} (${host.ipAddress})`,
    ipAddress: host.ipAddress,
    type: discoveryRegistrationType(host, gateway),
    vendor: discoveryVendor(host) ?? undefined,
    model: discoveryIdentity(host)?.hardwareModel || undefined,
    macAddress: host.macAddress || undefined,
    siteId: network?.siteId ?? null,
    networkId: network?.id ?? null,
    parentId: parent?.id ?? null,
    isMonitored: true,
    snmpEnabled: discoveryHasSnmp(host),
  }
})

onMounted(async () => {
  void devicesStore.fetchDevices()
  void discoveryStore.fetchEnvironment()
  void discoveryStore.fetchConflicts()
  void discoveryStore.fetchDiscoveryRuns()
  // A lista de redes precisa existir antes de honrar o `?networkId=` da URL.
  await networksStore.fetchNetworks()
  if (selectedNetworkId.value === null && scannableNetworks.value.length === 1) {
    selectedNetworkId.value = scannableNetworks.value[0].id
  }
  await applyRouteIntent()
})

/**
 * O botão "Escanear" de /networks traz o bloco escolhido (`networkId`) e, quando
 * pediu a varredura, `scan=1`. A ordem é consumida uma vez — a query sai da URL
 * antes do disparo, senão cada F5 iniciaria uma varredura nova.
 */
async function applyRouteIntent() {
  const requestedId = Number(route.query.networkId)
  if (!Number.isInteger(requestedId) || requestedId <= 0) return

  const shouldScan = route.query.scan === '1'
  if (shouldScan) {
    await router.replace({ path: '/discovery', query: { networkId: String(requestedId) } })
  }
  if (!scannableNetworks.value.some((network) => network.id === requestedId)) {
    notify('A rede escolhida não tem uma faixa CIDR varredurável.', 'warning')
    return
  }
  if (discoveryStore.scanning) {
    notify(
      'Já existe uma varredura em andamento — aguarde ou cancele para iniciar outra.',
      'warning'
    )
    return
  }
  selectedNetworkId.value = requestedId
  if (shouldScan) await scanSelectedNetwork()
}

async function refreshData() {
  await Promise.all([
    devicesStore.fetchDevices(),
    discoveryStore.fetchDiscoveryRuns(),
    discoveryStore.fetchEnvironment(),
    discoveryStore.fetchConflicts(),
  ])
}

async function scanSelectedNetwork() {
  if (selectedNetworkId.value === null) return
  tab.value = 'results'
  const runId = await discoveryStore.startScan(selectedNetworkId.value)
  if (runId === null) {
    notify(discoveryStore.error || 'Não foi possível iniciar a varredura.', 'error')
  }
}

async function cancelScan() {
  const cancelled = await discoveryStore.cancelScan()
  if (!cancelled) notify(discoveryStore.error || 'Não foi possível cancelar.', 'error')
}

async function onCheckConflicts() {
  const found = await discoveryStore.checkConflicts()
  if (found.length > 0) {
    notify(`${found.length} conflito(s) de rede detectado(s).`, 'warning')
  } else {
    notify('Auditoria concluída: nenhum conflito de rede detectado.')
  }
}

function openDetails(host: StreamedDiscoveryHost) {
  detailsHost.value = host
  detailsOpen.value = true
}

function openRegistration(host: StreamedDiscoveryHost) {
  detailsOpen.value = false
  registrationHost.value = host
  registrationOpen.value = true
}

async function onDeviceSaved() {
  registrationHost.value = null
  registrationOpen.value = false
  await devicesStore.fetchDevices()
  notify('Dispositivo cadastrado com sucesso.')
}

async function handleCleanup() {
  const ok = await confirm({
    title: 'Limpar histórico de varreduras',
    message: 'Isso apagará varreduras com mais de 7 dias e todos os seus resultados. Continuar?',
    confirmText: 'Limpar',
    confirmColor: 'warning',
    icon: 'mdi-broom',
  })
  if (!ok) return

  cleaning.value = true
  const result = await discoveryStore.cleanup(7)
  cleaning.value = false
  if (result) {
    notify(`${result.removedRuns} varredura(s) antiga(s) removida(s).`)
    await discoveryStore.fetchDiscoveryRuns()
  } else {
    notify(discoveryStore.error ?? 'Não foi possível limpar o histórico.', 'error')
  }
}
</script>
