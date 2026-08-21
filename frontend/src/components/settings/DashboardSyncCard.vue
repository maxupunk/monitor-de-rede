<template>
  <v-card elevation="2" class="rounded-lg pa-4">
    <v-card-title class="font-weight-bold d-flex align-center">
      <v-icon start color="info">mdi-view-dashboard-variant-outline</v-icon>
      Sincronização do Dashboard
    </v-card-title>
    <v-card-text class="mt-2">
      <p class="text-caption text-grey-darken-1 mb-3">
        Escolha se deseja utilizar a organização de cards compartilhada no servidor ou manter uma
        distribuição customizada apenas neste navegador.
      </p>

      <v-radio-group
        :model-value="dashboardStore.syncMode"
        color="info"
        hide-details
        @update:model-value="(val) => dashboardStore.setSyncMode(val as SyncMode)"
      >
        <v-radio value="server">
          <template #label>
            <div>
              <div class="font-weight-bold text-subtitle-2">
                Sincronizado com o Servidor (Global)
              </div>
              <div class="text-caption text-grey">
                Recebe atualizações de layout em tempo real (SSE) emitidas por qualquer dispositivo.
              </div>
            </div>
          </template>
        </v-radio>

        <v-radio value="local" class="mt-3">
          <template #label>
            <div>
              <div class="font-weight-bold text-subtitle-2">Personalizado Localmente</div>
              <div class="text-caption text-grey">
                Mantém a disposição dos cards exclusiva deste navegador/dispositivo.
              </div>
            </div>
          </template>
        </v-radio>
      </v-radio-group>
    </v-card-text>
    <v-card-actions class="justify-space-between flex-wrap ga-2 pt-0">
      <v-btn
        color="info"
        variant="tonal"
        size="small"
        prepend-icon="mdi-cloud-download"
        :loading="dashboardStore.loadingServer"
        @click="dashboardStore.fetchServerLayout"
      >
        Baixar do Servidor
      </v-btn>
      <v-btn
        color="primary"
        variant="flat"
        size="small"
        prepend-icon="mdi-cloud-upload"
        :loading="dashboardStore.savingGlobal"
        @click="dashboardStore.saveLayoutGlobally"
      >
        Salvar como Padrão Global
      </v-btn>
    </v-card-actions>
  </v-card>
</template>

<script setup lang="ts">
import { useDashboardStore, type SyncMode } from '@/stores/dashboard'

const dashboardStore = useDashboardStore()
</script>
