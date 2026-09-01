<template>
  <div>
    <PageHeader title="Imagens Docker" subtitle="Camadas, metadados e limpeza de imagens sem uso">
      <template #actions>
        <v-btn
          v-if="auth.isAdmin"
          color="warning"
          variant="tonal"
          prepend-icon="mdi-broom"
          :loading="docker.actionLoading"
          @click="pruneImages"
        >
          Limpar sem uso
        </v-btn>
        <v-btn
          variant="tonal"
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
          label="Buscar imagem ou tag"
          prepend-inner-icon="mdi-magnify"
          variant="outlined"
          density="compact"
          hide-details
          clearable
        ></v-text-field>
      </div>
      <ResponsiveDataTable
        :headers="headers"
        :items="filteredImages"
        :loading="docker.loading"
        no-data-text="Nenhuma imagem encontrada"
        clickable
        @click:row="onRowClick"
      >
        <template #item.name="{ item }">
          <div class="py-2">
            <div class="font-weight-bold">{{ imageName(item) }}</div>
            <div class="text-caption text-medium-emphasis font-mono">{{ shortId(item.id) }}</div>
          </div>
        </template>
        <template #item.created="{ item }">{{ formatEpoch(item.created) }}</template>
        <template #item.size="{ item }">{{ formatDecimalBytes(item.size) }}</template>
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
              icon="mdi-delete-outline"
              size="small"
              color="error"
              variant="text"
              title="Remover"
              :loading="docker.actionLoading"
              @click="removeImage(item)"
            ></v-btn>
          </div>
        </template>
        <template #mobile-item="{ item }">
          <div class="d-flex align-start justify-space-between ga-2">
            <div class="min-w-0">
              <div class="font-weight-bold text-truncate">{{ imageName(item) }}</div>
              <div class="text-caption text-medium-emphasis">
                {{ formatDecimalBytes(item.size) }}
              </div>
              <div class="text-caption">{{ formatEpoch(item.created) }}</div>
            </div>
            <v-icon color="primary">mdi-layers-outline</v-icon>
          </div>
        </template>
      </ResponsiveDataTable>
    </v-card>

    <v-dialog v-model="detailDialog" max-width="860" scrollable>
      <v-card rounded="xl">
        <v-card-title class="d-flex align-center ga-2">
          <v-icon color="primary">mdi-layers-outline</v-icon>
          <span class="text-truncate">{{
            detail ? detail.repoTags[0] || shortId(detail.id) : 'Imagem'
          }}</span>
          <v-spacer></v-spacer>
          <v-btn icon="mdi-close" variant="text" @click="detailDialog = false"></v-btn>
        </v-card-title>
        <v-divider></v-divider>
        <v-card-text>
          <v-skeleton-loader v-if="detailLoading" type="article"></v-skeleton-loader>
          <template v-else-if="detail">
            <v-list density="compact">
              <v-list-item title="ID" :subtitle="detail.id"></v-list-item>
              <v-list-item
                title="Criada em"
                :subtitle="formatDateTime(detail.created)"
              ></v-list-item>
              <v-list-item
                title="Tamanho do conteúdo"
                :subtitle="formatDecimalBytes(detail.size)"
              ></v-list-item>
              <v-list-item
                title="Usuário"
                :subtitle="detail.user || 'padrão da imagem'"
              ></v-list-item>
              <v-list-item
                title="Diretório de trabalho"
                :subtitle="detail.workingDir || '—'"
              ></v-list-item>
              <v-list-item title="Camadas" :subtitle="String(detail.layers.length)"></v-list-item>
            </v-list>
            <div class="text-subtitle-2 mt-4 mb-2">Comando</div>
            <pre class="docker-code">{{ detail.command.join(' ') || '—' }}</pre>
            <div class="text-subtitle-2 mt-4 mb-2">Ambiente (segredos ocultados pelo servidor)</div>
            <pre class="docker-code">{{ detail.environment.join('\n') || '—' }}</pre>
          </template>
        </v-card-text>
      </v-card>
    </v-dialog>

    <v-snackbar v-model="feedback.visible" :color="feedback.color" timeout="4500">
      {{ feedback.message }}
    </v-snackbar>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import PageHeader from '@/components/PageHeader.vue'
