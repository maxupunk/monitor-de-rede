<template>
  <v-dialog
    :model-value="modelValue"
    max-width="1100"
    scrollable
    :fullscreen="$vuetify.display.smAndDown"
    @update:model-value="close"
  >
    <v-card class="rounded-lg">
      <v-card-title class="d-flex align-center ga-2 flex-wrap">
        <v-icon color="primary">mdi-puzzle-edit-outline</v-icon>
        {{ title }}
        <v-chip v-if="summary" :color="status.color" size="small" variant="flat">
          {{ status.label }}
        </v-chip>
        <v-chip v-if="summary" :color="source.color" size="small" variant="tonal">
          {{ source.label }}
        </v-chip>
      </v-card-title>

      <v-alert v-if="readonly" type="info" variant="tonal" density="compact" class="mx-4 flex-0-0">
        Plugins embutidos não são editados. Duplique-o para criar uma versão sua.
      </v-alert>

      <v-tabs v-model="tab" color="primary" density="comfortable" show-arrows class="flex-0-0">
        <v-tab value="manifest" prepend-icon="mdi-card-text-outline">Manifesto</v-tab>
        <v-tab value="script" prepend-icon="mdi-code-braces">Script</v-tab>
        <v-tab value="usage" prepend-icon="mdi-book-open-variant">Uso</v-tab>
        <v-tab value="tests" prepend-icon="mdi-test-tube">Testes</v-tab>
        <v-tab value="preview" prepend-icon="mdi-eye-outline">Prévia</v-tab>
        <v-tab v-if="compatibility.length" value="compat" prepend-icon="mdi-check-decagram">
          Compatibilidade ({{ compatibility.length }})
        </v-tab>
      </v-tabs>
      <v-divider></v-divider>

      <v-card-text class="editor-body">
        <v-progress-linear v-if="loading" indeterminate color="primary"></v-progress-linear>
        <v-window v-else v-model="tab">
          <v-window-item value="manifest">
            <CodeTextarea
              v-model="manifestText"
              :readonly="readonly"
              :error-messages="jsonError(manifestText)"
              hint="slug, name, version, transports, match e actions (ver a skill de autoria)"
              persistent-hint
            ></CodeTextarea>
          </v-window-item>
          <v-window-item value="script">
            <CodeTextarea
              v-model="script"
              :readonly="readonly"
              hint="Rhai. Cada ação é fn <id>(device, params). Senhas: {{username}} e {{password}}."
              persistent-hint
            ></CodeTextarea>
          </v-window-item>
          <v-window-item value="usage">
            <v-row>
              <v-col cols="12" md="6">
                <CodeTextarea
                  v-model="usage"
                  :readonly="readonly"
                  hint="Markdown com uma seção por ação"
                  persistent-hint
                ></CodeTextarea>
              </v-col>
              <v-col cols="12" md="6">
                <div class="markdown-body usage-preview pa-3 rounded-lg" v-html="usageHtml"></div>
              </v-col>
            </v-row>
          </v-window-item>
          <v-window-item value="tests">
            <CodeTextarea
              v-model="testsText"
              :readonly="readonly"
              :error-messages="jsonError(testsText)"
              hint="unit (fixtures com as respostas reais) e functional (no equipamento)"
              persistent-hint
            ></CodeTextarea>
          </v-window-item>
          <v-window-item value="preview">
            <PluginPreview ref="previewPane" :pkg="parsed" />
          </v-window-item>
          <v-window-item value="compat">
            <v-table density="compact" class="border rounded-lg">
              <thead>
                <tr>
                  <th>Sistema</th>
                  <th>Modelo</th>
                  <th>Firmware</th>
                  <th>Versão do plugin</th>
                  <th>Resultado</th>
                  <th>Quando</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="(entry, index) in compatibility" :key="index">
                  <td>{{ entry.platform ?? '—' }}</td>
                  <td>{{ entry.model ?? '—' }}</td>
                  <td>{{ entry.firmware ?? '—' }}</td>
                  <td>{{ entry.pluginVersion }}</td>
                  <td>
                    <v-chip
                      size="x-small"
                      :color="entry.status === 'passed' ? 'success' : 'error'"
                      variant="flat"
                    >
                      {{ entry.status === 'passed' ? 'Passou' : 'Falhou' }}
                    </v-chip>
                  </td>
                  <td>{{ formatDateTime(entry.validatedAt) }}</td>
                </tr>
              </tbody>
            </v-table>
          </v-window-item>
        </v-window>

        <v-alert
          v-if="report"
          :type="report.passed ? 'success' : 'error'"
          variant="tonal"
          class="mt-4"
        >
          <div class="font-weight-bold">
            {{
              report.passed
                ? `Testes aprovados (${report.total})`
                : `Testes reprovados: ${report.failed} de ${report.total}`
            }}
          </div>
          <ul v-if="report.problems.length" class="ml-4 mt-1 text-body-2">
            <li v-for="problem in report.problems" :key="problem">{{ problem }}</li>
          </ul>
          <div v-if="report.hints.length" class="text-body-2 mt-1">
            {{ report.hints.length }} sugestão(ões) de usabilidade —
            <a href="#" class="font-weight-bold" @click.prevent="tab = 'preview'">ver na Prévia</a>.
          </div>
          <ul class="ml-4 mt-1 text-body-2">
            <li v-for="item in failedCases" :key="item.name">
              <strong>{{ item.name }}</strong
              >: {{ item.message }}
            </li>
          </ul>
        </v-alert>
        <v-alert v-if="saveError" type="error" variant="tonal" class="mt-4">{{
          saveError
        }}</v-alert>
      </v-card-text>

      <v-divider></v-divider>
      <v-card-actions class="flex-wrap ga-2">
        <v-checkbox
          v-if="!pluginId && deviceId"
          v-model="exclusive"
          color="primary"
          density="compact"
          hide-details
          label="Exclusivo deste dispositivo"
        ></v-checkbox>
        <v-spacer></v-spacer>
        <v-btn variant="text" color="secondary" @click="close(false)">Fechar</v-btn>
        <v-btn
          v-if="readonly && pluginId"
          color="primary"
          variant="tonal"
          prepend-icon="mdi-content-copy"
          :loading="busy"
          @click="duplicate"
        >
          Duplicar para editar
        </v-btn>
        <template v-else>
          <v-btn
            color="primary"
            variant="tonal"
            prepend-icon="mdi-content-save-outline"
            :loading="busy"
            :disabled="!parsed"
            @click="save(false)"
          >
            Salvar
          </v-btn>
          <v-btn
            color="primary"
            variant="flat"
            prepend-icon="mdi-test-tube"
            :loading="busy"
            :disabled="!parsed"
            @click="save(true)"
          >
            Salvar e testar
          </v-btn>
        </template>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import type { CompatEntry } from '@/bindings/CompatEntry'
