<template>
  <v-card elevation="2" class="rounded-lg pa-3 pa-sm-4">
    <v-card-title class="font-weight-bold d-flex align-center">
      <v-icon start color="info">mdi-database-outline</v-icon>
      Banco de Dados
    </v-card-title>
    <v-card-text class="mt-2">
      <p class="text-caption text-grey-darken-1 mb-4">
        Tamanho ocupado pelo arquivo do banco de dados do NetMonitor no servidor e intervalo
        histórico armazenado.
      </p>

      <v-row dense>
        <v-col cols="12" sm="6" md="4">
          <div class="d-flex align-center justify-space-between pa-3 rounded border h-100">
            <div>
              <div class="font-weight-bold text-subtitle-2">Tamanho Total</div>
              <div class="text-caption text-grey">{{ formattedSize }}</div>
            </div>
            <v-icon color="info" size="28">mdi-harddisk</v-icon>
          </div>
        </v-col>

        <v-col cols="12" sm="6" md="4">
          <div class="d-flex align-center justify-space-between pa-3 rounded border h-100">
            <div>
              <div class="font-weight-bold text-subtitle-2">Tipo</div>
              <div class="text-caption text-grey">{{ dbTypeLabel }}</div>
            </div>
            <v-icon color="info" size="28">mdi-server-network</v-icon>
          </div>
        </v-col>

        <v-col cols="12" sm="6" md="4">
          <div class="d-flex align-center justify-space-between pa-3 rounded border h-100">
            <div>
              <div class="font-weight-bold text-subtitle-2">Total de Histórico</div>
              <div class="text-caption text-grey">{{ formattedHistorySpan }}</div>
            </div>
            <v-icon color="info" size="28">mdi-history</v-icon>
          </div>
        </v-col>

        <v-col cols="12" sm="6" md="6">
          <div class="d-flex align-center justify-space-between pa-3 rounded border h-100">
            <div>
              <div class="font-weight-bold text-subtitle-2">Primeiro Registro</div>
              <div class="text-caption text-grey">{{ formattedEarliestRecord }}</div>
            </div>
            <v-icon color="info" size="28">mdi-calendar-start-outline</v-icon>
          </div>
        </v-col>

        <v-col cols="12" sm="6" md="6">
          <div class="d-flex align-center justify-space-between pa-3 rounded border h-100">
            <div>
              <div class="font-weight-bold text-subtitle-2">Último Registro</div>
              <div class="text-caption text-grey">{{ formattedLatestRecord }}</div>
            </div>
            <v-icon color="info" size="28">mdi-calendar-end-outline</v-icon>
          </div>
        </v-col>
      </v-row>

      <v-alert
        v-if="successMessage"
        type="success"
        variant="tonal"
        density="compact"
        class="mt-4"
        closable
        @click:close="successMessage = ''"
      >
        {{ successMessage }}
      </v-alert>

      <v-alert
        v-if="error"
        type="error"
        variant="tonal"
        density="compact"
        class="mt-4"
        :text="error"
      ></v-alert>
    </v-card-text>

    <v-card-actions class="d-flex justify-space-between align-center flex-wrap ga-2">
      <div class="d-flex align-center flex-wrap ga-2">
        <v-btn
          color="error"
          variant="tonal"
          size="small"
          prepend-icon="mdi-delete-sweep-outline"
          :disabled="loading || clearing || clearingAll"
          @click="confirmClearDialog = true"
        >
          Apagar Histórico
        </v-btn>

        <v-btn
          color="error"
          variant="tonal"
          size="small"
          prepend-icon="mdi-database-remove-outline"
          :disabled="loading || clearing || clearingAll"
          @click="confirmClearAllDialog = true"
        >
          Apagar Todos os Itens
        </v-btn>
      </div>

      <v-btn
        color="info"
        variant="tonal"
        size="small"
        prepend-icon="mdi-refresh"
        :loading="loading"
        :disabled="clearing || clearingAll"
        @click="fetchDatabaseInfo"
      >
        Atualizar
      </v-btn>
    </v-card-actions>

    <v-dialog
      v-model="confirmClearDialog"
      max-width="540"
      :fullscreen="Boolean($vuetify?.display?.xs)"
    >
      <v-card class="rounded-lg">
        <v-card-title class="font-weight-bold d-flex align-center text-error">
          <v-icon start color="error">mdi-alert-outline</v-icon>
          Apagar Histórico do Banco de Dados
        </v-card-title>
        <v-card-text>
          <p class="mb-3">
            Esta ação apagará <strong>permanentemente</strong> todo o histórico técnico armazenado
            para liberar espaço em disco:
          </p>
          <ul class="text-body-2 pl-4 mb-3 text-grey-darken-2">
            <li>Séries temporais de métricas de telemetria</li>
            <li>Resultados históricos de monitores (ping, SNMP, portas, etc.)</li>
            <li>Histórico de logs de dispositivos (Syslog)</li>
            <li>Histórico de eventos de alertas passados</li>
          </ul>
          <v-alert type="info" variant="tonal" density="compact" class="mb-0">
            <strong>Cadastros preservados:</strong> Seus dispositivos, monitores, regras de alerta,
            redes, topologia, sites, usuários e preferências <strong>não</strong> serão alterados.
          </v-alert>
        </v-card-text>
        <v-card-actions class="justify-end">
          <v-btn variant="text" :disabled="clearing" @click="confirmClearDialog = false">
            Cancelar
          </v-btn>
          <v-btn
            color="error"
            variant="flat"
            :loading="clearing"
            prepend-icon="mdi-delete-sweep-outline"
            @click="clearHistory"
          >
            Confirmar e Apagar
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <v-dialog
      v-model="confirmClearAllDialog"
      max-width="560"
      :fullscreen="Boolean($vuetify?.display?.xs)"
    >
      <v-card class="rounded-lg">
        <v-card-title class="font-weight-bold d-flex align-center text-error">
          <v-icon start color="error">mdi-alert-octagon-outline</v-icon>
          Apagar Todos os Itens e Cadastros
        </v-card-title>
        <v-card-text>
          <p class="mb-3">
            Esta ação apagará <strong>permanentemente todos os cadastros e dados</strong> do
            sistema:
          </p>
          <ul class="text-body-2 pl-4 mb-3 text-grey-darken-2">
            <li>Todos os dispositivos, interfaces e enlaces de topologia</li>
            <li>Todos os monitores, métricas e histórico de coletas</li>
            <li>Todas as redes e histórico de descoberta (Discovery)</li>
            <li>Regras de alerta customizadas, eventos e notificações</li>
            <li>Sites, servidores e peers de VPN</li>
            <li>Histórico de logs de dispositivos (Syslog)</li>
          </ul>
          <v-alert type="warning" variant="tonal" density="compact" class="mb-3">
            <strong>Atenção:</strong> O inventário voltará ao estado inicial limpo. O dispositivo do
            servidor local (<em>Servidor NetMonitor</em>) e o monitor de saúde serão recriados
            automaticamente.
          </v-alert>
          <v-alert type="info" variant="tonal" density="compact" class="mb-0">
            <strong>Usuários preservados:</strong> Sua conta de administrador e credenciais
            <strong>não</strong> serão afetadas (você continua logado).
          </v-alert>
        </v-card-text>
        <v-card-actions class="justify-end">
          <v-btn variant="text" :disabled="clearingAll" @click="confirmClearAllDialog = false">
            Cancelar
          </v-btn>
          <v-btn
            color="error"
            variant="flat"
            :loading="clearingAll"
            prepend-icon="mdi-database-remove-outline"
            @click="clearAllItems"
          >
            Confirmar e Apagar Tudo
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </v-card>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { apiService } from '@/services/apiService'
import type { DatabaseInfo } from '@/bindings/DatabaseInfo'
import type { ClearHistoryStats } from '@/bindings/ClearHistoryStats'
import type { ClearAllItemsStats } from '@/bindings/ClearAllItemsStats'
import { formatBinaryBytes, formatDateTime, formatTimeSpan } from '@/utils/formatters'

