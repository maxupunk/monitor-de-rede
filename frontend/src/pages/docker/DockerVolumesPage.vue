<template>
  <div>
    <PageHeader title="Volumes" subtitle="Persistência, inspeção, exportação e remoção segura">
      <template #actions>
        <v-btn
          color="primary"
          prepend-icon="mdi-refresh"
          :loading="docker.loading"
          @click="docker.refreshAll()"
        >
          Atualizar
        </v-btn>
      </template>
    </PageHeader>

    <v-alert v-if="docker.error" type="error" variant="tonal" closable class="mb-4">
      {{ docker.error }}
    </v-alert>
    <v-card rounded="xl" variant="outlined">
      <div class="pa-4">
        <v-text-field
          v-model="search"
          label="Buscar volume"
          prepend-inner-icon="mdi-magnify"
          variant="outlined"
          density="compact"
          hide-details
          clearable
        ></v-text-field>
      </div>
      <ResponsiveDataTable
        :headers="headers"
        :items="filteredVolumes"
        :loading="docker.loading"
        no-data-text="Nenhum volume encontrado"
        clickable
        @click:row="onRowClick"
      >
        <template #item.name="{ item }">
          <div class="py-2">
            <div class="font-weight-bold">{{ item.name }}</div>
            <div class="text-caption text-medium-emphasis">{{ projectName(item.labels) }}</div>
          </div>
        </template>
        <template #item.createdAt="{ item }">
          {{ formatDateTime(item.createdAt) }}
        </template>
        <template #item.actions="{ item }">
          <div class="d-flex justify-end ga-1" @click.stop>
            <v-btn
              icon="mdi-eye-outline"
              size="small"
              variant="text"
              title="Inspecionar"
              @click="openDetail(item)"
            ></v-btn>
            <v-btn
              v-if="auth.isAdmin"
              icon="mdi-download-outline"
              size="small"
              color="primary"
              variant="text"
              title="Exportar"
              :loading="exportingName === item.name"
              @click="exportVolume(item.name)"
            ></v-btn>
            <v-btn
              v-if="auth.isAdmin"
              icon="mdi-delete-outline"
              size="small"
              color="error"
              variant="text"
              title="Remover"
              :loading="docker.actionLoading"
              @click="removeVolume(item.name)"
            ></v-btn>
          </div>
        </template>
        <template #mobile-item="{ item }">
          <div class="d-flex align-start justify-space-between ga-2">
            <div class="min-w-0">
              <div class="font-weight-bold text-truncate">{{ item.name }}</div>
              <div class="text-caption text-medium-emphasis">
                {{ item.driver }} · {{ item.scope }}
              </div>
            </div>
            <v-icon color="primary">mdi-database-outline</v-icon>
          </div>
        </template>
      </ResponsiveDataTable>
    </v-card>

    <v-dialog v-model="detailDialog" max-width="760">
      <v-card rounded="xl">
        <v-card-title class="d-flex align-center ga-2">
          <v-icon color="primary">mdi-database-outline</v-icon>
          {{ detail?.name || 'Volume' }}
          <v-spacer></v-spacer>
          <v-btn icon="mdi-close" variant="text" @click="detailDialog = false"></v-btn>
        </v-card-title>
        <v-divider></v-divider>
        <v-card-text>
          <v-skeleton-loader v-if="detailLoading" type="article"></v-skeleton-loader>
          <v-list v-else-if="detail" density="compact">
            <v-list-item title="Driver" :subtitle="detail.driver"></v-list-item>
            <v-list-item title="Escopo" :subtitle="detail.scope"></v-list-item>
            <v-list-item
              title="Criado em"
              :subtitle="formatDateTime(detail.createdAt)"
            ></v-list-item>
            <v-list-item title="Ponto de montagem" :subtitle="detail.mountpoint"></v-list-item>
            <v-list-item title="Projeto" :subtitle="projectName(detail.labels)"></v-list-item>
            <v-list-item
              title="Opções"
              :subtitle="
                Object.entries(detail.options)
                  .map(([key, value]) => `${key}=${value}`)
                  .join(', ') || '—'
              "
            ></v-list-item>
          </v-list>
        </v-card-text>
        <v-card-actions v-if="auth.isAdmin" class="justify-end pa-4">
          <v-btn
            prepend-icon="mdi-download-outline"
            color="primary"
            variant="tonal"
            @click="detail && exportVolume(detail.name)"
          >
            Exportar .tar.gz
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <v-snackbar v-model="feedback.visible" :color="feedback.color" timeout="4500">
      {{ feedback.message }}
    </v-snackbar>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import PageHeader from '@/components/PageHeader.vue'
