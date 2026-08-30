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
    <v-card elevation="2" rounded="lg">
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
            <!-- Top Row: Nome e Status Chip -->
            <div class="d-flex align-center justify-space-between ga-2">
              <span class="text-subtitle-1 font-weight-bold text-truncate">{{ item.name }}</span>
              <v-chip
                :color="getStatusColor(item.status)"
                size="small"
                variant="tonal"
                class="font-weight-bold px-2.5 flex-shrink-0"
              >
                {{ (item.status || 'UNKNOWN').toUpperCase() }}
              </v-chip>
            </div>

            <!-- Middle: Localização, IP e Heartbeat -->
            <div class="d-flex flex-column ga-1 text-caption text-grey">
              <div class="d-flex flex-wrap align-center ga-2">
                <span v-if="item.location" class="d-inline-flex align-center ga-1">
                  <v-icon size="13">mdi-map-marker-outline</v-icon>
                  {{ item.location }}
                </span>
                <span v-if="item.ipAddress" class="font-mono text-medium-emphasis">
                  {{ item.ipAddress }}
                </span>
              </div>
              <div class="d-flex align-center ga-1 text-grey-darken-1 mt-0.5">
                <v-icon size="12">mdi-heart-pulse</v-icon>
                <span>Último heartbeat: {{ item.lastHeartbeatAt || '—' }}</span>
              </div>
            </div>

            <!-- Footer Actions -->
            <div class="d-flex align-center justify-end ga-1.5 pt-2 mt-1 border-t">
              <v-btn
                size="small"
                color="primary"
                variant="tonal"
                prepend-icon="mdi-lightning-bolt"
                :loading="probesStore.testingId === item.id"
                class="text-caption px-2"
                @click="probesStore.testProbe(item.id)"
              >
                Testar
              </v-btn>
              <v-btn
                v-if="item.status !== 'revoked'"
                size="small"
                color="error"
                variant="tonal"
                prepend-icon="mdi-cancel"
                class="text-caption px-2"
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
import { confirm } from '@/composables/useConfirm'

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
  const ok = await confirm({
    title: 'Revogar token do Probe',
    message:
      'Tem certeza de que deseja revogar o token deste Probe? Ele perderá a comunicação com o servidor.',
    confirmText: 'Revogar token',
    confirmColor: 'error',
    icon: 'mdi-key-remove',
  })
  if (ok) {
    await probesStore.revokeProbe(id)
  }
}
</script>
