<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 820"
    :fullscreen="$vuetify.display.xs"
    scrollable
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card v-if="connection" class="rounded-lg">
      <v-card-title class="font-weight-bold d-flex align-center pt-4 px-6">
        <v-icon start :color="engine.color">{{ engine.icon }}</v-icon>
        Cópias de {{ connection.name }}
      </v-card-title>
      <v-card-subtitle class="px-6 text-wrap">
        {{ engine.label }} em <code>{{ connection.host }}:{{ connection.port }}</code>
        <span v-if="connection.viaProbeId != null"> {{ routeLabel(connection) }}</span>
        <span v-if="connection.storageDestinationName">
          → {{ connection.storageDestinationName }}</span
        >
      </v-card-subtitle>

      <v-card-text class="px-6">
        <DatabaseJobProgress v-if="backupJob" :job="backupJob" class="mb-4" />

        <div class="d-flex flex-wrap align-center ga-2 mb-3">
          <v-btn
            color="success"
            variant="flat"
            prepend-icon="mdi-database-export-outline"
            :disabled="!connection.storageDestinationId || backupJob?.status === 'running'"
            @click="emit('backup')"
          >
            Fazer backup agora
          </v-btn>
          <v-btn
            color="primary"
            variant="tonal"
            prepend-icon="mdi-refresh"
            :loading="loading"
            @click="load"
          >
            Atualizar
          </v-btn>
          <v-spacer></v-spacer>
          <v-select
            v-if="databaseNames.length > 1"
            v-model="filter"
            :items="[
              { title: 'Todos os bancos', value: null },
              ...databaseNames.map((n) => ({ title: n, value: n })),
            ]"
            density="compact"
            variant="outlined"
            hide-details
            style="max-width: 220px"
          ></v-select>
        </div>

        <v-alert v-if="error" type="error" variant="tonal" density="compact" class="mb-3">
          {{ error }}
        </v-alert>
        <v-progress-linear v-if="loading" indeterminate color="primary" class="mb-2" />

        <div v-if="!loading && visible.length === 0" class="text-center py-8 border rounded-lg">
          <v-icon size="40" color="info">mdi-database-clock-outline</v-icon>
          <div class="font-weight-bold mt-2">Nenhuma cópia ainda</div>
          <div class="text-body-2 text-high-emphasis">
            Clique em "Fazer backup agora" para gerar a primeira.
          </div>
        </div>

        <v-list v-else lines="two" class="py-0 border rounded-lg">
          <v-list-item
            v-for="(item, index) in visible"
            :key="item.id"
            :class="{ 'border-t': index > 0 }"
          >
            <template #prepend>
              <v-avatar :color="statusColor(item.status)" variant="tonal" size="36">
                <v-icon size="20">{{ statusIcon(item.status) }}</v-icon>
              </v-avatar>
            </template>
            <v-list-item-title class="font-weight-medium">
              {{ item.databaseName }} · {{ formatDateTime(item.startedAt) }}
            </v-list-item-title>
            <v-list-item-subtitle class="text-high-emphasis">
              <template v-if="item.status === 'failed'">
                <span class="text-error">{{ item.error }}</span>
              </template>
              <template v-else>{{ details(item) }}</template>
            </v-list-item-subtitle>
            <template #append>
              <div class="d-flex align-center ga-1">
                <v-icon v-if="item.warnings.length" color="warning" size="20">
                  mdi-alert-outline
                  <v-tooltip activator="parent" location="top" max-width="320">
                    <div v-for="warning in item.warnings" :key="warning">{{ warning }}</div>
                  </v-tooltip>
                </v-icon>
                <v-btn
                  v-if="item.status === 'success' && item.objectKey && item.storageDestinationId"
                  icon
                  size="small"
                  variant="text"
                  color="primary"
                  :loading="downloading === item.id"
                  @click="download(item)"
                >
                  <v-icon>mdi-download</v-icon>
                  <v-tooltip activator="parent" location="top">Baixar .sql.gz</v-tooltip>
                </v-btn>
                <v-btn
                  v-if="item.status === 'success'"
                  size="small"
                  variant="tonal"
                  color="warning"
                  prepend-icon="mdi-database-import-outline"
                  @click="emit('restore', item)"
                >
                  <span class="hidden-xs">Restaurar</span>
                </v-btn>
              </div>
            </template>
          </v-list-item>
        </v-list>
      </v-card-text>

      <v-card-actions class="px-6 pb-4">
        <v-spacer></v-spacer>
        <v-btn variant="text" @click="emit('update:modelValue', false)">Fechar</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { DatabaseBackupResponse } from '@/bindings/DatabaseBackupResponse'
