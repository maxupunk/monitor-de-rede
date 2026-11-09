<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 900"
    :fullscreen="$vuetify.display.xs"
    scrollable
    persistent
    @update:model-value="close"
  >
    <v-card class="rounded-lg">
      <v-card-item class="pt-4 px-6">
        <template #prepend>
          <v-avatar color="primary" variant="tonal" rounded="lg">
            <v-icon>mdi-file-key</v-icon>
          </v-avatar>
        </template>
        <v-card-title class="font-weight-bold">Variáveis (.env) — {{ target?.name }}</v-card-title>
        <v-card-subtitle class="text-wrap text-high-emphasis">
          Guardadas cifradas na central. Cada versão salva fica no histórico, e o deploy grava no
          servidor a versão que usou — o rollback volta a imagem e o .env juntos.
        </v-card-subtitle>
      </v-card-item>

      <v-tabs v-model="tab" color="primary" class="px-4">
        <v-tab value="edit" prepend-icon="mdi-pencil">Editar</v-tab>
        <v-tab value="history" prepend-icon="mdi-history">Histórico</v-tab>
      </v-tabs>

      <v-card-text class="px-6">
        <v-alert
          v-if="error"
          type="error"
          variant="tonal"
          closable
          class="mb-4"
          @click:close="error = null"
        >
          {{ error }}
        </v-alert>

        <v-window v-model="tab">
          <v-window-item value="edit">
            <v-skeleton-loader v-if="loading" type="table-row@6"></v-skeleton-loader>
            <template v-else>
              <div class="d-flex flex-wrap align-center ga-2 mb-3">
                <v-btn-toggle v-model="mode" color="primary" density="compact" mandatory divided>
                  <v-btn value="list" prepend-icon="mdi-format-list-bulleted">Lista</v-btn>
                  <v-btn value="raw" prepend-icon="mdi-text-box-outline">Texto (.env)</v-btn>
                </v-btn-toggle>
                <v-spacer></v-spacer>
                <v-text-field
                  v-if="mode === 'list'"
                  v-model="search"
                  prepend-inner-icon="mdi-magnify"
                  placeholder="Filtrar (ex.: WUZAPI_)"
                  density="compact"
                  variant="outlined"
                  hide-details
                  clearable
                  class="env-search"
                ></v-text-field>
              </div>

              <template v-if="mode === 'list'">
                <div
                  v-for="row in visibleRows"
                  :key="row.index"
                  class="d-flex align-start ga-2 mb-2"
                  data-test="env-row"
                >
                  <v-text-field
                    :model-value="row.key"
                    label="Variável"
                    density="compact"
                    variant="outlined"
                    :error-messages="isValidEnvKey(row.key) ? [] : ['Nome inválido']"
                    class="env-key"
                    @update:model-value="updateKey(row.index, $event)"
                  ></v-text-field>
                  <v-text-field
                    :model-value="row.value"
                    label="Valor"
                    density="compact"
                    variant="outlined"
                    :type="row.secret && !revealed.has(row.index) ? 'password' : 'text'"
                    autocomplete="off"
                    class="flex-grow-1"
                    @update:model-value="updateValue(row.index, $event)"
                  >
                    <template v-if="row.secret" #append-inner>
                      <v-icon
                        :aria-label="revealed.has(row.index) ? 'Esconder' : 'Mostrar'"
                        @click="toggleReveal(row.index)"
                      >
                        {{ revealed.has(row.index) ? 'mdi-eye-off' : 'mdi-eye' }}
                      </v-icon>
                    </template>
                  </v-text-field>
                  <v-btn
                    icon="mdi-delete"
                    color="error"
                    variant="text"
                    :aria-label="`Remover ${row.key}`"
                    @click="removeRow(row.index)"
                  ></v-btn>
                </div>
                <div v-if="!visibleRows.length" class="text-body-2 text-high-emphasis mb-3">
                  {{ search ? 'Nenhuma variável com esse nome.' : 'Nenhuma variável ainda.' }}
                </div>
                <div class="d-flex flex-wrap align-center ga-2">
                  <v-btn color="primary" variant="tonal" prepend-icon="mdi-plus" @click="addRow">
                    Adicionar variável
                  </v-btn>
                  <span v-if="otherLines" class="text-body-2 text-high-emphasis">
                    {{ otherLines }} linha(s) de comentário preservada(s) — edite no modo texto.
                  </span>
                </div>
              </template>
              <CodeTextarea
                v-else
                v-model="content"
                height="min(50vh, 460px)"
                placeholder="Cole aqui o .env inteiro (comentários são preservados)"
                hide-details
              />

              <v-alert
                v-if="diff"
                :type="diffEmpty ? 'info' : 'warning'"
                variant="tonal"
                class="mt-4"
                data-test="env-diff"
              >
                <div class="font-weight-bold mb-2">
                  {{
                    diffEmpty ? 'Nenhuma variável mudou.' : 'O que muda em relação à versão atual'
                  }}
                </div>
                <div
                  v-for="group in diffGroups"
                  :key="group.label"
                  class="d-flex flex-wrap align-center ga-1 mb-1"
                >
                  <span class="text-body-2 font-weight-bold mr-1">{{ group.label }}:</span>
                  <v-chip
                    v-for="key in group.keys"
                    :key="key"
                    :color="group.color"
                    size="small"
                    variant="tonal"
                    label
                  >
                    {{ key }}
                  </v-chip>
                </div>
              </v-alert>
            </template>
          </v-window-item>

          <v-window-item value="history">
            <v-list v-if="versions.length" density="compact" class="bg-transparent pa-0">
              <v-list-item
                v-for="version in versions"
                :key="version.id"
                :title="versionTitle(version)"
                :subtitle="version.note ?? `${version.keys.length} variável(is)`"
                class="px-0"
              >
                <template #prepend>
                  <v-icon :color="version.current ? 'success' : 'primary'" class="mr-3">
                    {{ version.current ? 'mdi-check-decagram' : 'mdi-file-clock' }}
                  </v-icon>
                </template>
                <template #append>
                  <v-btn
                    size="small"
                    color="primary"
                    variant="tonal"
                    prepend-icon="mdi-file-replace-outline"
                    :loading="loadingVersion === version.id"
                    @click="loadVersion(version.id)"
                  >
                    Abrir no editor
                  </v-btn>
                </template>
              </v-list-item>
            </v-list>
            <div v-else class="text-body-2 text-high-emphasis py-4">
              Nenhuma versão salva. A primeira vira o .env do próximo deploy.
            </div>
          </v-window-item>
        </v-window>
      </v-card-text>

      <v-card-actions class="px-6 pb-4 flex-wrap ga-2">
        <v-btn variant="text" @click="close">Fechar</v-btn>
        <v-spacer></v-spacer>
        <template v-if="tab === 'edit' && !loading">
          <v-btn
            v-if="!diff"
            color="primary"
            variant="flat"
            prepend-icon="mdi-file-compare"
            :loading="previewing"
            :disabled="!dirty || invalidKeys"
            @click="review"
          >
            Revisar mudanças
          </v-btn>
          <template v-else>
            <v-btn variant="text" color="primary" @click="diff = null">Voltar a editar</v-btn>
            <v-btn
              color="primary"
              variant="tonal"
              prepend-icon="mdi-content-save"
              :loading="saving === 'save'"
              @click="save(false)"
            >
              Salvar para o próximo deploy
            </v-btn>
            <v-btn
              color="success"
              variant="flat"
              prepend-icon="mdi-rocket-launch"
              :loading="saving === 'apply'"
              :disabled="!target?.live"
              @click="save(true)"
            >
              Salvar e aplicar agora
              <v-tooltip v-if="!target?.live" activator="parent" location="top" max-width="280">
                Este alvo ainda não tem versão no ar: o .env entra no primeiro deploy.
              </v-tooltip>
            </v-btn>
          </template>
        </template>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import CodeTextarea from '@/components/CodeTextarea.vue'
