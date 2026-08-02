<template>
  <div>
    <div class="d-flex align-center justify-space-between mb-6">
      <div>
        <h1 class="text-h4 font-weight-bold">Agentes Remotos (Probes)</h1>
        <p class="text-subtitle-1 text-grey-darken-1">Status de probes distribuídos e gerenciamento de autenticação</p>
      </div>
      <v-btn color="primary" prepend-icon="mdi-refresh" @click="probesStore.fetchProbes()">
        Atualizar Probes
      </v-btn>
    </div>

    <!-- Tabela de Probes -->
    <v-card elevation="2" class="rounded-lg">
      <v-data-table
        :headers="headers"
        :items="probesStore.probes"
        :loading="probesStore.loading"
        no-data-text="Nenhum probe cadastrado ou conectado"
      >
        <template #item.status="{ item }">
          <v-chip :color="getStatusColor(item.status)" size="small">
            {{ (item.status || 'UNKNOWN').toUpperCase() }}
          </v-chip>
        </template>

        <template #item.actions="{ item }">
          <div class="d-flex ga-2" style="gap: 8px;">
            <v-btn
              size="small"
              color="secondary"
              variant="tonal"
              prepend-icon="mdi-lightning-bolt"
              :loading="probesStore.testingId === item.id"
              @click="probesStore.testProbe(item.id)"
            >
              Testar
            </v-btn>
            <v-btn
              v-if="item.status !== 'revoked'"
              size="small"
              color="error"
              variant="outlined"
              @click="confirmRevoke(item.id)"
            >
              Revogar
            </v-btn>
          </div>
        </template>
      </v-data-table>
    </v-card>
  </div>
</template>

<script setup lang="ts">
import { onMounted } from 'vue'
import { useProbesStore } from '@/stores/probes'

const probesStore = useProbesStore()

const headers = [
  { title: 'ID', key: 'id', width: '60px' },
  { title: 'Nome do Probe', key: 'name' },
  { title: 'Localização', key: 'location' },
  { title: 'Endereço IP', key: 'ipAddress' },
  { title: 'Status', key: 'status', width: '120px' },
  { title: 'Último Heartbeat', key: 'lastHeartbeatAt' },
  { title: 'Ações', key: 'actions', sortable: false, width: '220px' },
]

onMounted(() => {
  probesStore.fetchProbes()
})

function getStatusColor(status: string) {
  switch (status) {
    case 'online': return 'success'
    case 'offline': return 'error'
    case 'revoked': return 'grey'
    default: return 'warning'
  }
}

async function confirmRevoke(id: number) {
  if (confirm('Tem certeza de que deseja revogar o token deste Probe?')) {
    await probesStore.revokeProbe(id)
  }
}
</script>
