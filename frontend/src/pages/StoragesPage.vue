<template>
  <div>
    <PageHeader
      title="Armazenamentos"
      subtitle="Para onde vão as cópias de segurança das configurações: uma pasta no servidor, um NAS por SFTP ou um bucket na nuvem"
    >
      <template #actions>
        <v-btn color="primary" prepend-icon="mdi-plus" @click="openForm(null)">
          <span class="hidden-sm-and-down">Novo armazenamento</span>
          <span class="hidden-md-and-up">Novo</span>
        </v-btn>
      </template>
    </PageHeader>

    <v-alert
      v-if="storagesStore.error"
      type="error"
      variant="tonal"
      density="compact"
      class="mb-4"
      closable
      @click:close="storagesStore.error = null"
    >
      {{ storagesStore.error }}
    </v-alert>

    <v-progress-linear
      v-if="storagesStore.loading && !storagesStore.loaded"
      indeterminate
      color="primary"
      class="mb-4"
    />

    <!-- Estado vazio: explica por que isso existe antes de pedir um cadastro. -->
    <v-card
      v-if="storagesStore.loaded && storagesStore.storages.length === 0"
      elevation="2"
      rounded="lg"
      class="pa-6 pa-sm-10 text-center"
    >
      <v-avatar color="primary" variant="tonal" size="72" class="mb-4">
        <v-icon size="40">mdi-database-lock-outline</v-icon>
      </v-avatar>
      <div class="text-h6 font-weight-bold mb-2">Guarde a configuração fora deste servidor</div>
      <p class="text-body-2 text-high-emphasis mx-auto mb-6" style="max-width: 560px">
        Cadastre um armazenamento e o sistema envia sozinho, no horário que você escolher, uma cópia
        de dispositivos, monitores, regras de alerta e VPN. Se o disco falhar ou uma mudança der
        errado, você restaura em um clique.
      </p>
      <v-btn color="primary" size="large" prepend-icon="mdi-plus" @click="openForm(null)">
        Cadastrar o primeiro armazenamento
      </v-btn>
    </v-card>

    <v-row v-else>
      <v-col v-for="storage in storagesStore.storages" :key="storage.id" cols="12" md="6" xl="4">
        <v-card elevation="2" rounded="lg" class="h-100 d-flex flex-column">
          <div class="d-flex align-start ga-3 pa-4 pb-2">
            <v-avatar
              :color="providerInfo(storage.provider).color"
              variant="tonal"
              rounded="lg"
              size="48"
            >
              <v-icon size="28">{{ providerInfo(storage.provider).icon }}</v-icon>
            </v-avatar>
            <div class="flex-grow-1 min-w-0">
              <div class="text-subtitle-1 font-weight-bold text-truncate">{{ storage.name }}</div>
              <div
                v-if="storage.name !== providerInfo(storage.provider).label"
                class="text-caption text-high-emphasis"
              >
                {{ providerInfo(storage.provider).label }}
              </div>
              <code v-if="storage.target" class="target d-block text-truncate mt-1">
                {{ storage.target }}
              </code>
              <div v-else class="text-caption text-error mt-1">
                Credencial ilegível — edite e informe de novo.
              </div>
            </div>
            <v-menu location="bottom end">
              <template #activator="{ props: menuProps }">
                <v-btn icon size="small" variant="text" color="primary" v-bind="menuProps">
                  <v-icon>mdi-dots-vertical</v-icon>
                </v-btn>
              </template>
              <v-list density="compact">
                <v-list-item
                  prepend-icon="mdi-folder-search-outline"
                  title="Explorar arquivos"
                  @click="openExplorer(storage)"
                />
                <v-list-item
                  prepend-icon="mdi-connection"
                  title="Testar conexão"
                  @click="testConnection(storage)"
                />
                <v-list-item
                  prepend-icon="mdi-pencil"
                  title="Editar"
                  @click="openForm(storage.id)"
                />
                <v-list-item
                  prepend-icon="mdi-delete-outline"
                  title="Excluir"
                  base-color="error"
                  @click="removeStorage(storage)"
                />
              </v-list>
            </v-menu>
          </div>

          <div class="px-4 pb-3 flex-grow-1">
            <div class="d-flex flex-wrap ga-2 mb-3">
              <v-chip
                v-if="storage.backupEnabled"
                color="success"
                variant="tonal"
                size="small"
                prepend-icon="mdi-calendar-sync"
              >
                {{ intervalLabel(storage.backupIntervalHours) }} · mantém
                {{ storage.backupRetention }}
              </v-chip>
              <v-chip
                v-else
                color="info"
                variant="tonal"
                size="small"
                prepend-icon="mdi-hand-back-right-outline"
              >
                Só backup manual
              </v-chip>
            </div>

            <div class="d-flex align-center ga-2 text-body-2">
              <v-icon size="18" :color="lastBackupColor(storage)">
                {{ lastBackupIcon(storage) }}
              </v-icon>
              <span>{{ lastBackupText(storage) }}</span>
            </div>
            <v-alert
              v-if="storage.lastBackupStatus === 'failed' && storage.lastBackupError"
              type="error"
              variant="tonal"
              density="compact"
              class="mt-2 text-body-2"
            >
              <div class="error-text" :title="storage.lastBackupError">
                {{ storage.lastBackupError }}
              </div>
            </v-alert>
            <div v-if="storage.nextBackupAt" class="d-flex align-center ga-2 text-body-2 mt-1">
              <v-icon size="18" color="primary">mdi-clock-outline</v-icon>
              <span>Próximo automático: {{ formatTimeUntil(storage.nextBackupAt) }}</span>
            </div>
          </div>

          <v-divider></v-divider>
          <div class="d-flex flex-wrap ga-2 pa-3">
            <v-btn
              color="success"
              variant="flat"
              prepend-icon="mdi-cloud-upload-outline"
              :loading="storagesStore.running.includes(storage.id)"
              @click="backupNow(storage)"
            >
              Fazer backup agora
            </v-btn>
            <v-btn
              color="primary"
              variant="tonal"
              prepend-icon="mdi-history"
              @click="openBackups(storage)"
            >
              Cópias e restauração
            </v-btn>
          </div>
        </v-card>
      </v-col>
    </v-row>

    <StorageFormDialog v-model="formDialog" :storage-id="editingId" @saved="onSaved" />
    <StorageBackupsDialog
      v-model="backupsDialog"
      :storage="selected"
      @restored="notify('Configuração restaurada a partir da cópia.')"
    />
    <StorageExplorerDialog v-model="explorerDialog" :storage="selected" />

    <v-snackbar v-model="feedback.visible" :color="feedback.color" timeout="5000">
      {{ feedback.message }}
    </v-snackbar>
  </div>
