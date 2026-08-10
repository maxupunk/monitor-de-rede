<template>
  <div>
    <PageHeader
      title="Agentes Remotos (Probes)"
      subtitle="Status de probes distribuídos e gerenciamento de autenticação"
    >
      <template #actions>
        <v-btn color="primary" prepend-icon="mdi-refresh" @click="probesStore.fetchProbes()">
          <span class="hidden-sm-and-down">Atualizar Probes</span>
          <span class="hidden-md-and-up">Atualizar</span>
        </v-btn>
      </template>
    </PageHeader>

    <!-- Tabela de Probes -->
    <v-card elevation="2" class="mobile-full-bleed">
      <ResponsiveDataTable
        :headers="headers"
        :items="probesStore.probes"
        :loading="probesStore.loading"
        :items-per-page="-1"
        hide-default-footer
        no-data-text="Nenhum probe cadastrado ou conectado"
        :clickable="false"
      >
        <template #item.status="{ item }">
          <v-chip :color="getStatusColor(item.status)" size="small">
            {{ (item.status || 'UNKNOWN').toUpperCase() }}
          </v-chip>
        </template>

        <template #item.actions="{ item }">
          <div class="d-flex align-center ga-2">
            <v-btn
              size="small"
              color="primary"
              variant="outlined"
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

        <template #mobile-item="{ item }">
          <div class="d-flex flex-column ga-2">
            <div class="d-flex align-start justify-space-between ga-2">
              <div class="flex-grow-1 text-break">
                <div class="text-subtitle-2 font-weight-bold">{{ item.name }}</div>
                <div class="text-caption text-grey-darken-1">
                  {{ item.location || 'Sem localização' }} · {{ item.ipAddress || '—' }}
                </div>
                <div class="text-caption text-grey mt-1">
                  Último heartbeat: {{ item.lastHeartbeatAt || '—' }}
                </div>
              </div>
              <v-chip :color="getStatusColor(item.status)" size="small">
                {{ (item.status || 'UNKNOWN').toUpperCase() }}
              </v-chip>
            </div>
            <div class="d-flex align-center ga-2 mt-1">
              <v-btn
                size="small"
                color="primary"
                variant="outlined"
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
          </div>
        </template>
      </ResponsiveDataTable>
    </v-card>
  </div>
</template>

<script setup lang="ts">
import { onMounted } from 'vue'
import { useProbesStore } from '@/stores/probes'
import { getStatusColor } from '@/utils/monitorPresentation'
import PageHeader from '@/components/PageHeader.vue'
import ResponsiveDataTable from '@/components/ResponsiveDataTable.vue'

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

async function confirmRevoke(id: number) {
  if (confirm('Tem certeza de que deseja revogar o token deste Probe?')) {
    await probesStore.revokeProbe(id)
  }
}
</script>
