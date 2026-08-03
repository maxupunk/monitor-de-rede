<template>
  <div>
    <div class="d-flex align-center justify-space-between mb-6">
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

    <!-- Abas: Resultados & Execuções -->
    <v-card elevation="2" class="rounded-lg">
      <v-tabs v-model="tab" color="primary">
        <v-tab value="results">Resultados Encontrados ({{ discoveryStore.results.length }})</v-tab>
        <v-tab value="runs">Histórico de Escaneamento</v-tab>
      </v-tabs>
      <v-divider></v-divider>

      <v-card-text class="pa-4">
        <v-window v-model="tab">
          <!-- Resultados Encontrados -->
          <v-window-item value="results">
            <v-data-table
              :headers="resultsHeaders"
              :items="discoveryStore.results"
              :loading="discoveryStore.loading"
              no-data-text="Nenhum novo dispositivo pendente de aprovação"
            >
              <template #item.confidenceScore="{ item }">
                <v-progress-linear
                  :model-value="item.confidenceScore"
                  color="success"
                  height="18"
                  rounded
                >
                  <template #default="{ value }">
                    <strong class="text-caption text-white">{{ Math.ceil(value) }}%</strong>
                  </template>
                </v-progress-linear>
              </template>

              <template #item.status="{ item }">
                <v-chip :color="getStatusColor(item.status)" size="small" variant="tonal">
                  {{ (item.status || 'PENDING').toUpperCase() }}
                </v-chip>
              </template>

              <template #item.actions="{ item }">
                <div v-if="item.status === 'pending'" class="d-flex ga-2" style="gap: 8px">
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
              </template>
            </v-data-table>
          </v-window-item>

          <!-- Histórico de Varreduras -->
          <v-window-item value="runs">
            <v-data-table
              :headers="runsHeaders"
              :items="discoveryStore.runs"
              :loading="discoveryStore.loading"
              no-data-text="Nenhuma varredura recente"
            >
              <template #item.status="{ item }">
                <v-chip :color="item.status === 'completed' ? 'success' : 'warning'" size="small">
                  {{ item.status }}
                </v-chip>
              </template>
            </v-data-table>
          </v-window-item>
        </v-window>
      </v-card-text>
    </v-card>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useDiscoveryStore } from '@/stores/discovery'

const discoveryStore = useDiscoveryStore()
const tab = ref('results')

const resultsHeaders = [
  { title: 'IP', key: 'ipAddress' },
  { title: 'MAC Address', key: 'macAddress' },
  { title: 'Nome Sugerido', key: 'suggestedName' },
  { title: 'Fabricante', key: 'suggestedVendor' },
  { title: 'Confiança', key: 'confidenceScore', width: '140px' },
  { title: 'Status', key: 'status', width: '120px' },
  { title: 'Ações', key: 'actions', sortable: false, width: '200px' },
]

const runsHeaders = [
  { title: 'ID Run', key: 'id' },
  { title: 'Rede ID', key: 'networkId' },
  { title: 'Dispositivos Encontrados', key: 'devicesFound' },
  { title: 'Status', key: 'status' },
  { title: 'Iniciado em', key: 'startedAt' },
]

onMounted(() => {
  refreshData()
})

function refreshData() {
  discoveryStore.fetchDiscoveryResults()
  discoveryStore.fetchDiscoveryRuns()
}

function getStatusColor(status: string) {
  switch (status) {
    case 'accepted':
      return 'success'
    case 'ignored':
      return 'grey'
    case 'merged':
      return 'info'
    default:
      return 'warning'
  }
}

async function handleAccept(id: number) {
  await discoveryStore.acceptResult(id)
}

async function handleIgnore(id: number) {
  await discoveryStore.ignoreResult(id)
}
</script>
