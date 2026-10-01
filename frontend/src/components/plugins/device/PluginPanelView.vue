<template>
  <div>
    <!-- Busca e barra -->
    <div class="d-flex align-center flex-wrap ga-2 mb-3">
      <v-text-field
        v-if="panel.searchParam"
        v-model="search"
        :label="searchLabel"
        prepend-inner-icon="mdi-magnify"
        density="compact"
        variant="outlined"
        hide-details
        clearable
        class="search"
        @keyup.enter="load"
        @click:clear="clearSearch"
      ></v-text-field>
      <v-btn
        color="primary"
        variant="flat"
        :prepend-icon="panel.searchParam ? 'mdi-magnify' : 'mdi-refresh'"
        :loading="loading"
        :disabled="disabled"
        @click="load"
      >
        {{ panel.searchParam ? 'Buscar' : 'Carregar' }}
      </v-btn>
      <v-spacer></v-spacer>
      <v-btn
        v-for="action in toolbarActions"
        :key="action.id"
        :color="action.effect === 'write' ? 'warning' : 'secondary'"
        variant="flat"
        :prepend-icon="effectPresentation(action.effect).icon"
        :disabled="disabled"
        @click="emit('run', action, {})"
      >
        {{ action.title }}
      </v-btn>
    </div>

    <div class="text-body-2 mb-2">{{ scopeText }}</div>

    <v-alert v-if="listRun?.error" type="error" variant="tonal" density="compact" class="mb-3">
      {{ listRun.error }}
    </v-alert>

    <v-progress-linear v-if="loading" indeterminate color="primary" class="mb-2" />

    <v-data-table
      v-if="rows.length"
      :headers="headers"
      :items="rows"
      :item-value="panel.keyColumn"
      :items-per-page="25"
      density="compact"
      class="border rounded-lg panel-table"
      hover
      @click:row="selectRow"
    >
      <template v-for="column in booleanColumns" #[`item.${column}`]="{ value }" :key="column">
        <v-chip size="x-small" :color="value ? 'success' : 'secondary'" variant="flat">
          {{ value ? 'Sim' : 'Não' }}
        </v-chip>
      </template>
      <template #[`item.${panel.keyColumn}`]="{ item }">
        <span class="font-weight-bold" :class="{ 'text-primary': keyOf(item) === selectedKey }">
          <v-icon v-if="keyOf(item) === selectedKey" size="16" color="primary"
            >mdi-check-circle</v-icon
          >
          {{ keyOf(item) }}
        </span>
      </template>
    </v-data-table>
    <div v-else-if="listRun?.status === 'succeeded'" class="text-body-2 py-4">
      Nada encontrado.
      <span v-if="panel.toolbar?.length">
        Lista vazia? Use “{{ toolbarActions[0]?.title }}” e busque de novo.
      </span>
    </div>
    <div v-else-if="!listRun && !loading" class="text-body-2 py-4">
      {{ panel.searchParam ? 'Busque ou carregue a lista.' : 'Carregue a lista.' }}
    </div>

    <!-- Linha escolhida e o que dá para fazer com ela -->
    <v-card v-if="selected" border flat class="rounded-lg mt-3 selection">
      <v-card-text class="d-flex align-center flex-wrap ga-2">
        <v-icon color="primary">mdi-cursor-default-click-outline</v-icon>
        <span class="font-weight-bold">{{ selectedKey }}</span>
        <span v-if="selected.version" class="text-body-2">{{ selected.version }}</span>
        <v-spacer></v-spacer>
        <v-btn
          v-for="row in availableRowActions"
          :key="row.action"
          :color="actionOf(row.action)?.effect === 'write' ? 'error' : 'primary'"
          variant="flat"
          :prepend-icon="row.icon ?? 'mdi-play'"
          :disabled="disabled"
          @click="runRow(row)"
        >
          {{ row.label }}
        </v-btn>
        <v-btn variant="text" color="secondary" @click="selectedKey = null">Limpar</v-btn>
      </v-card-text>
    </v-card>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import type { PanelRowAction } from '@/bindings/PanelRowAction'
import type { PluginAction } from '@/bindings/PluginAction'
import type { PluginPanel } from '@/bindings/PluginPanel'
import { usePluginsStore } from '@/stores/plugins'
import { effectPresentation } from '@/utils/pluginPresentation'

type Row = Record<string, unknown>

const props = defineProps<{
  deviceId: number
  pluginId: number
  panel: PluginPanel
  actions: PluginAction[]
  /** Plugin ativo: a lista carrega sozinha ao abrir (sem pedir aprovação). */
  autoload: boolean
  disabled?: boolean
  /** Execução de ação disparada por este painel que, ao terminar bem, recarrega a lista. */
  lastActionRunId: number | null
}>()

const emit = defineEmits<{
  run: [action: PluginAction, params: Record<string, unknown>]
  error: [message: string]
}>()

