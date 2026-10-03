<template>
  <PluginItemList
    :list="list"
    :rows="rows"
    :actions="actionInfo"
    :can-write="!disabled"
    :loading="loading"
    :search-label="searchLabel"
    :presentation="sourceAction"
    @add="runById(list.add, {})"
    @edit="(row) => runTarget(list.edit, row)"
    @remove="(row) => runTarget(list.remove, row)"
    @action="(target, row) => runTarget(target, row)"
    @toolbar="(id) => runById(id, {})"
    @search="load"
  >
    <template #status>
      <div class="d-flex align-center flex-wrap ga-2 mb-2">
        <span class="text-body-2">{{ scopeText }}</span>
        <v-spacer></v-spacer>
        <v-btn
          v-if="!list.searchParam"
          size="small"
          color="primary"
          variant="tonal"
          prepend-icon="mdi-refresh"
          :loading="loading"
          :disabled="disabled"
          @click="load('')"
        >
          {{ listRun ? 'Atualizar' : 'Carregar' }}
        </v-btn>
      </div>
      <v-alert v-if="listRun?.error" type="error" variant="tonal" density="compact" class="mb-3">
        {{ listRun.error }}
      </v-alert>
    </template>
    <template #empty>
      <template v-if="listRun?.status === 'succeeded'">
        Nada encontrado.
        <span v-if="toolbarTitle">Lista vazia? Use “{{ toolbarTitle }}” e busque de novo.</span>
      </template>
      <template v-else>
        {{ list.searchParam ? 'Busque ou carregue a lista.' : 'Carregue a lista.' }}
      </template>
    </template>
  </PluginItemList>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import type { ItemAction } from '@/bindings/ItemAction'
import type { ItemList } from '@/bindings/ItemList'
import type { PluginAction } from '@/bindings/PluginAction'
import PluginItemList from '@/components/plugins/items/PluginItemList.vue'
import { usePluginsStore } from '@/stores/plugins'
import { deviceRows, paramsFromItem, type ItemRow, type ListActionInfo } from '@/utils/itemList'

/**
 * A lista de itens na aba do equipamento: as linhas vêm de `list.source`
 * (com a busca, quando há) e cada botão vira a ação do plugin com os
 * parâmetros tirados do item — quem abre o formulário ou confirma é a aba.
 */
const props = defineProps<{
  deviceId: number
  pluginId: number
  list: ItemList
  actions: PluginAction[]
  /** Plugin ativo: a lista carrega sozinha ao abrir (sem pedir aprovação). */
  autoload: boolean
  disabled?: boolean
  /** Execução disparada por esta lista que, ao terminar bem, a recarrega. */
  lastActionRunId: number | null
}>()

const emit = defineEmits<{
  run: [action: PluginAction, params: Record<string, unknown>]
  error: [message: string]
}>()

const store = usePluginsStore()
const listRunId = ref<number | null>(null)
const starting = ref(false)
const searched = ref('')

const listRun = computed(() => store.runById(props.deviceId, listRunId.value))
const loading = computed(() => starting.value || listRun.value?.status === 'running')
const sourceAction = computed(() => actionOf(props.list.source))
const rows = computed<ItemRow[]>(() =>
  listRun.value?.status === 'succeeded' ? deviceRows(listRun.value.output, props.list) : []
)
const actionInfo = computed<ListActionInfo[]>(() =>
  props.actions.map((action) => ({ id: action.id, title: action.title, effect: action.effect }))
)
const toolbarTitle = computed(() => actionOf(props.list.toolbar?.[0])?.title)
const searchLabel = computed(() => {
  const properties = sourceAction.value?.params?.properties
  const field =
    typeof properties === 'object' && properties !== null && props.list.searchParam
      ? (properties as Record<string, Record<string, unknown>>)[props.list.searchParam]
      : undefined
  return typeof field?.title === 'string' ? field.title : undefined
})
const scopeText = computed(() => {
  if (!props.list.searchParam || !listRun.value) return ''
  return searched.value
    ? `Resultado da busca por “${searched.value}”.`
    : 'Sem busca: mostrando o que já está instalado.'
})

function actionOf(id: string | null | undefined): PluginAction | null {
  return props.actions.find((action) => action.id === id) ?? null
}

function runById(id: string | null | undefined, params: Record<string, unknown>) {
  const action = actionOf(id)
  if (action) emit('run', action, params)
}

function runTarget(target: ItemAction | null | undefined, row: ItemRow) {
  if (target) runById(target.action, paramsFromItem(target, row.item))
}

async function load(term: string) {
  if (props.disabled || !props.list.source) return
  starting.value = true
  const text = term.trim()
  const params: Record<string, unknown> = {}
  if (props.list.searchParam && text) params[props.list.searchParam] = text
  try {
    listRunId.value = await store.runAction(
      props.deviceId,
      props.pluginId,
      props.list.source,
      params,
      false
    )
    searched.value = text
  } catch (err: unknown) {
    emit('error', err instanceof Error ? err.message : 'Falha ao carregar a lista')
  } finally {
    starting.value = false
  }
}

onMounted(() => {
  if (props.autoload && !props.disabled) void load('')
})

// Uma ação desta lista (instalar, remover, atualizar) terminou bem: a lista é
// recarregada uma vez, para mostrar o novo estado.
watch(
  () => store.runById(props.deviceId, props.lastActionRunId)?.status,
  (status, previous) => {
    if (status === 'succeeded' && previous === 'running') void load(searched.value)
  }
)
</script>
