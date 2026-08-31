<template>
  <v-card elevation="2" class="rounded-lg pa-3 pa-sm-4">
    <v-card-title class="font-weight-bold d-flex align-center">
      <v-icon start color="info">mdi-database-outline</v-icon>
      Banco de Dados
    </v-card-title>
    <v-card-text class="mt-2">
      <p class="text-caption text-grey-darken-1 mb-4">
        Tamanho ocupado pelo arquivo do banco de dados do NetMonitor no servidor.
      </p>

      <v-row dense>
        <v-col cols="12" sm="6">
          <div class="d-flex align-center justify-space-between pa-3 rounded border h-100">
            <div>
              <div class="font-weight-bold text-subtitle-2">Tamanho Total</div>
              <div class="text-caption text-grey">{{ formattedSize }}</div>
            </div>
            <v-icon color="info" size="28">mdi-harddisk</v-icon>
          </div>
        </v-col>

        <v-col cols="12" sm="6">
          <div class="d-flex align-center justify-space-between pa-3 rounded border h-100">
            <div>
              <div class="font-weight-bold text-subtitle-2">Tipo</div>
              <div class="text-caption text-grey">{{ dbTypeLabel }}</div>
            </div>
            <v-icon color="info" size="28">mdi-server-network</v-icon>
          </div>
        </v-col>
      </v-row>

      <v-alert
        v-if="error"
        type="error"
        variant="tonal"
        density="compact"
        class="mt-4"
        :text="error"
      ></v-alert>
    </v-card-text>
    <v-card-actions class="justify-end">
      <v-btn
        color="info"
        variant="tonal"
        size="small"
        prepend-icon="mdi-refresh"
        :loading="loading"
        @click="fetchDatabaseInfo"
      >
        Atualizar
      </v-btn>
    </v-card-actions>
  </v-card>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { apiService } from '@/services/apiService'
import type { DatabaseInfo } from '@/bindings/DatabaseInfo'

const loading = ref(false)
const error = ref('')
const info = ref<DatabaseInfo | null>(null)

const formattedSize = computed(() => {
  if (!info.value) return '—'
  return formatBytes(Number(info.value.sizeBytes))
})

const dbTypeLabel = computed(() => {
  if (!info.value) return '—'
  return info.value.dbType === 'sqlite' ? 'SQLite' : 'PostgreSQL'
})

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const k = 1024
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(k)), units.length - 1)
  const value = bytes / k ** i
  return `${value.toFixed(i === 0 ? 0 : 2)} ${units[i]}`
}

async function fetchDatabaseInfo(): Promise<void> {
  loading.value = true
  error.value = ''
  try {
    info.value = await apiService.get<DatabaseInfo>('/settings/database-size')
  } catch (erro) {
    error.value =
      erro instanceof Error ? erro.message : 'Não foi possível carregar o tamanho do banco.'
  } finally {
    loading.value = false
  }
}

onMounted(() => {
  void fetchDatabaseInfo()
})
</script>