const store = usePluginsStore()
const search = ref('')
const searched = ref('')
const listRunId = ref<number | null>(null)
const starting = ref(false)
const selectedKey = ref<string | null>(null)

const listRun = computed(() => store.runById(props.deviceId, listRunId.value))
const loading = computed(() => starting.value || listRun.value?.status === 'running')
const listAction = computed(() => actionOf(props.panel.listAction))
const searchLabel = computed(() => {
  const schema = listAction.value?.params?.properties
  const field =
    typeof schema === 'object' && schema !== null && props.panel.searchParam
      ? (schema as Record<string, Record<string, unknown>>)[props.panel.searchParam]
      : undefined
  return typeof field?.title === 'string' ? field.title : 'Buscar'
})
const scopeText = computed(() => {
  if (!props.panel.searchParam || !listRun.value) return ''
  return searched.value
    ? `Resultado da busca por “${searched.value}”.`
    : 'Sem busca: mostrando o que já está instalado.'
})

const rows = computed<Row[]>(() => {
  const output = listRun.value?.status === 'succeeded' ? listRun.value.output : null
  return Array.isArray(output)
    ? output.filter((row): row is Row => typeof row === 'object' && row !== null)
    : []
})

/** Coluna sem nenhum valor (ex.: descrição na lista de instalados) não aparece. */
function hasValues(key: string): boolean {
  return rows.value.some((row) => row[key] !== '' && row[key] !== null && row[key] !== undefined)
}

const declared = computed(() => props.panel.columns ?? [])
const columns = computed(() => {
  if (declared.value.length) {
    return declared.value
      .map((column) => column.key)
      .filter((key) => key === props.panel.keyColumn || hasValues(key))
  }
  const keys = new Set<string>()
  for (const row of rows.value.slice(0, 50)) Object.keys(row).forEach((key) => keys.add(key))
  const list = [...keys].filter((key) => key !== props.panel.keyColumn && hasValues(key))
  return [props.panel.keyColumn, ...list]
})
const booleanColumns = computed(() =>
  columns.value.filter((key) => rows.value.some((row) => typeof row[key] === 'boolean'))
)
const headers = computed(() =>
  columns.value.map((key) => ({ title: label(key), key, sortable: true }))
)

const selected = computed<Row | null>(
  () => rows.value.find((row) => keyOf(row) === selectedKey.value) ?? null
)
const toolbarActions = computed(() =>
  (props.panel.toolbar ?? [])
    .map((id) => actionOf(id))
    .filter((action): action is PluginAction => action !== null)
)
const availableRowActions = computed(() =>
  (props.panel.rowActions ?? []).filter((row) => {
    const value = selected.value
    if (!value) return false
    if (row.showWhen && value[row.showWhen] !== true) return false
    if (row.hideWhen && value[row.hideWhen] === true) return false
    return actionOf(row.action) !== null
  })
)

function actionOf(id: string): PluginAction | null {
  return props.actions.find((action) => action.id === id) ?? null
}

function label(key: string): string {
  const named = declared.value.find((column) => column.key === key)
  if (named) return named.label
  const text = key.replace(/_/g, ' ')
  return text.charAt(0).toUpperCase() + text.slice(1)
}

function keyOf(row: Row): string {
  return String(row[props.panel.keyColumn] ?? '')
}

function selectRow(_event: Event, { item }: { item: Row }) {
  selectedKey.value = keyOf(item)
}

function clearSearch() {
  search.value = ''
  void load()
}

async function load() {
  if (props.disabled) return
  starting.value = true
  const term = (search.value ?? '').trim()
  const params: Record<string, unknown> = {}
  if (props.panel.searchParam && term) params[props.panel.searchParam] = term
  try {
    listRunId.value = await store.runAction(
      props.deviceId,
      props.pluginId,
      props.panel.listAction,
      params,
      false
    )
    searched.value = term
    selectedKey.value = null
  } catch (err: unknown) {
    emit('error', err instanceof Error ? err.message : 'Falha ao carregar a lista')
  } finally {
    starting.value = false
  }
}

function runRow(row: PanelRowAction) {
  const action = actionOf(row.action)
  const value = selected.value
  if (!action || !value) return
  const params: Record<string, unknown> = {}
  for (const [param, column] of Object.entries(row.params)) params[param] = value[column]
  emit('run', action, params)
}

onMounted(() => {
  if (props.autoload && !props.disabled) void load()
})

// Uma ação do painel (instalar, remover, atualizar a lista) terminou bem: a
// lista é recarregada uma vez, para mostrar o novo estado.
watch(
  () => store.runById(props.deviceId, props.lastActionRunId)?.status,
  (status, previous) => {
    if (status === 'succeeded' && previous === 'running') void load()
  }
)
</script>

<style scoped>
.search {
  max-width: 360px;
  min-width: 200px;
}
.panel-table :deep(tbody tr) {
  cursor: pointer;
}
.selection {
  border-color: rgb(var(--v-theme-primary)) !important;
}
</style>