const loading = ref(false)
const clearing = ref(false)
const clearingAll = ref(false)
const error = ref('')
const successMessage = ref('')
const confirmClearDialog = ref(false)
const confirmClearAllDialog = ref(false)
const info = ref<DatabaseInfo | null>(null)

const formattedSize = computed(() => {
  if (!info.value) return '—'
  return formatBinaryBytes(Number(info.value.sizeBytes))
})

const dbTypeLabel = computed(() => {
  if (!info.value) return '—'
  return info.value.dbType === 'sqlite' ? 'SQLite' : 'PostgreSQL'
})

const formattedEarliestRecord = computed(() => {
  if (!info.value?.earliestRecord) return '—'
  return formatDateTime(info.value.earliestRecord)
})

const formattedLatestRecord = computed(() => {
  if (!info.value?.latestRecord) return '—'
  return formatDateTime(info.value.latestRecord)
})

const formattedHistorySpan = computed(() => {
  if (!info.value?.earliestRecord || !info.value?.latestRecord) return '—'
  return formatTimeSpan(info.value.earliestRecord, info.value.latestRecord)
})

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

async function clearHistory(): Promise<void> {
  clearing.value = true
  error.value = ''
  successMessage.value = ''
  try {
    const stats = await apiService.post<ClearHistoryStats>('/settings/clear-history', {})
    confirmClearDialog.value = false
    const count = Number(stats.totalDeleted)
    successMessage.value = `Histórico apagado com sucesso! ${count.toLocaleString('pt-BR')} registros removidos e espaço em disco liberado.`
    await fetchDatabaseInfo()
  } catch (erro) {
    error.value =
      erro instanceof Error ? erro.message : 'Não foi possível apagar o histórico do banco.'
  } finally {
    clearing.value = false
  }
}

async function clearAllItems(): Promise<void> {
  clearingAll.value = true
  error.value = ''
  successMessage.value = ''
  try {
    const stats = await apiService.post<ClearAllItemsStats>('/settings/clear-all-items', {})
    confirmClearAllDialog.value = false
    const devCount = Number(stats.devicesDeleted)
    const monCount = Number(stats.monitorsDeleted)
    const netCount = Number(stats.networksDeleted)
    successMessage.value = `Todos os itens foram apagados com sucesso! Removidos: ${devCount.toLocaleString('pt-BR')} dispositivo(s), ${monCount.toLocaleString('pt-BR')} monitor(es) e ${netCount.toLocaleString('pt-BR')} rede(s). Dispositivo local padrão restaurado.`
    await fetchDatabaseInfo()
  } catch (erro) {
    error.value =
      erro instanceof Error ? erro.message : 'Não foi possível apagar todos os itens do banco.'
  } finally {
    clearingAll.value = false
  }
}

onMounted(() => {
  void fetchDatabaseInfo()
})
</script>
