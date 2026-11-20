<template>
  <div>
    <SystemRestoreConfirm
      v-if="backupStore.pendingCounts && backupStore.pendingName"
      :name="backupStore.pendingName"
      :counts="backupStore.pendingCounts"
      :loading="backupStore.restoring"
      class="mb-4"
      @confirm="restore"
      @cancel="cancel"
    />

    <div v-if="progress" class="mb-4" data-upload-progress>
      <div class="d-flex justify-space-between text-body-2 mb-1">
        <span class="font-weight-bold">{{ progress.title }}</span>
        <span>{{ progress.detail }}</span>
      </div>
      <v-progress-linear
        :model-value="progress.percent ?? undefined"
        :indeterminate="progress.percent == null"
        color="primary"
        height="8"
        rounded
      ></v-progress-linear>
    </div>

    <v-alert
      v-if="backupStore.error"
      type="error"
      variant="tonal"
      density="compact"
      class="mb-4"
      closable
      @click:close="backupStore.error = null"
    >
      {{ backupStore.error }}
    </v-alert>

    <v-file-input
      v-if="!backupStore.pendingCounts"
      v-model="file"
      label="Arquivo de backup (.json ou .json.gz)"
      accept=".json,.gz,application/json,application/gzip"
      variant="outlined"
      prepend-icon=""
      prepend-inner-icon="mdi-file-upload-outline"
      :disabled="busy"
      hide-details
      @update:model-value="onFile"
    ></v-file-input>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import type { BackupRestoreResponse } from '@/bindings/BackupRestoreResponse'
import SystemRestoreConfirm from './SystemRestoreConfirm.vue'
import { useBackupStore } from '@/stores/backup'
import { formatBinaryBytes } from '@/utils/formatters'

/**
 * Restaurar a partir de um arquivo do computador: escolher, conferir o que há
 * nele (a prévia) e aplicar — com o andamento do envio nas duas idas. Serve à
 * tela de Backup e ao assistente do primeiro acesso; o que vem depois da
 * restauração é de quem usa (`restored`).
 */
const emit = defineEmits<{ restored: [result: BackupRestoreResponse] }>()

const backupStore = useBackupStore()
const file = ref<File | File[] | null>(null)

const busy = computed(() => backupStore.upload != null || backupStore.restoring)

const progress = computed(() => {
  const upload = backupStore.upload
  if (!upload) return null
  const sent = Math.min(upload.sentBytes, upload.totalBytes)
  const done = upload.totalBytes > 0 && sent >= upload.totalBytes
  if (upload.phase === 'preview') {
    return done
      ? {
          title: 'Conferindo o arquivo…',
          detail: formatBinaryBytes(upload.totalBytes),
          percent: null,
        }
      : {
          title: 'Enviando para conferir',
          detail: `${formatBinaryBytes(sent)} de ${formatBinaryBytes(upload.totalBytes)}`,
          percent: (sent / upload.totalBytes) * 100,
        }
  }
  return done
    ? {
        title: 'Gravando no banco — um backup grande leva alguns minutos',
        detail: formatBinaryBytes(upload.totalBytes),
        percent: null,
      }
    : {
        title: 'Restaurando: enviando o arquivo',
        detail: `${formatBinaryBytes(sent)} de ${formatBinaryBytes(upload.totalBytes)}`,
        percent: (sent / upload.totalBytes) * 100,
      }
})

async function onFile(value: File | File[] | null) {
  const chosen = Array.isArray(value) ? value[0] : value
  if (!chosen) {
    backupStore.clearFile()
    return
  }
  await backupStore.loadFile(chosen)
  file.value = null
}

function cancel() {
  backupStore.clearFile()
}

async function restore() {
  const result = await backupStore.restoreFile()
  if (result) emit('restored', result)
}

// Uma escolha deixada por outra tela não vale aqui.
onMounted(() => {
  backupStore.clearFile()
  backupStore.error = null
})
</script>
