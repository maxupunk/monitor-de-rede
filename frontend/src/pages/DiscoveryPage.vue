<template>
  <div>
    <div class="d-flex align-center justify-space-between mb-6 flex-wrap ga-3">
      <div>
        <h1 class="text-h4 font-weight-bold">Central de Descoberta (Discovery)</h1>
        <p class="text-subtitle-1 text-grey-darken-1">
          Revise equipamentos encontrados na rede para aprovação ou mesclagem
        </p>
      </div>
      <v-btn color="primary" prepend-icon="mdi-refresh" @click="refreshData">
        Atualizar Descobertas
      </v-btn>
    </div>

    <!-- Disparo de varredura por faixa cadastrada em /networks -->
    <v-card elevation="2" class="rounded-lg mb-6 pa-4">
      <div class="d-flex align-center ga-4 flex-wrap">
        <v-icon color="secondary" size="28">mdi-radar</v-icon>
        <div class="flex-grow-1" style="min-width: 260px">
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
        <v-btn
          color="secondary"
          prepend-icon="mdi-radar"
          :disabled="selectedNetworkId === null"
          :loading="networksStore.scanningId !== null"
          @click="scanSelectedNetwork"
        >
          Escanear bloco
        </v-btn>
        <v-btn variant="text" prepend-icon="mdi-lan" to="/networks">Gerenciar redes</v-btn>
      </div>
    </v-card>

    <!-- Abas: Resultados & Execuções -->
    <v-card elevation="2" class="rounded-lg">
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
              <v-table hover density="comfortable" class="rounded-lg border">
                <thead>
                  <tr>
                    <th>IP</th>
                    <th>MAC Address</th>
                    <th>Nome Sugerido</th>
                    <th>Fabricante</th>
                    <th>Rede</th>
                    <th style="width: 140px">Confiança</th>
                    <th style="width: 110px">Status</th>
                    <th style="width: 200px">Ações</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="item in results.items.value" :key="item.id">
                    <td class="font-weight-medium">{{ item.ipAddress }}</td>
                    <td>{{ item.macAddress || '—' }}</td>
                    <td>{{ item.mdnsName || item.hostname || '—' }}</td>
                    <td>{{ item.vendor || '—' }}</td>
                    <td>
                      <span v-if="item.discoveryRun?.network" class="text-body-2">
                        {{ item.discoveryRun.network.name }}
                        <div class="text-caption text-grey">
                          {{ item.discoveryRun.network.cidr }}
                        </div>
                      </span>
                      <span v-else class="text-caption text-grey">—</span>
                    </td>
                    <td>
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
                    </td>
                    <td>
                      <v-chip :color="getStatusColor(item.status)" size="small" variant="tonal">
                        {{ (item.status || 'PENDING').toUpperCase() }}
                      </v-chip>
                    </td>
                    <td>
                      <div v-if="item.status === 'pending'" class="d-flex ga-2">
                        <v-btn
                          size="small"
                          color="success"
                          prepend-icon="mdi-check"
                          @click="handleAccept(item.id)"
                        >
                          Aceitar
                        </v-btn>
                        <v-btn
                          size="small"
                          color="grey"
                          variant="outlined"
                          @click="handleIgnore(item.id)"
                        >
                          Ignorar
                        </v-btn>
                      </div>
                      <span v-else class="text-caption text-grey">Processado</span>
                    </td>
                  </tr>
                </tbody>
              </v-table>
              <template #empty>
                <div class="text-caption text-grey text-center py-4">
                  Nenhum novo dispositivo pendente de aprovação.
                </div>
              </template>
            </v-infinite-scroll>
          </v-window-item>

          <!-- Histórico de Varreduras -->
          <v-window-item value="runs">
            <v-infinite-scroll :key="runs.scrollKey.value" @load="runs.load">
              <v-table hover density="comfortable" class="rounded-lg border">
                <thead>
                  <tr>
                    <th>ID Run</th>
                    <th>Rede</th>
                    <th>Faixa</th>
                    <th>Dispositivos Encontrados</th>
                    <th>Status</th>
                    <th>Iniciado em</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="item in runs.items.value" :key="item.id">
                    <td>#{{ item.id }}</td>
                    <td>{{ item.networkName || `Rede #${item.networkId}` }}</td>
                    <td>{{ item.cidr || '—' }}</td>
                    <td>{{ item.devicesFound }}</td>
                    <td>
                      <v-chip :color="runStatusColor(item.status)" size="small">
                        {{ runStatusLabel(item.status) }}
                      </v-chip>
                    </td>
                    <td>{{ formatDateTime(item.startedAt) }}</td>
                  </tr>
                </tbody>
              </v-table>
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
  </div>
</template>

<script setup lang="ts">
import { computed, ref, onMounted, reactive } from 'vue'
import { useDiscoveryStore, type DiscoveryResult, type DiscoveryRun } from '@/stores/discovery'
import { useNetworksStore } from '@/stores/networks'
import { useInfiniteList } from '@/composables/useInfiniteList'
import { getStatusColor } from '@/utils/monitorPresentation'
import { formatDateTime } from '@/utils/formatters'

/** Espelha `MAX_SCAN_HOSTS` de `modules/discovery/cidr_range.ts` */
const MAX_SCAN_HOSTS = 1024

const discoveryStore = useDiscoveryStore()
const networksStore = useNetworksStore()
const tab = ref('results')
const selectedNetworkId = ref<number | null>(null)
const feedback = reactive({ visible: false, message: '', color: 'success' })

const results = useInfiniteList<DiscoveryResult>(() => '/discovery/results?status=pending', {
  label: 'resultados de descoberta',
})
const runs = useInfiniteList<DiscoveryRun>(() => '/discovery/runs', {
  label: 'histórico de varreduras',
})

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
  refreshData()
})

function refreshData() {
  results.reset()
  runs.reset()
}

async function scanSelectedNetwork() {
  if (selectedNetworkId.value === null) return

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

async function handleAccept(id: number) {
  const ok = await discoveryStore.acceptResult(id)
  feedback.color = ok ? 'success' : 'error'
  feedback.message = ok
    ? 'Dispositivo criado e monitor de ping cadastrado.'
    : (discoveryStore.error ?? 'Não foi possível aceitar o resultado.')
  feedback.visible = true
  results.reset()
}

async function handleIgnore(id: number) {
  await discoveryStore.ignoreResult(id)
  results.reset()
}
</script>