import type { PluginPackage } from '@/bindings/PluginPackage'
import type { PluginSummary } from '@/bindings/PluginSummary'
import type { TestReport } from '@/bindings/TestReport'
import { usePluginsStore } from '@/stores/plugins'
import { formatDateTime } from '@/utils/formatters'
import { renderMarkdown } from '@/utils/markdown'
import { packageTemplate, sourcePresentation, statusPresentation } from '@/utils/pluginPresentation'
import CodeTextarea from '@/components/CodeTextarea.vue'
import PluginPreview from './editor/PluginPreview.vue'

const props = defineProps<{
  modelValue: boolean
  /** Plugin a editar; `null` cria um novo. */
  pluginId: number | null
  /** Dispositivo da tela (para "exclusivo" e para recarregar a aba). */
  deviceId?: number | null
  /** Pacote inicial de um plugin novo (ex.: importado para edição). */
  initial?: PluginPackage | null
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  saved: [pluginId: number]
}>()

const store = usePluginsStore()
const tab = ref('manifest')
const previewPane = ref<{ refresh: () => Promise<void> } | null>(null)
/** A prévia roda sozinha na primeira vez que a aba é aberta. */
const previewed = ref(false)
const loading = ref(false)
const busy = ref(false)
const saveError = ref<string | null>(null)
const report = ref<TestReport | null>(null)
const summary = ref<PluginSummary | null>(null)
const compatibility = ref<CompatEntry[]>([])
const manifestText = ref('')
const script = ref('')
const usage = ref('')
const testsText = ref('')
const exclusive = ref(false)

