<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 820"
    :fullscreen="$vuetify.display.xs"
    scrollable
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card v-if="storage" class="rounded-lg">
      <v-card-title class="font-weight-bold d-flex align-center pt-4 px-6">
        <v-icon start :color="info.color">{{ info.icon }}</v-icon>
        Arquivos em {{ storage.name }}
      </v-card-title>

      <v-card-text class="px-6">
        <div class="d-flex align-center flex-wrap ga-1 mb-3">
          <v-btn
            size="small"
            variant="tonal"
            color="primary"
            prepend-icon="mdi-home-outline"
            @click="open('')"
          >
            Raiz
          </v-btn>
          <template v-for="crumb in crumbs" :key="crumb.path">
            <v-icon size="16" color="primary">mdi-chevron-right</v-icon>
            <v-btn size="small" variant="text" color="primary" @click="open(crumb.path)">
              {{ crumb.name }}
            </v-btn>
          </template>
          <v-spacer></v-spacer>
          <v-btn
            icon
            size="small"
            variant="text"
            color="primary"
            :loading="loading"
            @click="open(path)"
          >
            <v-icon>mdi-refresh</v-icon>
            <v-tooltip activator="parent" location="top">Atualizar</v-tooltip>
          </v-btn>
        </div>

        <v-alert
          v-if="error"
          type="error"
          variant="tonal"
          density="compact"
          class="mb-3"
          closable
          @click:close="error = null"
        >
          {{ error }}
        </v-alert>

        <v-progress-linear v-if="loading" indeterminate color="primary" class="mb-2" />

        <div v-if="!loading && objects.length === 0" class="text-center py-8 border rounded-lg">
          <v-icon size="40" color="info">mdi-folder-open-outline</v-icon>
          <div class="font-weight-bold mt-2">Pasta vazia</div>
        </div>

        <v-list v-else density="comfortable" class="py-0 border rounded-lg">
          <v-list-item
            v-for="(item, index) in objects"
            :key="item.key"
            :title="item.name"
            :subtitle="item.isDirectory ? 'Pasta' : describe(item)"
            :class="{ 'border-t': index > 0 }"
            @click="item.isDirectory ? open(item.key) : undefined"
          >
            <template #prepend>
              <v-icon :color="item.isDirectory ? 'warning' : 'info'">
                {{ item.isDirectory ? 'mdi-folder' : 'mdi-file-outline' }}
              </v-icon>
            </template>
            <template #append>
              <div class="d-flex ga-1">
                <v-btn
                  v-if="!item.isDirectory"
                  icon
                  size="small"
                  variant="text"
                  color="primary"
                  :loading="busy === item.key"
                  @click.stop="download(item)"
                >
                  <v-icon>mdi-download</v-icon>
                  <v-tooltip activator="parent" location="top">Baixar</v-tooltip>
                </v-btn>
                <v-btn icon size="small" variant="text" color="error" @click.stop="remove(item)">
                  <v-icon>mdi-delete-outline</v-icon>
                  <v-tooltip activator="parent" location="top">Excluir</v-tooltip>
                </v-btn>
              </div>
            </template>
          </v-list-item>
        </v-list>

        <div v-if="nextCursor" class="text-center mt-3">
          <v-btn variant="tonal" color="primary" :loading="loading" @click="loadMore">
            Carregar mais
          </v-btn>
        </div>
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
import type { StorageDestinationResponse } from '@/bindings/StorageDestinationResponse'
import type { StorageObjectResponse } from '@/bindings/StorageObjectResponse'
import { confirm } from '@/composables/useConfirm'
import { useStoragesStore } from '@/stores/storages'
import { formatBytes, formatDateTime } from '@/utils/formatters'
import { providerInfo } from '@/utils/storagePresentation'

const props = defineProps<{
  modelValue: boolean
  storage: StorageDestinationResponse | null
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
}>()

const storagesStore = useStoragesStore()

const path = ref('')
const objects = ref<StorageObjectResponse[]>([])
const nextCursor = ref<string | null>(null)
const loading = ref(false)
const busy = ref<string | null>(null)
const error = ref<string | null>(null)

const info = computed(() => providerInfo(props.storage?.provider ?? 'local'))
const crumbs = computed(() => {
  const parts = path.value.split('/').filter(Boolean)
  return parts.map((name, index) => ({ name, path: parts.slice(0, index + 1).join('/') }))
})

function fail(err: unknown, fallback: string) {
  error.value = err instanceof Error ? err.message : fallback
}

function describe(item: StorageObjectResponse): string {
  return [item.size != null ? formatBytes(item.size) : null, formatDateTime(item.lastModified, '')]
    .filter(Boolean)
    .join(' · ')
}

async function fetchPage(target: string, cursor: string | null) {
  if (!props.storage) return
  loading.value = true
  error.value = null
  try {
    const page = await storagesStore.browse(props.storage.id, target, cursor)
    path.value = page.path
    objects.value = cursor ? [...objects.value, ...page.objects] : page.objects
    nextCursor.value = page.nextCursor
  } catch (err) {
    fail(err, 'Erro ao listar a pasta')
  } finally {
    loading.value = false
  }
}

function open(target: string) {
  void fetchPage(target, null)
}

function loadMore() {
  void fetchPage(path.value, nextCursor.value)
}

watch(
  () => props.modelValue,
  (opened) => {
    if (!opened) return
    objects.value = []
    nextCursor.value = null
    open('')
  }
)

async function download(item: StorageObjectResponse) {
  if (!props.storage) return
  busy.value = item.key
  try {
    await storagesStore.downloadObject(props.storage.id, item.key, item.name)
  } catch (err) {
    fail(err, 'Erro ao baixar o arquivo')
  } finally {
    busy.value = null
  }
}

async function remove(item: StorageObjectResponse) {
  if (!props.storage) return
  const ok = await confirm({
    title: item.isDirectory ? 'Excluir pasta' : 'Excluir arquivo',
    message: item.isDirectory
      ? `A pasta "${item.name}" e tudo o que está dentro dela serão apagados do destino. Não há como desfazer.`
      : `"${item.name}" será apagado do destino. Não há como desfazer.`,
    confirmText: 'Excluir',
    confirmColor: 'error',
    icon: 'mdi-delete-alert-outline',
  })
  if (!ok) return
  try {
    await storagesStore.deleteObject(props.storage.id, item.key, item.isDirectory)
    objects.value = objects.value.filter((object) => object.key !== item.key)
  } catch (err) {
    fail(err, 'Erro ao excluir')
  }
}
</script>
