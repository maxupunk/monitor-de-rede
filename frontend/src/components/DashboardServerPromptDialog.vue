<template>
  <v-dialog v-model="isOpen" max-width="640" persistent>
    <v-card class="rounded-lg pa-2">
      <v-card-item class="pb-2">
        <template #prepend>
          <v-avatar color="info" variant="tonal" size="48" class="mr-3">
            <v-icon size="28" color="info">mdi-cloud-sync-outline</v-icon>
          </v-avatar>
        </template>
        <v-card-title class="text-h6 font-weight-bold">
          Nova Organização do Dashboard Detectada
        </v-card-title>
        <v-card-subtitle class="text-wrap mt-1">
          A organização dos cards no servidor foi alterada. Escolha como prefere visualizar o
          Dashboard neste dispositivo:
        </v-card-subtitle>
      </v-card-item>

      <v-card-text class="pt-3">
        <v-row>
          <v-col cols="12" sm="6">
            <v-card
              variant="outlined"
              class="pa-4 h-100 rounded-lg cursor-pointer choice-card border-info"
              @click="choose('server')"
            >
              <div class="d-flex align-center ga-2 mb-2">
                <v-icon color="info">mdi-cloud-sync</v-icon>
                <span class="font-weight-bold text-subtitle-2">Usar Layout do Servidor</span>
              </div>
              <p class="text-caption text-grey-darken-1 mb-0">
                Sincroniza a organização e os gráficos em tempo real (via SSE) com todos os outros
                computadores e celulares conectados.
              </p>
            </v-card>
          </v-col>

          <v-col cols="12" sm="6">
            <v-card
              variant="outlined"
              class="pa-4 h-100 rounded-lg cursor-pointer choice-card"
              @click="choose('local')"
            >
              <div class="d-flex align-center ga-2 mb-2">
                <v-icon color="grey-darken-2">mdi-laptop</v-icon>
                <span class="font-weight-bold text-subtitle-2">Manter Layout Local</span>
              </div>
              <p class="text-caption text-grey-darken-1 mb-0">
                Mantém uma organização de cards exclusiva armazenada apenas neste navegador. Você
                pode alterar essa escolha nas Configurações.
              </p>
            </v-card>
          </v-col>
        </v-row>
      </v-card-text>

      <v-divider class="my-2"></v-divider>

      <v-card-actions class="px-4 pb-3 justify-end ga-2">
        <v-btn color="grey-darken-1" variant="text" size="small" @click="choose('local')">
          Usar Local
        </v-btn>
        <v-btn
          color="info"
          variant="flat"
          size="small"
          prepend-icon="mdi-cloud-check"
          @click="choose('server')"
        >
          Sincronizar com Servidor
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useDashboardStore, type SyncMode } from '@/stores/dashboard'

const dashboardStore = useDashboardStore()

const isOpen = computed({
  get: () => dashboardStore.showServerPrompt,
  set: (val) => {
    dashboardStore.showServerPrompt = val
  },
})

function choose(mode: SyncMode) {
  dashboardStore.chooseInitialSyncMode(mode)
}
</script>

<style scoped>
.choice-card {
  transition: all 0.2s ease-in-out;
}

.choice-card:hover {
  border-color: rgba(var(--v-theme-info), 0.8) !important;
  box-shadow: 0 4px 12px rgba(2, 132, 199, 0.15);
  transform: translateY(-2px);
}

.ga-2 {
  gap: 8px;
}
</style>