const readonly = computed(() => summary.value?.source === 'builtin')
const title = computed(() =>
  summary.value ? `${summary.value.name} v${summary.value.version}` : 'Novo plugin'
)
const status = computed(() => statusPresentation(summary.value?.status ?? 'draft'))
const source = computed(() => sourcePresentation(summary.value?.source ?? 'user'))
const usageHtml = computed(() => renderMarkdown(usage.value, { headings: true }))
const failedCases = computed(() => (report.value?.cases ?? []).filter((item) => !item.passed))

function load(pkg: PluginPackage) {
  manifestText.value = JSON.stringify(pkg.manifest, null, 2)
  script.value = pkg.script
  usage.value = pkg.usage
  testsText.value = JSON.stringify(pkg.tests, null, 2)
  compatibility.value = pkg.compatibility ?? []
}

function jsonError(text: string): string | undefined {
  try {
    JSON.parse(text)
    return undefined
  } catch (err: unknown) {
    return err instanceof Error ? `JSON inválido: ${err.message}` : 'JSON inválido'
  }
}

const parsed = computed<PluginPackage | null>(() => {
  try {
    return {
      format: 1,
      manifest: JSON.parse(manifestText.value),
      script: script.value,
      usage: usage.value,
      compatibility: compatibility.value,
      tests: JSON.parse(testsText.value),
    }
  } catch {
    return null
  }
})

watch(
  () => props.modelValue,
  async (open) => {
    if (!open) return
    tab.value = 'manifest'
    previewed.value = false
    report.value = null
    saveError.value = null
    exclusive.value = false
    summary.value = null
    if (props.pluginId === null) {
      load(props.initial ?? packageTemplate())
      return
    }
    loading.value = true
    try {
      const detail = await store.fetchDetail(props.pluginId)
      summary.value = detail.summary
      load(detail.package)
    } catch (err: unknown) {
      saveError.value = err instanceof Error ? err.message : 'Falha ao carregar o plugin'
    } finally {
      loading.value = false
    }
  }
)

watch(tab, async (current) => {
  if (current !== 'preview' || previewed.value) return
  previewed.value = true
  await nextTick()
  await previewPane.value?.refresh()
})

function close(value = false) {
  emit('update:modelValue', value)
}

async function save(test: boolean) {
  const pkg = parsed.value
  if (!pkg) return
  busy.value = true
  saveError.value = null
  report.value = null
  try {
    const detail = summary.value
      ? await store.updatePlugin(summary.value.id, pkg, props.deviceId)
      : await store.createPlugin(
          pkg,
          exclusive.value ? (props.deviceId ?? null) : null,
          props.deviceId
        )
    summary.value = detail.summary
    if (test) report.value = await store.runTests(detail.summary.id, props.deviceId)
    summary.value = (await store.fetchDetail(detail.summary.id)).summary
    emit('saved', detail.summary.id)
  } catch (err: unknown) {
    saveError.value = err instanceof Error ? err.message : 'Falha ao salvar o plugin'
  } finally {
    busy.value = false
  }
}

async function duplicate() {
  if (!props.pluginId) return
  busy.value = true
  saveError.value = null
  try {
    const copy = await store.lifecycle(props.pluginId, 'duplicate', props.deviceId)
    summary.value = copy.summary
    load(copy.package)
    emit('saved', copy.summary.id)
  } catch (err: unknown) {
    saveError.value = err instanceof Error ? err.message : 'Falha ao duplicar o plugin'
  } finally {
    busy.value = false
  }
}
</script>

<style scoped>
.editor-body {
  min-height: 420px;
}
.usage-preview {
  border: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
  /* Mesma altura do campo ao lado: o Markdown longo rola aqui dentro. */
  height: var(--code-editor-height);
  overflow-y: auto;
}
</style>
