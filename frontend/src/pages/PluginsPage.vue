<template>
  <div>
    <PageHeader
      title="Plugins de dispositivo"
      subtitle="Drivers que agem nos equipamentos por SSH, HTTP ou Telnet. Instale-os na aba Plugins de cada equipamento."
    >
      <template #actions>
        <v-btn color="primary" variant="flat" prepend-icon="mdi-plus" @click="openEditor(null)">
          Novo plugin
        </v-btn>
        <v-btn
          color="secondary"
          variant="tonal"
          prepend-icon="mdi-file-import-outline"
          @click="fileInput?.click()"
        >
          Importar
        </v-btn>
        <input
          ref="fileInput"
          type="file"
          accept=".json,application/json"
          class="d-none"
          @change="importFile"
        />
      </template>
    </PageHeader>

    <v-alert v-if="store.error" type="error" variant="tonal" class="mb-4">{{
      store.error
    }}</v-alert>

    <v-alert
      v-if="attention.length > 0"
      type="warning"
      variant="tonal"
      class="mb-4"
      density="comfortable"
      icon="mdi-bell-alert-outline"
    >
      {{ attentionText }}
      <template #append>
        <v-btn color="warning" variant="flat" size="small" @click="filter = 'pending'"> Ver </v-btn>
      </template>
    </v-alert>

    <div class="d-flex flex-wrap align-center ga-3 mb-4">
      <v-chip-group v-model="filter" mandatory column class="flex-grow-1">
        <v-chip
          v-for="option in LIBRARY_FILTERS"
          :key="option.value"
          :value="option.value"
          :color="option.color"
          :base-color="option.color"
          :prepend-icon="option.icon"
          :variant="filter === option.value ? 'flat' : 'tonal'"
          :disabled="option.value !== 'all' && counts[option.value] === 0"
        >
          {{ option.label }} · {{ counts[option.value] }}
        </v-chip>
      </v-chip-group>
      <v-text-field
        v-model="search"
        prepend-inner-icon="mdi-magnify"
        placeholder="Buscar plugin"
        aria-label="Buscar plugin por nome, slug ou descrição"
        variant="outlined"
        density="compact"
        hide-details
        clearable
        class="search"
      ></v-text-field>
    </div>

    <v-row v-if="store.libraryLoading && store.library.length === 0" dense>
      <v-col v-for="n in 6" :key="n" cols="12" md="6" xl="4">
        <v-skeleton-loader type="list-item-avatar-two-line, paragraph, actions" class="rounded-lg">
        </v-skeleton-loader>
      </v-col>
    </v-row>

    <v-card v-else-if="store.library.length === 0" border flat class="rounded-lg text-center pa-8">
      <v-icon size="48" color="primary">mdi-puzzle-plus-outline</v-icon>
      <div class="text-h6 mt-2">Nenhum plugin ainda</div>
      <p class="text-body-2 mb-4">Crie um no editor, peça à IA ou importe um pacote.</p>
      <v-btn color="primary" variant="flat" prepend-icon="mdi-plus" @click="openEditor(null)">
        Novo plugin
      </v-btn>
    </v-card>

    <div v-else-if="visible.length === 0" class="text-center pa-6">
      <div class="text-body-1 font-weight-medium mb-2">Nenhum plugin com esses filtros.</div>
      <v-btn color="primary" variant="tonal" prepend-icon="mdi-filter-off" @click="resetFilters">
        Limpar filtros
      </v-btn>
    </div>

    <v-row v-else dense>
      <v-col v-for="item in visible" :key="item.id" cols="12" md="6" xl="4">
        <PluginLibraryCard
          :plugin="item"
          @edit="openEditor($event.id)"
          @review="openReview($event.id)"
          @export="store.exportPlugin($event)"
          @remove="remove"
          @lifecycle="step"
        />
      </v-col>
    </v-row>

    <PluginEditorDialog v-model="editor.open" :plugin-id="editor.pluginId" />
    <PluginReviewDialog
      v-model="review.open"
      :detail="review.detail"
      :busy="review.busy"
      @review="reviewAgain"
      @accept="acceptReview"
    />
    <v-snackbar v-model="feedback.show" :color="feedback.color" timeout="6000" location="bottom">
      {{ feedback.text }}
    </v-snackbar>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import type { PluginDetail } from '@/bindings/PluginDetail'