import ResponsiveDataTable from '@/components/ResponsiveDataTable.vue'
import { confirm } from '@/composables/useConfirm'
import { dockerService } from '@/services/dockerService'
import { useAuthStore } from '@/stores/auth'
import { useDockerStore } from '@/stores/docker'
import { formatDateTime, formatDecimalBytes } from '@/utils/formatters'
import type { DockerImageDetail } from '@/bindings/DockerImageDetail'
import type { DockerImageSummary } from '@/bindings/DockerImageSummary'

const docker = useDockerStore()
const auth = useAuthStore()
const search = ref('')
const detailDialog = ref(false)
const detailLoading = ref(false)
const detail = ref<DockerImageDetail | null>(null)
const feedback = ref({ visible: false, color: 'success', message: '' })

const headers = [
  { title: 'Imagem', key: 'name' },
  { title: 'Criada em', key: 'created', width: '190px' },
  { title: 'Uso em disco', key: 'size', width: '130px' },
  { title: 'Containers', key: 'containers', width: '110px' },
  { title: 'Ações', key: 'actions', width: '110px', sortable: false },
]

const filteredImages = computed(() => {
  const term = search.value.trim().toLocaleLowerCase()
  return term
    ? docker.images.filter((image) =>
        [...image.repoTags, ...image.repoDigests, image.id]
          .join(' ')
          .toLocaleLowerCase()
          .includes(term)
      )
    : docker.images
})

function imageName(image: DockerImageSummary): string {
  return image.repoTags[0] || '<sem tag>'
}

function shortId(id: string): string {
  return id.replace(/^sha256:/, '').slice(0, 12)
}

function formatEpoch(epoch: number): string {
  return epoch > 0 ? formatDateTime(new Date(epoch * 1000)) : '—'
}

function onRowClick(_event: MouseEvent, row: { item: DockerImageSummary }): void {
  void openDetail(row.item)
}

async function openDetail(image: DockerImageSummary): Promise<void> {
  detailDialog.value = true
  detailLoading.value = true
  try {
    detail.value = await dockerService.image(image.id)
  } catch (reason: unknown) {
    notify(reason instanceof Error ? reason.message : 'Erro ao inspecionar imagem', 'error')
  } finally {
    detailLoading.value = false
  }
}

async function removeImage(image: DockerImageSummary): Promise<void> {
  const accepted = await confirm({
    title: 'Remover imagem',
    message: `Remover a imagem "${imageName(image)}"? Containers que dependem dela podem impedir a operação.`,
    confirmText: 'Remover imagem',
    confirmColor: 'error',
    icon: 'mdi-layers-remove',
  })
  if (!accepted) return
  const success = await docker.runAction(() => dockerService.removeImage(image.id))
  notify(
    success ? 'Imagem removida.' : docker.error || 'Erro ao remover imagem',
    success ? 'success' : 'error'
  )
}

async function pruneImages(): Promise<void> {
  const accepted = await confirm({
    title: 'Limpar imagens sem uso',
    message: 'Remover todas as imagens pendentes (dangling) que não são usadas por containers?',
    confirmText: 'Executar limpeza',
    confirmColor: 'warning',
    icon: 'mdi-broom',
  })
  if (!accepted) return
  let removed = 0
  let reclaimed = 0
  const success = await docker.runAction(async () => {
    const result = await dockerService.pruneImages()
    removed = result.imagesDeleted
    reclaimed = result.spaceReclaimed
  })
  notify(
    success
      ? `${removed} registro(s) removido(s); ${formatDecimalBytes(reclaimed)} recuperados.`
      : docker.error || 'Erro ao limpar imagens',
    success ? 'success' : 'error'
  )
}

function notify(message: string, color: string): void {
  feedback.value = { visible: true, color, message }
}
</script>

<style scoped>
.docker-code {
  background: rgb(var(--v-theme-surface-variant));
  border-radius: 8px;
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  font-size: 0.78rem;
  overflow: auto;
  padding: 12px;
  white-space: pre-wrap;
  word-break: break-all;
}
</style>
