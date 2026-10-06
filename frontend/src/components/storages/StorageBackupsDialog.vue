<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 760"
    :fullscreen="$vuetify.display.xs"
    scrollable
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card v-if="storage" class="rounded-lg">
      <v-card-title class="font-weight-bold d-flex align-center pt-4 px-6">
        <v-icon start :color="info.color">{{ info.icon }}</v-icon>
        Cópias em {{ storage.name }}
      </v-card-title>
      <v-card-subtitle class="px-6 text-wrap">
        <code>{{ storage.target ?? '—' }}</code>
      </v-card-subtitle>

      <v-card-text class="px-6">
        <!-- Prévia: o passo entre "escolhi a cópia" e "apaguei a configuração atual". -->
        <v-alert
          v-if="pending"
          type="warning"
          variant="tonal"
          border="start"
          class="mb-4"
          icon="mdi-database-import-outline"
        >
          <div class="font-weight-bold mb-1">Restaurar {{ pending.name }}?</div>
          <div class="text-body-2 mb-3">
            Toda a configuração atual — dispositivos, monitores, regras, VPN — é substituída pela
            desta cópia, e o histórico de coleta dos equipamentos atuais é descartado. Contas de
            acesso não mudam.
          </div>
          <BackupCountsSummary
            v-if="pending.counts"
            :name="pending.name"
            :counts="pending.counts"
            class="mb-3"
          />
          <div class="d-flex flex-wrap ga-2">
            <v-btn
              color="error"
              variant="flat"
              prepend-icon="mdi-database-import-outline"
              :loading="restoring"
              :disabled="!pending.counts"
              @click="restore"
            >
              Restaurar esta cópia
            </v-btn>
            <v-btn variant="text" :disabled="restoring" @click="pending = null">Cancelar</v-btn>
          </div>
        </v-alert>

        <v-alert v-if="restored" type="success" variant="tonal" density="compact" class="mb-4">
          Configuração restaurada — {{ restored.totalRows }} registros aplicados.
        </v-alert>
        <v-alert
          v-if="error"
          type="error"
          variant="tonal"
          density="compact"
          class="mb-4"
          closable
          @click:close="error = null"
        >
          {{ error }}
        </v-alert>

        <div class="d-flex flex-wrap align-center ga-2 mb-3">
          <v-btn
            color="success"
            variant="flat"
            prepend-icon="mdi-cloud-upload-outline"
            :loading="storagesStore.running.includes(storage.id)"
            @click="backupNow"
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
          <span class="text-caption text-high-emphasis">
            {{ backups.length }} {{ backups.length === 1 ? 'cópia' : 'cópias' }} · mantém
            {{ storage.backupRetention }}
          </span>
        </div>

        <v-progress-linear v-if="loading" indeterminate color="primary" class="mb-2" />

        <div v-if="!loading && backups.length === 0" class="text-center py-8 border rounded-lg">
          <v-icon size="40" color="info">mdi-archive-outline</v-icon>
          <div class="font-weight-bold mt-2">Nenhuma cópia neste armazenamento ainda</div>
          <div class="text-body-2 text-high-emphasis">
            Clique em "Fazer backup agora" para enviar a primeira.
          </div>
        </div>

        <v-list v-else lines="two" class="py-0 border rounded-lg">
          <v-list-item
            v-for="(backup, index) in backups"
            :key="backup.key"
            :title="formatDateTime(backup.lastModified ?? stampOf(backup.name))"
            :subtitle="subtitleOf(backup)"
            :class="{ 'border-t': index > 0 }"
          >
            <template #prepend>
              <v-avatar :color="index === 0 ? 'success' : 'info'" variant="tonal" size="36">
                <v-icon size="20">{{
                  index === 0 ? 'mdi-star-outline' : 'mdi-file-cog-outline'
                }}</v-icon>
              </v-avatar>
            </template>
            <template #append>
              <div class="d-flex ga-1">
                <v-btn
                  icon
                  size="small"
                  variant="text"
                  color="primary"
                  :loading="downloading === backup.key"
                  @click="download(backup)"
                >
                  <v-icon>mdi-download</v-icon>
                  <v-tooltip activator="parent" location="top">Baixar</v-tooltip>
                </v-btn>
                <v-btn
                  size="small"
                  variant="tonal"
                  color="warning"
                  prepend-icon="mdi-database-import-outline"
                  :loading="previewing === backup.key"
                  @click="askRestore(backup)"
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
import type { BackupCountsResponse } from '@/bindings/BackupCountsResponse'
import type { StorageBackupResponse } from '@/bindings/StorageBackupResponse'
import type { StorageDestinationResponse } from '@/bindings/StorageDestinationResponse'
import BackupCountsSummary from '@/components/settings/BackupCountsSummary.vue'
import { useStoragesStore } from '@/stores/storages'
import { formatBytes, formatDateTime, formatRelativeTime } from '@/utils/formatters'
import { providerInfo } from '@/utils/storagePresentation'

