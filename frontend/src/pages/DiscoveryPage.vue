<template>
  <div>
    <PageHeader
      title="Central de Descoberta (Discovery)"
      subtitle="Revise equipamentos encontrados na rede para aprovação ou mesclagem"
    >
      <template #actions>
        <v-btn color="primary" prepend-icon="mdi-refresh" @click="refreshData">
          <span class="hidden-sm-and-down">Atualizar Descobertas</span>
          <span class="hidden-md-and-up">Atualizar</span>
        </v-btn>
      </template>
    </PageHeader>

    <!-- Disparo de varredura por faixa cadastrada em /networks -->
    <v-card elevation="2" class="mobile-full-bleed mb-6 pa-4">
      <div class="d-flex flex-column flex-md-row align-start align-md-center ga-3">
        <v-icon color="secondary" size="28" class="hidden-sm-and-down">mdi-radar</v-icon>
        <div class="flex-grow-1 w-100" style="min-width: 260px">
          <v-select
            v-model="selectedNetworkId"
            :items="scannableNetworks"
            item-title="label"
            item-value="id"
            label="Varrer o bloco de IP de uma rede cadastrada"
            :hint="selectedNetworkHint"
            persistent-hint
            variant="outlined"
            density="compact"
            :no-data-text="
              networksStore.networks.length === 0
                ? 'Nenhuma rede cadastrada — cadastre uma em Redes'
                : 'Nenhuma rede com faixa CIDR válida'
            "
          ></v-select>
        </div>
        <div class="d-flex ga-2 w-100 w-md-auto">
          <v-btn
            color="secondary"
            prepend-icon="mdi-radar"
            :disabled="selectedNetworkId === null"
            :loading="networksStore.scanningId !== null"
            class="flex-grow-1 flex-md-grow-0"
            @click="scanSelectedNetwork"
          >
            <span class="hidden-sm-and-down">Escanear bloco</span>
            <span class="hidden-md-and-up">Escanear</span>
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

      <v-card-text class="pa-4">
        <v-window v-model="tab">
          <!-- Resultados Encontrados -->
          <v-window-item value="results">
            <v-infinite-scroll :key="results.scrollKey.value" @load="results.load">
              <ResponsiveDataTable
                :headers="resultHeaders"
                :items="results.items.value"
                :items-per-page="-1"
                hide-default-footer
                no-data-text=""
                clickable
                @click:row="(_event, { item }) => openDetailDialog(item)"
              >
                <template #item.ipAddress="{ item }">
                  <span class="font-weight-medium">{{ item.ipAddress }}</span>
                </template>

                <template #item.suggestedName="{ item }">
                  <span>{{ item.mdnsName || item.hostname || '—' }}</span>
                </template>

                <template #item.network="{ item }">
                  <span v-if="item.discoveryRun?.network" class="text-body-2">
                    {{ item.discoveryRun.network.name }}
                    <div class="text-caption text-grey">{{ item.discoveryRun.network.cidr }}</div>
                  </span>
                  <span v-else class="text-caption text-grey">—</span>
                </template>

                <template #item.confidence="{ item }">
                  <v-progress-linear
                    :model-value="item.confidence"
                    color="success"
                    height="18"
                    rounded
                  >
                    <template #default="{ value }">
                      <strong class="text-caption text-white">{{ Math.ceil(value) }}%</strong>
                    </template>
                  </v-progress-linear>
                </template>

                <template #item.actions="{ item }">
                  <div v-if="!isIpAdded(item.ipAddress)" class="d-flex ga-2">
                    <v-btn
                      size="small"
                      color="success"
                      prepend-icon="mdi-plus"
                      @click.stop="handleAdd(item)"
                    >
                      Adicionar
                    </v-btn>
                  </div>
                  <div v-else>
                    <v-chip size="small" color="success" variant="tonal">Já adicionado</v-chip>
                  </div>
                </template>

                <template #mobile-item="{ item }">
                  <div class="d-flex flex-column ga-2">
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
                        </div>
                        <div v-if="item.discoveryRun?.network" class="text-caption text-grey mt-1">
                          {{ item.discoveryRun.network.name }} —
                          {{ item.discoveryRun.network.cidr }}
                        </div>
                      </div>
                      <div class="text-caption font-weight-medium text-success">
                        {{ Math.ceil(item.confidence) }}%
                      </div>
                    </div>
                    <div v-if="!isIpAdded(item.ipAddress)" class="d-flex ga-2 mt-1">
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
                    <div v-else>
                      <v-chip size="small" color="success" variant="tonal">Já adicionado</v-chip>
                    </div>
                  </div>
                </template>
              </ResponsiveDataTable>
              <template #empty>
                <div class="text-caption text-grey text-center py-4">
                  Nenhum novo dispositivo pendente de aprovação.
                </div>
              </template>
            </v-infinite-scroll>
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
            <v-infinite-scroll :key="runs.scrollKey.value" @load="runs.load">
              <ResponsiveDataTable
                :headers="runHeaders"
                :items="runs.items.value"
                :items-per-page="-1"
                hide-default-footer
                no-data-text=""
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
                          <v-chip
                            :color="runStatusColor(item.status)"
                            size="x-small"
                            variant="tonal"
                          >
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
              <template #empty>
                <div class="text-caption text-grey text-center py-4">
                  Nenhuma outra varredura registrada.
                </div>
              </template>
            </v-infinite-scroll>
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
import { computed, ref, onMounted, reactive } from 'vue'
import { useDiscoveryStore, type DiscoveryResult, type DiscoveryRun } from '@/stores/discovery'
import { useNetworksStore } from '@/stores/networks'
import { useDevicesStore } from '@/stores/devices'
import { useInfiniteList } from '@/composables/useInfiniteList'
import { formatDateTime } from '@/utils/formatters'
import DeviceDialog from '@/components/DeviceDialog.vue'
import DiscoveryResultDialog from '@/components/DiscoveryResultDialog.vue'
import PageHeader from '@/components/PageHeader.vue'
import ResponsiveDataTable from '@/components/ResponsiveDataTable.vue'
import type { Device } from '@/stores/devices'