import type { DatabaseConnectionResponse } from '@/bindings/DatabaseConnectionResponse'
import DatabaseJobProgress from './DatabaseJobProgress.vue'
import { useDatabasesStore } from '@/stores/databases'
import { useStoragesStore } from '@/stores/storages'
import { engineInfo, routeLabel } from '@/utils/databasePresentation'
import {
  formatBytes,
  formatCompactCount,
  formatDateTime,
  formatElapsedMs,
} from '@/utils/formatters'

const props = defineProps<{
  modelValue: boolean
  connection: DatabaseConnectionResponse | null
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  (e: 'backup'): void
  (e: 'restore', value: DatabaseBackupResponse): void
}>()

const databasesStore = useDatabasesStore()
const storagesStore = useStoragesStore()

const backups = ref<DatabaseBackupResponse[]>([])
const loading = ref(false)
const error = ref<string | null>(null)
const downloading = ref<number | null>(null)
const filter = ref<string | null>(null)

const engine = computed(() => engineInfo(props.connection?.engine ?? 'postgres'))
const backupJob = computed(() =>
  props.connection ? databasesStore.jobFor('backup', props.connection.id) : null
)
const databaseNames = computed(() =>
  [...new Set(backups.value.map((item) => item.databaseName))].sort()
)
const visible = computed(() =>
  filter.value ? backups.value.filter((item) => item.databaseName === filter.value) : backups.value
)

async function load() {
  if (!props.connection) return
  loading.value = true
  error.value = null
  try {
    backups.value = await databasesStore.history(props.connection.id)
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Erro ao carregar o histórico'
  } finally {
    loading.value = false
  }
}

watch(
  () => props.modelValue,
  (open) => {
    if (!open) return
    backups.value = []
    filter.value = null
    void load()
  }
)

// O backup em andamento terminou: a cópia nova entra na lista.
watch(
  () => backupJob.value?.status,
  (status, previous) => {
    if (props.modelValue && previous === 'running' && status !== 'running') void load()
  }
)

function statusColor(status: string): string {
  if (status === 'success') return 'success'
  if (status === 'failed') return 'error'
  return 'info'
}

function statusIcon(status: string): string {
  if (status === 'success') return 'mdi-check'
  if (status === 'failed') return 'mdi-close'
  return 'mdi-progress-clock'
}

function details(item: DatabaseBackupResponse): string {
  if (item.status === 'running') return 'Em andamento…'
  return [
    item.sizeBytes != null ? formatBytes(item.sizeBytes) : null,
    `${item.tables} tabelas`,
    `${formatCompactCount(item.rows)} linhas`,
    formatElapsedMs(item.durationMs),
    item.trigger === 'scheduled' ? 'automático' : 'manual',
  ]
    .filter(Boolean)
    .join(' · ')
}

async function download(item: DatabaseBackupResponse) {
  if (!item.objectKey || item.storageDestinationId == null) return
  downloading.value = item.id
  try {
    const name = item.objectKey.split('/').pop() ?? 'backup.sql.gz'
    await storagesStore.downloadObject(item.storageDestinationId, item.objectKey, name)
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Erro ao baixar a cópia'
  } finally {
    downloading.value = null
  }
}
</script>