const props = defineProps<{
  modelValue: boolean
  storage: StorageDestinationResponse | null
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  (e: 'restored', value: BackupCountsResponse): void
}>()

const storagesStore = useStoragesStore()

const backups = ref<StorageBackupResponse[]>([])
const loading = ref(false)
const error = ref<string | null>(null)
const downloading = ref<string | null>(null)
const previewing = ref<string | null>(null)
const restoring = ref(false)
const restored = ref<BackupCountsResponse | null>(null)
const pending = ref<{ key: string; name: string; counts: BackupCountsResponse | null } | null>(null)

const info = computed(() => providerInfo(props.storage?.provider ?? 'local'))

function fail(err: unknown, fallback: string) {
  error.value = err instanceof Error ? err.message : fallback
}

async function load() {
  if (!props.storage) return
  loading.value = true
  error.value = null
  try {
    backups.value = await storagesStore.listBackups(props.storage.id)
  } catch (err) {
    fail(err, 'Erro ao listar as cópias')
  } finally {
    loading.value = false
  }
}

watch(
  () => props.modelValue,
  (open) => {
    if (!open) return
    backups.value = []
    pending.value = null
    restored.value = null
    error.value = null
    void load()
  }
)

/** `netmonitor-backup-20260307-040506.json` → instante, quando o provider não dá a data. */
function stampOf(name: string): string | null {
  const match = /(\d{4})(\d{2})(\d{2})-(\d{2})(\d{2})(\d{2})/.exec(name)
  if (!match) return null
  const [, y, mo, d, h, mi, s] = match
  return `${y}-${mo}-${d}T${h}:${mi}:${s}Z`
}

function subtitleOf(backup: StorageBackupResponse): string {
  const when = backup.lastModified ?? stampOf(backup.name)
  const size = backup.size != null ? formatBytes(backup.size) : null
  return [formatRelativeTime(when), size].filter(Boolean).join(' · ')
}

async function backupNow() {
  if (!props.storage) return
  error.value = null
  try {
    await storagesStore.runBackup(props.storage.id)
    await load()
  } catch (err) {
    fail(err, 'Erro ao fazer o backup')
  }
}

async function download(backup: StorageBackupResponse) {
  if (!props.storage) return
  downloading.value = backup.key
  try {
    await storagesStore.downloadObject(props.storage.id, backup.key, backup.name)
  } catch (err) {
    fail(err, 'Erro ao baixar a cópia')
  } finally {
    downloading.value = null
  }
}

async function askRestore(backup: StorageBackupResponse) {
  if (!props.storage) return
  previewing.value = backup.key
  restored.value = null
  error.value = null
  try {
    const counts = await storagesStore.previewBackup(props.storage.id, backup.key)
    pending.value = { key: backup.key, name: backup.name, counts }
  } catch (err) {
    fail(err, 'Erro ao ler a cópia')
  } finally {
    previewing.value = null
  }
}

async function restore() {
  if (!props.storage || !pending.value) return
  restoring.value = true
  error.value = null
  try {
    restored.value = await storagesStore.restoreBackup(props.storage.id, pending.value.key)
    pending.value = null
    emit('restored', restored.value)
  } catch (err) {
    fail(err, 'Erro ao restaurar a cópia')
  } finally {
    restoring.value = false
  }
}
</script>