/** Espelha `MAX_SCAN_HOSTS` de `modules/discovery/cidr_range.ts` */
const MAX_SCAN_HOSTS = 1024

const discoveryStore = useDiscoveryStore()
const networksStore = useNetworksStore()
const devicesStore = useDevicesStore()
const tab = ref('results')
const selectedNetworkId = ref<number | null>(null)
const feedback = reactive({ visible: false, message: '', color: 'success' })
const deviceDialogOpen = ref(false)
const selectedResult = ref<DiscoveryResult | null>(null)
const resultDialogOpen = ref(false)
const selectedDetailResult = ref<DiscoveryResult | null>(null)
const cleanupLoading = ref(false)

const addedIpSet = computed(() => new Set(devicesStore.devices.map((d) => d.ipAddress).filter(Boolean)))

function isIpAdded(ip: string): boolean {
  return addedIpSet.value.has(ip)
}

const dialogPrefill = computed<Partial<Device> | null>(() => {
  if (!selectedResult.value) return null
  const result = selectedResult.value
  return {
    name: result.mdnsName || result.hostname || result.ipAddress,
    ipAddress: result.ipAddress,
    type: result.deviceType || 'other',
    vendor: result.vendor || undefined,
    macAddress: result.macAddress || undefined,
    siteId: result.discoveryRun?.network?.siteId ?? null,
    networkId: result.discoveryRun?.network?.id ?? null,
    isMonitored: true,
    snmpEnabled: false,
  }
})

const results = useInfiniteList<DiscoveryResult>(() => '/discovery/results/latest', {
  label: 'resultados de descoberta',
})
const runs = useInfiniteList<DiscoveryRun>(() => '/discovery/runs', {
  label: 'histórico de varreduras',
})

const resultHeaders = [
  { title: 'IP', key: 'ipAddress' },
  { title: 'MAC Address', key: 'macAddress' },
  { title: 'Nome Sugerido', key: 'suggestedName' },
  { title: 'Fabricante', key: 'vendor' },
  { title: 'Rede', key: 'network' },
  { title: 'Confiança', key: 'confidence', width: '140px' },
  { title: 'Ações', key: 'actions', sortable: false, width: '200px' },
]

const runHeaders = [
  { title: 'ID Run', key: 'id', width: '80px' },
  { title: 'Rede', key: 'network' },
  { title: 'Faixa', key: 'cidr' },
  { title: 'Dispositivos Encontrados', key: 'devicesFound', width: '120px' },
  { title: 'Status', key: 'status', width: '110px' },
  { title: 'Iniciado em', key: 'startedAt', width: '160px' },
]

/** Só faz sentido oferecer redes cujo CIDR o backend consegue expandir */
const scannableNetworks = computed(() =>
  networksStore.networks
    .filter((network) => network.scannable !== false)
    .map((network) => ({
      id: network.id,
      label: `${network.name} — ${network.cidr}`,
      usableHosts: network.usableHosts ?? 0,
    }))
)

const selectedNetworkHint = computed(() => {
  const selected = scannableNetworks.value.find((n) => n.id === selectedNetworkId.value)
  if (!selected) return 'A varredura roda no scheduler; os achados aparecem aqui.'

  return selected.usableHosts > MAX_SCAN_HOSTS
    ? `${selected.usableHosts} endereços na faixa — serão varridos os primeiros ${MAX_SCAN_HOSTS}.`
    : `${selected.usableHosts} endereço(s) serão varridos.`
})

onMounted(() => {
  networksStore.fetchNetworks()
  devicesStore.fetchDevices()
  refreshData()
})

function refreshData() {
  results.reset()
  runs.reset()
}

async function scanSelectedNetwork() {
  if (selectedNetworkId.value === null) return

  // Limpa os resultados anteriores imediatamente: após o scan só deve
  // aparecer o resultado da última varredura.
  results.reset()

  const result = await networksStore.scanNetwork(selectedNetworkId.value)
  if (!result) {
    feedback.color = 'error'
    feedback.message = networksStore.error || 'Não foi possível iniciar a varredura.'
    feedback.visible = true
    return
  }

  feedback.color = result.alreadyQueued ? 'warning' : 'success'
  feedback.message = result.message
  feedback.visible = true

  // A run nasce pendente: aparece já no histórico, e os resultados chegam
  // quando o scheduler a executar.
  runs.reset()
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

function handleAdd(item: DiscoveryResult) {
  selectedResult.value = item
  deviceDialogOpen.value = true
}

function openDetailDialog(item: DiscoveryResult) {
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

  // O backend já remove o discovery_result ao criar o device. Recarrega as
  // listas para refletir a alteração.
  await devicesStore.fetchDevices()
  results.reset()

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
    runs.reset()
  }
}
</script>