import { useConfirm } from '@/composables/useConfirm'
import type { DeployEnvVersionView } from '@/bindings/DeployEnvVersionView'
import type { DeployRunView } from '@/bindings/DeployRunView'
import type { DeployTargetView } from '@/bindings/DeployTargetView'
import type { DeployEnvDiff } from '@/bindings/DeployEnvDiff'
import { useDeploysStore } from '@/stores/deploys'
import { isValidEnvKey, looksSecret, parseEnv, serializeEnv, type EnvLine } from '@/utils/envFile'
import { formatShortDateTime } from '@/utils/formatters'

const props = defineProps<{ modelValue: boolean; target: DeployTargetView | null }>()
const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  saved: [message: string, run: DeployRunView | null]
}>()

const deploys = useDeploysStore()
const { confirm } = useConfirm()

const tab = ref<'edit' | 'history'>('edit')
const mode = ref<'list' | 'raw'>('list')
const loading = ref(false)
const error = ref<string | null>(null)
const original = ref('')
const content = ref('')
const versions = ref<DeployEnvVersionView[]>([])
const search = ref('')
const revealed = reactive(new Set<number>())
const diff = ref<DeployEnvDiff | null>(null)
const previewing = ref(false)
const saving = ref<'save' | 'apply' | null>(null)
const loadingVersion = ref<number | null>(null)