import ResponsiveDataTable from '@/components/ResponsiveDataTable.vue'
import { confirm } from '@/composables/useConfirm'
import { dockerService } from '@/services/dockerService'
import { useAuthStore } from '@/stores/auth'
import { useDockerStore } from '@/stores/docker'
import { formatDateTime } from '@/utils/formatters'
import type { DockerVolumeDetail } from '@/bindings/DockerVolumeDetail'
import type { DockerVolumeSummary } from '@/bindings/DockerVolumeSummary'

const docker = useDockerStore()
const auth = useAuthStore()
const search = ref('')
const detailDialog = ref(false)
const detailLoading = ref(false)
const detail = ref<DockerVolumeDetail | null>(null)
const exportingName = ref<string | null>(null)
const feedback = ref({ visible: false, color: 'success', message: '' })

const headers = [
  { title: 'Volume', key: 'name' },
  { title: 'Driver', key: 'driver', width: '130px' },
  { title: 'Escopo', key: 'scope', width: '120px' },
  { title: 'Criado em', key: 'createdAt', width: '190px' },
  { title: 'Ações', key: 'actions', width: '160px', sortable: false },
]

const filteredVolumes = computed(() => {
  const term = search.value.trim().toLocaleLowerCase()
  return term
    ? docker.volumes.filter((volume) =>
        [volume.name, volume.driver, projectName(volume.labels)]
          .join(' ')
          .toLocaleLowerCase()
          .includes(term)
      )
    : docker.volumes
})

onMounted(() => void docker.refreshAll())

function projectName(labels: Record<string, string>): string {
  return labels['com.docker.compose.project'] || 'Sem projeto Compose'
}

function onRowClick(_event: MouseEvent, row: { item: DockerVolumeSummary }): void {
  void openDetail(row.item)
}

async function openDetail(volume: DockerVolumeSummary): Promise<void> {
  detailDialog.value = true
  detailLoading.value = true
  try {
    detail.value = await dockerService.volume(volume.name)
  } catch (reason: unknown) {
    notify(reason instanceof Error ? reason.message : 'Erro ao inspecionar volume', 'error')
  } finally {
    detailLoading.value = false
  }
}

async function exportVolume(name: string): Promise<void> {
  exportingName.value = name
  try {
    await dockerService.exportVolume(name)
    notify('Exportação do volume concluída.', 'success')
  } catch (reason: unknown) {
    notify(reason instanceof Error ? reason.message : 'Erro ao exportar volume', 'error')
  } finally {
    exportingName.value = null
  }
}

async function removeVolume(name: string): Promise<void> {
  const accepted = await confirm({
    title: 'Remover volume',
    message: `Remover permanentemente o volume "${name}"? Os dados não poderão ser recuperados.`,
    confirmText: 'Remover volume',
    confirmColor: 'error',
    icon: 'mdi-database-remove-outline',
  })
  if (!accepted) return
  const success = await docker.runAction(() => dockerService.removeVolume(name))
  notify(
    success ? 'Volume removido.' : docker.error || 'Erro ao remover volume',
    success ? 'success' : 'error'
  )
}

function notify(message: string, color: string): void {
  feedback.value = { visible: true, color, message }
}
</script>
