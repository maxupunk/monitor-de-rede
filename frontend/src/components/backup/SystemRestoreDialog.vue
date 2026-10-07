<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 760"
    :fullscreen="$vuetify.display.xs"
    scrollable
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card class="rounded-lg">
      <v-card-title class="font-weight-bold d-flex align-center pt-4 px-6">
        <v-icon start color="primary">mdi-backup-restore</v-icon>
        Restaurar o NetMonitor
      </v-card-title>
      <v-card-subtitle class="px-6 text-wrap">
        Escolha a cópia que deve voltar. Antes de aplicar, você vê o que ela contém.
      </v-card-subtitle>

      <v-card-text class="px-6">
        <v-btn-toggle
          v-model="source"
          color="primary"
          variant="outlined"
          divided
          mandatory
          density="comfortable"
          class="mb-4"
        >
          <v-btn value="destination" prepend-icon="mdi-database-lock-outline">
            De um destino
          </v-btn>
          <v-btn value="file" prepend-icon="mdi-file-upload-outline">De um arquivo</v-btn>
        </v-btn-toggle>

        <SystemRestoreConfirm
          v-if="pending"
          :name="pending.name"
          :counts="pending.counts"
          :loading="restoring"
          class="mb-4"
          @confirm="restore"
          @cancel="pending = null"
        />

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

        <!-- Cópia guardada num destino -->
        <template v-if="source === 'destination'">
          <v-select
            v-model="storageId"
            :items="storageItems"
            item-title="name"
            item-value="id"
            label="Destino"
            variant="outlined"
            no-data-text="Nenhum destino cadastrado"
            class="mb-2"
          ></v-select>

          <v-progress-linear v-if="loading" indeterminate color="primary" class="mb-2" />

          <div
            v-if="!loading && storageId != null && copies.length === 0"
            class="text-center py-8 border rounded-lg"
          >
            <v-icon size="40" color="info">mdi-archive-outline</v-icon>
            <div class="font-weight-bold mt-2">Nenhuma cópia do NetMonitor neste destino</div>
          </div>

          <v-list v-else-if="copies.length" lines="two" class="py-0 border rounded-lg">
            <v-list-item
              v-for="(copy, index) in copies"
              :key="copy.key"
              :title="formatDateTime(copy.lastModified ?? stampOf(copy.name))"
              :subtitle="subtitleOf(copy, index)"
              :class="{ 'border-t': index > 0 }"
            >
              <template #prepend>
                <v-avatar :color="index === 0 ? 'success' : 'info'" variant="tonal" size="36">
                  <v-icon size="20">{{ index === 0 ? 'mdi-star-outline' : 'mdi-history' }}</v-icon>
                </v-avatar>
              </template>
              <template #append>
                <div class="d-flex ga-1">
                  <v-btn
                    icon
                    size="small"
                    variant="text"
                    color="primary"
                    :loading="downloading === copy.key"
                    @click="download(copy)"
                  >
                    <v-icon>mdi-download</v-icon>
                    <v-tooltip activator="parent" location="top">Baixar arquivo</v-tooltip>
                  </v-btn>
                  <v-btn
                    size="small"
                    variant="tonal"
                    color="warning"
                    :loading="previewing === copy.key"
                    @click="askCopy(copy)"
                  >
                    Restaurar
                  </v-btn>
                </div>
              </template>
            </v-list-item>
          </v-list>
        </template>

        <!-- Arquivo do computador -->
        <template v-else>
          <p class="text-body-2 text-high-emphasis mb-3">
            Um arquivo <code>netmonitor-backup-….json</code> baixado antes — pelo "Baixar a
            configuração" ou de um destino.
          </p>
          <v-file-input
            v-model="file"
            label="Arquivo de backup (.json)"
            accept="application/json,.json"
            variant="outlined"
            prepend-icon=""
            prepend-inner-icon="mdi-file-upload-outline"
            :loading="previewing === 'file'"
            hide-details
            @update:model-value="onFile"
          ></v-file-input>
        </template>
      </v-card-text>

      <v-card-actions class="px-6 pb-4">
        <v-spacer></v-spacer>
        <v-btn variant="text" color="primary" @click="emit('update:modelValue', false)">
          Fechar
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { BackupCountsResponse } from '@/bindings/BackupCountsResponse'
import type { SystemBackupCopyResponse } from '@/bindings/SystemBackupCopyResponse'
import SystemRestoreConfirm from './SystemRestoreConfirm.vue'
import { reloadAfterRestore, useBackupStore } from '@/stores/backup'
import { useStoragesStore } from '@/stores/storages'
import { formatBytes, formatDateTime, formatRelativeTime } from '@/utils/formatters'