const lines = computed<EnvLine[]>(() => parseEnv(content.value))
const dirty = computed(() => content.value !== original.value)
const otherLines = computed(
  () => lines.value.filter((line) => line.kind === 'other' && line.text.trim() !== '').length
)
const invalidKeys = computed(() =>
  lines.value.some((line) => line.kind === 'variable' && !isValidEnvKey(line.key))
)

interface Row {
  index: number
  key: string
  value: string
  secret: boolean
}

const visibleRows = computed<Row[]>(() => {
  const term = (search.value ?? '').trim().toLowerCase()
  const rows: Row[] = []
  lines.value.forEach((line, index) => {
    if (line.kind !== 'variable') return
    if (term && !line.key.toLowerCase().includes(term)) return
    rows.push({ index, key: line.key, value: line.value, secret: looksSecret(line.key) })
  })
  return rows
})

function rewrite(change: (lines: EnvLine[]) => void) {
  const next = parseEnv(content.value)
  change(next)
  content.value = serializeEnv(next)
  diff.value = null
}

function updateKey(index: number, key: string) {
  rewrite((next) => {
    const line = next[index]
    if (line?.kind === 'variable') line.key = key.trim()
  })
}

function updateValue(index: number, value: string) {
  rewrite((next) => {
    const line = next[index]
    if (line?.kind === 'variable') line.value = value
  })
}

function removeRow(index: number) {
  rewrite((next) => next.splice(index, 1))
}

function addRow() {
  search.value = ''
  rewrite((next) =>
    next.push({ kind: 'variable', key: 'NOVA_VARIAVEL', value: '', exported: false })
  )
}

function toggleReveal(index: number) {
  if (revealed.has(index)) revealed.delete(index)
  else revealed.add(index)
}

watch(content, () => {
  diff.value = null
})

const diffEmpty = computed(
  () =>
    !diff.value ||
    (diff.value.added.length === 0 &&
      diff.value.changed.length === 0 &&
      diff.value.removed.length === 0)
)
const diffGroups = computed(() =>
  diff.value
    ? [
        { label: 'Novas', keys: diff.value.added, color: 'success' },
        { label: 'Alteradas', keys: diff.value.changed, color: 'warning' },
        { label: 'Removidas', keys: diff.value.removed, color: 'error' },
      ].filter((group) => group.keys.length > 0)
    : []
)

function versionTitle(version: DeployEnvVersionView): string {
  const author = version.createdBy ? ` · ${version.createdBy}` : ''
  const current = version.current ? ' (em vigor)' : ''
  return `${formatShortDateTime(version.createdAt)}${author}${current}`
}

async function load() {
  if (!props.target) return
  loading.value = true
  error.value = null
  diff.value = null
  revealed.clear()
  try {
    const [current, history] = await Promise.all([
      deploys.envCurrent(props.target.id),
      deploys.envVersions(props.target.id),
    ])
    original.value = current.content
    content.value = current.content
    versions.value = history
    mode.value = current.content === '' ? 'raw' : 'list'
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Não foi possível abrir o .env'
  } finally {
    loading.value = false
  }
}

async function loadVersion(id: number) {
  loadingVersion.value = id
  try {
    content.value = (await deploys.envVersion(id)).content
    tab.value = 'edit'
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Não foi possível abrir a versão'
  } finally {
    loadingVersion.value = null
  }
}

async function review() {
  if (!props.target) return
  previewing.value = true
  error.value = null
  try {
    diff.value = await deploys.envPreview(props.target.id, content.value)
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'O .env tem uma linha inválida'
  } finally {
    previewing.value = false
  }
}

async function save(apply: boolean) {
  if (!props.target) return
  saving.value = apply ? 'apply' : 'save'
  error.value = null
  try {
    const saved = await deploys.envSave(props.target.id, content.value, apply)
    original.value = content.value
    emit(
      'saved',
      apply
        ? 'Variáveis salvas — o deploy com o .env novo já começou.'
        : 'Variáveis salvas: entram no próximo deploy.',
      saved.run
    )
    emit('update:modelValue', false)
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Não foi possível salvar o .env'
  } finally {
    saving.value = null
  }
}

async function close() {
  if (dirty.value) {
    const discard = await confirm({
      title: 'Descartar as mudanças?',
      message: 'As variáveis editadas ainda não foram salvas.',
      confirmText: 'Descartar',
      confirmColor: 'warning',
      icon: 'mdi-file-undo',
    })
    if (!discard) return
  }
  emit('update:modelValue', false)
}

watch(
  () => [props.modelValue, props.target?.id] as const,
  ([visible]) => {
    if (visible) {
      tab.value = 'edit'
      search.value = ''
      void load()
    }
  },
  { immediate: true }
)
</script>

<style scoped>
.env-search {
  max-width: 280px;
}

.env-key {
  max-width: 260px;
  flex: 0 0 38%;
}
</style>
