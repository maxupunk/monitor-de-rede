<template>
  <div>
    <PageHeader
      title="Plugins de dispositivo"
      subtitle="Drivers que agem nos equipamentos por SSH, HTTP ou Telnet — criados, importados ou gerados pela IA"
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

    <v-alert type="info" variant="tonal" class="mb-4" density="comfortable">
      Plugins são usados na aba <strong>Plugins</strong> de cada dispositivo. Importados entram em
      quarentena com revisão de segurança; rascunhos pedem aprovação a cada acesso até serem
      testados e ativados.
    </v-alert>
    <v-alert v-if="store.error" type="error" variant="tonal" class="mb-4">{{
      store.error
    }}</v-alert>

    <v-card v-if="appsStore.apps.length > 0" elevation="2" class="rounded-lg mb-4">
      <v-card-title class="d-flex align-center ga-2">
        <v-icon color="primary">mdi-apps</v-icon>
        Aplicativos
      </v-card-title>
      <v-card-subtitle>
        Plugins que trabalham com vários equipamentos de uma vez — também no menu Aplicativos.
      </v-card-subtitle>
      <v-card-text>
        <v-row dense>
          <v-col v-for="app in appsStore.apps" :key="app.id" cols="12" sm="6" lg="4">
            <v-card border flat class="rounded-lg h-100" :to="'/apps/' + app.id">
              <v-card-item
                :prepend-icon="app.icon"
                :title="app.title"
                :subtitle="`${app.members} equipamento(s)`"
              ></v-card-item>
              <v-card-text v-if="app.description" class="text-body-2">
                {{ app.description }}
              </v-card-text>
            </v-card>
          </v-col>
        </v-row>
      </v-card-text>
    </v-card>

    <v-card elevation="2" class="rounded-lg">
      <v-card-text>
        <v-text-field
          v-model="search"
          prepend-inner-icon="mdi-magnify"
          label="Buscar por nome ou slug"
          variant="outlined"
          density="compact"
          hide-details
          class="mb-4"
        ></v-text-field>
        <v-data-table
          :headers="headers"
          :items="filtered"
          :loading="store.libraryLoading"
          item-value="id"
          density="comfortable"
          no-data-text="Nenhum plugin"
        >
          <template #[`item.name`]="{ item }">
            <div class="font-weight-bold">{{ item.name }}</div>
            <div class="text-body-small font-mono">{{ item.slug }} · v{{ item.version }}</div>
          </template>
          <template #[`item.status`]="{ item }">
            <v-chip size="small" :color="statusPresentation(item.status).color" variant="flat">
              {{ statusPresentation(item.status).label }}
            </v-chip>
          </template>
          <template #[`item.source`]="{ item }">
            <v-chip size="small" :color="sourcePresentation(item.source).color" variant="tonal">
              {{ sourcePresentation(item.source).label }}
            </v-chip>
            <v-chip
              v-if="item.deviceId"
              size="small"
              color="secondary"
              variant="tonal"
              class="ml-1"
            >
              Exclusivo
            </v-chip>
          </template>
          <template #[`item.risk`]="{ item }">
            <v-chip
              v-if="item.risk"
              size="small"
              :color="severityPresentation(item.risk).color"
              variant="tonal"
            >
              {{ severityPresentation(item.risk).label }}
            </v-chip>
            <span v-else>—</span>
          </template>
          <template #[`item.validatedCount`]="{ item }">
            <v-chip
              size="small"
              :color="item.validatedCount > 0 ? 'success' : 'warning'"
              variant="tonal"
            >
              {{ item.validatedCount }} equipamento(s)
            </v-chip>
          </template>
          <template #[`item.actions`]="{ item }">
            <div class="d-flex justify-end ga-1">
              <v-btn
                v-if="item.status === 'quarantine'"
                size="small"
                color="error"
                variant="flat"
                @click="openReview(item.id)"
              >
                Revisão
              </v-btn>
              <v-btn
                v-if="item.status === 'tested'"
                size="small"
                color="success"
                variant="flat"
                @click="step(item, 'promote')"
              >
                Ativar
              </v-btn>
              <v-btn
                v-if="opensApp(item)"
                size="small"
                color="primary"
                variant="flat"
                :prepend-icon="item.fleet?.icon ?? 'mdi-apps'"
                :to="'/apps/' + item.id"
              >
                Abrir
              </v-btn>
              <v-btn
                icon="mdi-pencil-outline"
                size="small"
                variant="text"
                color="primary"
                :aria-label="`Abrir ${item.name}`"
                @click="openEditor(item.id)"
              />
              <v-btn
                icon="mdi-download-outline"
                size="small"
                variant="text"
                color="secondary"
                :aria-label="`Exportar ${item.name}`"
                @click="store.exportPlugin(item)"
              />
              <v-btn
                :icon="
                  item.status === 'disabled'
                    ? 'mdi-power-plug-outline'
                    : 'mdi-power-plug-off-outline'
                "
                size="small"
                variant="text"
                :color="item.status === 'disabled' ? 'success' : 'warning'"
                :aria-label="item.status === 'disabled' ? 'Reativar' : 'Desativar'"
                @click="step(item, item.status === 'disabled' ? 'enable' : 'disable')"
              />
              <v-btn
                v-if="item.source !== 'builtin'"
                icon="mdi-delete-outline"
                size="small"
                variant="text"
                color="error"
                :aria-label="`Excluir ${item.name}`"
                @click="remove(item)"
              />
            </div>
          </template>
        </v-data-table>
      </v-card-text>
    </v-card>

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
import PluginReviewDialog from '@/components/plugins/PluginReviewDialog.vue'
import { confirm } from '@/composables/useConfirm'
import { usePluginsStore } from '@/stores/plugins'
import { usePluginAppsStore } from '@/stores/pluginApps'
import {
  readPackageFile,
  severityPresentation,
  sourcePresentation,
  statusPresentation,
} from '@/utils/pluginPresentation'

const store = usePluginsStore()
const appsStore = usePluginAppsStore()
const fileInput = ref<HTMLInputElement | null>(null)
const search = ref('')
const feedback = reactive({ show: false, text: '', color: 'success' })
const editor = reactive({ open: false, pluginId: null as number | null })
const review = reactive({
  open: false,
  detail: null as PluginDetail | null,
  busy: null as 'review' | 'accept' | null,
})

const headers = [
  { title: 'Plugin', key: 'name' },
  { title: 'Status', key: 'status' },
  { title: 'Origem', key: 'source' },
  { title: 'Risco', key: 'risk' },
  { title: 'Validado em', key: 'validatedCount' },
  { title: '', key: 'actions', sortable: false, align: 'end' as const },
]

const filtered = computed(() => {
  const term = search.value.trim().toLowerCase()
  if (!term) return store.library
  return store.library.filter(
    (item) => item.name.toLowerCase().includes(term) || item.slug.includes(term)
  )
})

onMounted(() => {
  void store.fetchLibrary()
  void appsStore.fetchApps()
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

/** Plugin de frota ligado tem a página dele em Aplicativos. */
function opensApp(item: PluginSummary): boolean {
  return (
    item.surfaces.includes('fleet') && item.status !== 'disabled' && item.status !== 'quarantine'
  )
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