const props = defineProps<{ modelValue: boolean }>()
const emit = defineEmits<{ (e: 'update:modelValue', value: boolean): void }>()

const backupStore = useBackupStore()
const storagesStore = useStoragesStore()

const source = ref<'destination' | 'file'>('destination')
const storageId = ref<number | null>(null)
const copies = ref<SystemBackupCopyResponse[]>([])
const file = ref<File | File[] | null>(null)
const loading = ref(false)
const downloading = ref<string | null>(null)
const previewing = ref<string | null>(null)
const restoring = ref(false)
const error = ref<string | null>(null)
/** A cópia escolhida, com o que ela contém. `key` nulo é o arquivo enviado. */
const pending = ref<{ name: string; key: string | null; counts: BackupCountsResponse } | null>(null)

const storageItems = computed(() =>
  storagesStore.storages.map((storage) => ({ id: storage.id, name: storage.name }))
)

function fail(err: unknown, fallback: string) {
  error.value = err instanceof Error ? err.message : fallback
}

async function load() {
  copies.value = []
  if (storageId.value == null) return
  loading.value = true
  error.value = null
  try {
    copies.value = await backupStore.listCopies(storageId.value)
  } catch (err) {
    fail(err, 'Erro ao listar as cópias')
  } finally {
    loading.value = false
  }
}

watch(
  () => props.modelValue,
  async (open) => {
    if (!open) return
    pending.value = null
    error.value = null
    file.value = null
    backupStore.clearFile()
    source.value = 'destination'
    if (!storagesStore.loaded) await storagesStore.fetchStorages()
    storageId.value =
      backupStore.plan?.storageDestinationId ?? storagesStore.storages[0]?.id ?? null
    void load()
  }
)

watch(storageId, (current, previous) => {
  if (props.modelValue && previous !== undefined && current !== previous) {
    pending.value = null
    void load()
  }
})

watch(source, () => {
  pending.value = null
  error.value = null
})

/** `netmonitor-backup-20260307-040506.json` → instante, quando o destino não dá a data. */
function stampOf(name: string): string | null {
  const match = /(\d{4})(\d{2})(\d{2})-(\d{2})(\d{2})(\d{2})/.exec(name)
  if (!match) return null
  const [, y, mo, d, h, mi, s] = match
  return `${y}-${mo}-${d}T${h}:${mi}:${s}Z`
}

function subtitleOf(copy: SystemBackupCopyResponse, index: number): string {
  const when = copy.lastModified ?? stampOf(copy.name)
  return [
    index === 0 ? 'mais recente' : null,
    formatRelativeTime(when),
    copy.size != null ? formatBytes(copy.size) : null,
  ]
    .filter(Boolean)
    .join(' · ')
}

async function download(copy: SystemBackupCopyResponse) {
  if (storageId.value == null) return
  downloading.value = copy.key
  try {
    await storagesStore.downloadObject(storageId.value, copy.key, copy.name)
  } catch (err) {
    fail(err, 'Erro ao baixar a cópia')
  } finally {
    downloading.value = null
  }
}

async function askCopy(copy: SystemBackupCopyResponse) {
  if (storageId.value == null) return
  previewing.value = copy.key
  error.value = null
  try {
    const counts = await backupStore.previewCopy(storageId.value, copy.key)
    pending.value = {
      name: formatDateTime(copy.lastModified ?? stampOf(copy.name)),
      key: copy.key,
      counts,
    }
  } catch (err) {
    fail(err, 'Erro ao ler a cópia')
  } finally {
    previewing.value = null
  }
}

async function onFile(value: File | File[] | null) {
  const chosen = Array.isArray(value) ? value[0] : value
  pending.value = null
  if (!chosen) {
    backupStore.clearFile()
    return
  }
  previewing.value = 'file'
  error.value = null
  const ok = await backupStore.loadFile(chosen)
  previewing.value = null
  if (ok && backupStore.pendingCounts) {
    pending.value = { name: chosen.name, key: null, counts: backupStore.pendingCounts }
  } else {
    error.value = backupStore.error
  }
}

async function restore() {
  if (!pending.value) return
  restoring.value = true
  error.value = null
  try {
    if (pending.value.key != null && storageId.value != null) {
      await backupStore.restoreCopy(storageId.value, pending.value.key)
    } else if (!(await backupStore.restoreConfig())) {
      throw new Error(backupStore.error ?? 'Erro ao restaurar o arquivo')
    }
    reloadAfterRestore()
  } catch (err) {
    fail(err, 'Erro ao restaurar')
    restoring.value = false
  }
}
</script>