</template>

<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue'
import type { StorageDestinationDetail } from '@/bindings/StorageDestinationDetail'
import type { StorageDestinationResponse } from '@/bindings/StorageDestinationResponse'
import PageHeader from '@/components/PageHeader.vue'
import StorageBackupsDialog from '@/components/storages/StorageBackupsDialog.vue'
import StorageExplorerDialog from '@/components/storages/StorageExplorerDialog.vue'
import StorageFormDialog from '@/components/storages/StorageFormDialog.vue'
import { confirm } from '@/composables/useConfirm'
import { useStoragesStore } from '@/stores/storages'
import { formatBytes, formatRelativeTime, formatTimeUntil } from '@/utils/formatters'
import { intervalLabel, providerInfo } from '@/utils/storagePresentation'

const storagesStore = useStoragesStore()

const formDialog = ref(false)
const editingId = ref<number | null>(null)
const backupsDialog = ref(false)
const explorerDialog = ref(false)
const selected = ref<StorageDestinationResponse | null>(null)
const feedback = reactive({ visible: false, message: '', color: 'success' })

onMounted(() => {
  void storagesStore.fetchStorages()
})

function notify(message: string, color = 'success') {
  feedback.message = message
  feedback.color = color
  feedback.visible = true
}

function openForm(id: number | null) {
  editingId.value = id
  formDialog.value = true
}

function openBackups(storage: StorageDestinationResponse) {
  selected.value = storage
  backupsDialog.value = true
}

function openExplorer(storage: StorageDestinationResponse) {
  selected.value = storage
  explorerDialog.value = true
}

function onSaved(saved: StorageDestinationDetail, created: boolean) {
  if (!created) {
    notify('Armazenamento atualizado.')
  } else if (saved.backupEnabled) {
    notify('Armazenamento cadastrado. O primeiro backup automático sai nos próximos minutos.')
  } else {
    notify('Armazenamento cadastrado.')
  }
}

function lastBackupIcon(storage: StorageDestinationResponse): string {
  if (storage.lastBackupStatus === 'success') return 'mdi-check-circle'
  if (storage.lastBackupStatus === 'failed') return 'mdi-alert-circle'
  return 'mdi-circle-outline'
}

function lastBackupColor(storage: StorageDestinationResponse): string {
  if (storage.lastBackupStatus === 'success') return 'success'
  if (storage.lastBackupStatus === 'failed') return 'error'
  return 'info'
}

function lastBackupText(storage: StorageDestinationResponse): string {
  if (!storage.lastBackupAt) return 'Ainda não houve backup aqui'
  const when = formatRelativeTime(storage.lastBackupAt)
  return storage.lastBackupStatus === 'failed'
    ? `Último backup falhou ${when}`
    : `Último backup ${when}`
}

async function backupNow(storage: StorageDestinationResponse) {
  try {
    const result = await storagesStore.runBackup(storage.id)
    const size = result.backup.size != null ? ` (${formatBytes(result.backup.size)})` : ''
    const pruned =
      result.pruned > 0
        ? ` ${result.pruned} ${result.pruned === 1 ? 'cópia antiga removida' : 'cópias antigas removidas'}.`
        : ''
    notify(`Backup enviado para ${storage.name}${size}.${pruned}`)
  } catch (err) {
    notify(err instanceof Error ? err.message : 'Erro ao fazer o backup', 'error')
  }
}

async function testConnection(storage: StorageDestinationResponse) {
  try {
    const result = await storagesStore.testSaved(storage.id)
    notify(`${storage.name}: ${result.message}`, result.ok ? 'success' : 'error')
  } catch (err) {
    notify(err instanceof Error ? err.message : 'Erro ao testar a conexão', 'error')
  }
}

async function removeStorage(storage: StorageDestinationResponse) {
  const ok = await confirm({
    title: 'Excluir armazenamento',
    message: `"${storage.name}" deixa de receber backups. As cópias que já estão lá continuam onde estão — apague-as pelo explorador, se quiser.`,
    confirmText: 'Excluir',
    confirmColor: 'error',
    icon: 'mdi-delete-alert-outline',
  })
  if (!ok) return
  if (await storagesStore.remove(storage.id)) notify('Armazenamento excluído.')
}
</script>

<style scoped>
/* Erro longo não pode empurrar os botões para fora do cartão; o texto
   inteiro fica no `title`. */
.error-text {
  display: -webkit-box;
  -webkit-line-clamp: 3;
  line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
  word-break: break-word;
}
.target {
  font-size: 0.78rem;
}
.min-w-0 {
  min-width: 0;
}
</style>