import type { PluginSummary } from '@/bindings/PluginSummary'
import PageHeader from '@/components/PageHeader.vue'
import PluginEditorDialog from '@/components/plugins/PluginEditorDialog.vue'
import PluginLibraryCard from '@/components/plugins/PluginLibraryCard.vue'
import PluginReviewDialog from '@/components/plugins/PluginReviewDialog.vue'
import { confirm } from '@/composables/useConfirm'
import { usePluginsStore } from '@/stores/plugins'
import { usePluginAppsStore } from '@/stores/pluginApps'
import {
  filterLibrary,
  LIBRARY_FILTERS,
  libraryCounts,
  readPackageFile,
  type LibraryFilter,
} from '@/utils/pluginPresentation'

const store = usePluginsStore()
const appsStore = usePluginAppsStore()
const fileInput = ref<HTMLInputElement | null>(null)
const search = ref<string | null>('')
const filter = ref<LibraryFilter>('all')
const feedback = reactive({ show: false, text: '', color: 'success' })
const editor = reactive({ open: false, pluginId: null as number | null })
const review = reactive({
  open: false,
  detail: null as PluginDetail | null,
  busy: null as 'review' | 'accept' | null,
})

const counts = computed(() => libraryCounts(store.library))
const visible = computed(() => filterLibrary(store.library, filter.value, search.value ?? ''))

/** O que espera o operador: revisão de importados e ativação de testados. */
const attention = computed(() =>
  store.library.filter((item) => item.status === 'quarantine' || item.status === 'tested')
)
const attentionText = computed(() => {
  const toReview = attention.value.filter((item) => item.status === 'quarantine').length
  const toActivate = attention.value.length - toReview
  const parts = []
  if (toReview) parts.push(`${toReview} aguardando revisão de segurança`)
  if (toActivate) parts.push(`${toActivate} pronto(s) para ativar`)
  return `Plugins esperando você: ${parts.join(' e ')}.`
})

function resetFilters() {
  filter.value = 'all'
  search.value = ''
}

onMounted(() => {
  void store.fetchLibrary()
})

function notify(text: string, color = 'success') {
  feedback.text = text
  feedback.color = color
  feedback.show = true
}

function describe(err: unknown, fallback: string): string {
  return err instanceof Error ? err.message : fallback
}

function openEditor(pluginId: number | null) {
  editor.pluginId = pluginId
  editor.open = true
}

async function step(item: PluginSummary, action: 'promote' | 'enable' | 'disable') {
  try {
    await store.lifecycle(item.id, action)
    await appsStore.fetchApps()
  } catch (err: unknown) {
    notify(describe(err, 'Falha na operação'), 'error')
  }
}

async function remove(item: PluginSummary) {
  const ok = await confirm({
    title: 'Excluir plugin',
    message: `Excluir “${item.name}” v${item.version}? O histórico de execuções continua na auditoria.`,
    confirmText: 'Excluir',
    confirmColor: 'error',
  })
  if (!ok) return
  try {
    await store.deletePlugin(item.id)
    await appsStore.fetchApps()
  } catch (err: unknown) {
    notify(describe(err, 'Falha ao excluir'), 'error')
  }
}

async function openReview(pluginId: number) {
  try {
    review.detail = await store.fetchDetail(pluginId)
    review.open = true
  } catch (err: unknown) {
    notify(describe(err, 'Falha ao carregar a revisão'), 'error')
  }
}

async function reviewAgain() {
  if (!review.detail) return
  review.busy = 'review'
  try {
    review.detail = await store.lifecycle(review.detail.summary.id, 'review')
  } catch (err: unknown) {
    notify(describe(err, 'Falha ao revisar'), 'error')
  } finally {
    review.busy = null
  }
}

async function acceptReview(acknowledge: boolean) {
  if (!review.detail) return
  review.busy = 'accept'
  try {
    await store.acceptReview(review.detail.summary.id, acknowledge)
    review.open = false
    notify('Plugin instalado como rascunho. Rode os testes antes de ativar.')
  } catch (err: unknown) {
    notify(describe(err, 'Falha ao instalar'), 'error')
  } finally {
    review.busy = null
  }
}

async function importFile(event: Event) {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  input.value = ''
  if (!file) return
  try {
    review.detail = await store.importPlugin(await readPackageFile(file), null)
    review.open = true
  } catch (err: unknown) {
    notify(describe(err, 'Falha ao importar'), 'error')
  }
}
</script>

<style scoped>
.search {
  flex: 1 1 240px;
  max-width: 360px;
}
</style>
