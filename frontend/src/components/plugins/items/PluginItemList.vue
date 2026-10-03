<template>
  <div>
    <!-- Cabeçalho: nome da lista, busca, barra e "Adicionar" -->
    <div class="d-flex align-center flex-wrap ga-2 mb-3">
      <v-icon color="primary">{{ list.icon ?? 'mdi-format-list-bulleted' }}</v-icon>
      <span class="text-h6 font-weight-bold">{{ list.title ?? 'Itens' }}</span>
      <v-chip size="small" color="primary" variant="tonal">{{ rows.length }}</v-chip>
      <v-spacer></v-spacer>
      <template v-if="list.searchParam">
        <v-text-field
          v-model="term"
          :label="searchLabel ?? 'Buscar'"
          prepend-inner-icon="mdi-magnify"
          density="compact"
          variant="outlined"
          hide-details
          clearable
          class="search"
          @keyup.enter="emit('search', term ?? '')"
          @click:clear="clearSearch"
        ></v-text-field>
        <v-btn
          color="primary"
          variant="flat"
          prepend-icon="mdi-magnify"
          :loading="loading"
          @click="emit('search', term ?? '')"
        >
          Buscar
        </v-btn>
      </template>
      <v-btn
        v-for="action in toolbar"
        :key="action.id"
        :color="action.effect === 'write' ? 'warning' : 'secondary'"
        variant="flat"
        :prepend-icon="action.icon ?? effectPresentation(action.effect).icon"
        :disabled="!canWrite"
        @click="emit('toolbar', action.id)"
      >
        {{ action.title }}
      </v-btn>
      <v-btn
        v-if="canWrite && list.add"
        color="primary"
        variant="flat"
        prepend-icon="mdi-plus"
        @click="emit('add')"
      >
        {{ addLabel }}
      </v-btn>
    </div>

    <slot name="status"></slot>
    <v-progress-linear v-if="loading" indeterminate color="primary" class="mb-2" />

    <!-- Cartões: poucos itens com estado (redes, túneis) -->
    <v-row v-if="list.layout === 'cards'" dense>
      <v-col v-for="row in rows" :key="row.key" cols="12" md="6" xl="4">
        <v-card
          border
          flat
          class="rounded-lg h-100"
          :link="canEdit(row)"
          @click="canEdit(row) && emit('edit', row)"
        >
          <v-card-item>
            <template #prepend>
              <v-avatar :color="row.active ? 'primary' : 'secondary'" variant="tonal" rounded="lg">
                <v-icon>{{ list.icon ?? 'mdi-circle-outline' }}</v-icon>
              </v-avatar>
            </template>
            <v-card-title class="font-weight-bold">{{ row.key }}</v-card-title>
            <v-card-subtitle v-if="row.subtitle">{{ row.subtitle }}</v-card-subtitle>
            <template v-if="canWrite && buttonsFor(row).length > 0" #append>
              <v-menu>
                <template #activator="{ props: menu }">
                  <v-btn
                    v-bind="menu"
                    icon="mdi-dots-vertical"
                    size="small"
                    variant="text"
                    color="primary"
                    :aria-label="`Opções de ${row.key}`"
                    @click.stop
                  />
                </template>
                <v-list density="compact">
                  <v-list-item
                    v-for="button in buttonsFor(row)"
                    :key="button.key"
                    :prepend-icon="button.icon"
                    :base-color="button.color === 'error' ? 'error' : undefined"
                    :title="button.label"
                    @click="press(button, row)"
                  ></v-list-item>
                </v-list>
              </v-menu>
            </template>
          </v-card-item>
          <v-card-text v-if="row.entries.length > 0 || row.detailTotal !== null" class="pt-0">
            <div v-if="row.entries.length > 0" class="d-flex flex-wrap ga-1 mb-2">
              <v-chip
                v-for="entry in row.entries"
                :key="entry.deviceId"
                size="small"
                :color="stateColor(entry.state)"
                variant="tonal"
                prepend-icon="mdi-router-wireless"
              >
                {{ entry.name }}
              </v-chip>
            </div>
            <div class="text-body-2">{{ footnote(row) }}</div>
          </v-card-text>
        </v-card>
      </v-col>
    </v-row>

    <!-- Tabela: muitos itens (pacotes, regras), com seleção -->
    <template v-else>
      <v-data-table
        v-if="rows.length"
        :headers="headers"
        :items="tableItems"
        item-value="key"
        :items-per-page="25"
        density="compact"
        class="border rounded-lg item-table"
        hover
        @click:row="selectRow"
      >
        <template v-for="column in columns" #[`item.${column}`]="{ item }" :key="column">
          <span
            v-if="column === list.key"
            class="font-weight-bold"
            :class="{ 'text-primary': item.key === selectedKey }"
          >
            <v-icon v-if="item.key === selectedKey" size="16" color="primary">
              mdi-check-circle
            </v-icon>
            {{ item.key }}
          </span>
          <v-chip
            v-else-if="typeof item.row.item[column] === 'boolean'"
            size="x-small"
            :color="item.row.item[column] ? 'success' : 'secondary'"
            variant="flat"
          >
            {{ item.row.item[column] ? 'Sim' : 'Não' }}
          </v-chip>
          <v-chip
            v-else-if="isState(column, item.row.item[column])"
            size="x-small"
            :color="stateColor(item.row.item[column])"
            variant="flat"
          >
            {{ stateLabel(item.row.item[column]) }}
          </v-chip>
          <span v-else>{{ cell(column, item.row.item[column]) }}</span>
        </template>
      </v-data-table>

      <v-card v-if="selected" border flat class="rounded-lg mt-3 selection">
        <v-card-text class="d-flex align-center flex-wrap ga-2">
          <v-icon color="primary">mdi-cursor-default-click-outline</v-icon>
          <span class="font-weight-bold">{{ selected.key }}</span>
          <span v-if="selected.subtitle" class="text-body-2">{{ selected.subtitle }}</span>
          <v-spacer></v-spacer>
          <v-btn
            v-for="button in buttonsFor(selected)"
            :key="button.key"
            :color="button.color"
            variant="flat"
            :prepend-icon="button.icon"
            :disabled="!canWrite"
            @click="press(button, selected)"
          >
            {{ button.label }}
          </v-btn>
          <v-btn variant="text" color="primary" @click="selectedKey = null">Limpar</v-btn>
        </v-card-text>
      </v-card>
    </template>

    <div v-if="rows.length === 0 && !loading" class="text-body-2 py-4">
      <slot name="empty">A lista está vazia.</slot>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { ItemAction } from '@/bindings/ItemAction'
import type { ItemList } from '@/bindings/ItemList'
import { appliesTo, itemActionsOf, type ItemRow, type ListActionInfo } from '@/utils/itemList'
import {
  effectPresentation,
  formatOutputValue,
  keyLabel,
  outputFormat,
  outputLabel,
  stateColor,
  stateLabel,
  type OutputPresentation,
} from '@/utils/pluginPresentation'

/**
 * A lista de itens de um plugin — a mesma no equipamento e na frota. Só
 * desenha e avisa o que o operador escolheu; quem a usa decide de onde vêm as
 * linhas e o que cada botão roda.
 */
const props = defineProps<{
  list: ItemList
  rows: ItemRow[]
  actions: ListActionInfo[]
  canWrite: boolean
  loading?: boolean
  /** Na frota: quantos equipamentos há ("Em 2 de 3 equipamentos"). */
  members?: number
  /** Título do campo de busca (o do parâmetro). */
  searchLabel?: string
  /** Títulos e formatos das colunas — a ação que dá a lista. */
  presentation?: OutputPresentation
}>()

const emit = defineEmits<{
  add: []
  edit: [row: ItemRow]
  remove: [row: ItemRow]
  action: [target: ItemAction, row: ItemRow]
  toolbar: [actionId: string]
  search: [term: string]
}>()

interface Button {
  key: string
  kind: 'edit' | 'row' | 'remove'
  target: ItemAction
  label: string
  icon: string
  color: string
}

const term = ref<string | null>('')
const selectedKey = ref<string | null>(null)

const itemName = computed(() => props.list.itemName ?? 'item')
const addLabel = computed(() => `Adicionar ${itemName.value}`)

function infoOf(id: string): ListActionInfo | undefined {
  return props.actions.find((action) => action.id === id)
}

const toolbar = computed(() =>
  (props.list.toolbar ?? [])
    .map((id) => infoOf(id))
    .filter((action): action is ListActionInfo => action !== undefined)
)

const DEFAULT_ICON = { edit: 'mdi-pencil-outline', row: 'mdi-play', remove: 'mdi-delete-outline' }
const DEFAULT_LABEL = { edit: 'Editar', row: '', remove: 'Remover' }

/** Os botões que valem para o item, na ordem: editar, outras ações, remover. */
function buttonsFor(row: ItemRow): Button[] {
  return itemActionsOf(props.list)
    .filter(({ target }) => appliesTo(target, row.item) && infoOf(target.action) !== undefined)
    .map(({ kind, target }, index) => {
      const info = infoOf(target.action)
      return {
        key: `${kind}-${index}`,
        kind,
        target,
        label: target.label ?? (DEFAULT_LABEL[kind] || info?.title || target.action),
        icon: target.icon ?? DEFAULT_ICON[kind],
        color: kind === 'remove' ? 'error' : info?.effect === 'write' ? 'warning' : 'primary',
      }
    })
}

function canEdit(row: ItemRow): boolean {
  return props.canWrite && props.list.edit !== undefined && appliesTo(props.list.edit, row.item)
}

function press(button: Button, row: ItemRow) {
  if (button.kind === 'edit') emit('edit', row)
  else if (button.kind === 'remove') emit('remove', row)
  else emit('action', button.target, row)
}

function footnote(row: ItemRow): string {
  const parts: string[] = []
  if (props.members !== undefined) {
    parts.push(`Em ${row.entries.length} de ${props.members} equipamento(s)`)
  }
  if (row.detailTotal !== null && props.list.detail) {
    const label = outputLabel(props.list.detail, props.presentation?.labels)
    parts.push(`${row.detailTotal} ${label.toLowerCase()}`)
  }
  return parts.join(' · ')
}

function clearSearch() {
  term.value = ''
  emit('search', '')
}

// --- Tabela ---

function hasValues(key: string): boolean {
  return props.rows.some(
    (row) => row.item[key] !== '' && row.item[key] !== null && row.item[key] !== undefined
  )
}

/** As colunas declaradas (ou todas as chaves), sem as vazias nem as `_` da tela. */
const columns = computed(() => {
  const declared = props.list.columns ?? []
  if (declared.length > 0) {
    return declared
      .map((column) => column.key)
      .filter((key) => key === props.list.key || hasValues(key))
  }
  const keys = new Set<string>()
  for (const row of props.rows.slice(0, 50)) Object.keys(row.item).forEach((key) => keys.add(key))
  const rest = [...keys].filter(
    (key) => key !== props.list.key && !key.startsWith('_') && hasValues(key)
  )
  return [props.list.key, ...rest]
})

function columnLabel(key: string): string {
  const declared = props.list.columns?.find((column) => column.key === key)
  return declared?.label ?? props.presentation?.labels?.[key] ?? keyLabel(key)
}

const headers = computed(() =>
  columns.value.map((key) => ({
    title: columnLabel(key),
    key,
    sortable: true,
    value: (item: { row: ItemRow }) => item.row.item[key],
  }))
)
const tableItems = computed(() => props.rows.map((row) => ({ key: row.key, row })))

function isState(key: string, value: unknown): boolean {
  const declared = outputFormat(key, props.presentation) === 'state' || key === props.list.state
  return declared && typeof value === 'string' && value !== ''
}

function cell(key: string, value: unknown): string {
  if (value === null || value === undefined || value === '') return '—'
  if (typeof value === 'object') return JSON.stringify(value)
  const format = outputFormat(key, props.presentation)
  return format ? formatOutputValue(value, format) : String(value)
}

const selected = computed<ItemRow | null>(
  () => props.rows.find((row) => row.key === selectedKey.value) ?? null
)

function selectRow(_event: Event, { item }: { item: { key: string } }) {
  selectedKey.value = item.key
}

// Lista nova (busca, recarga): a seleção antiga não vale mais.
watch(
  () => props.rows,
  () => {
    if (!selected.value) selectedKey.value = null
  }
)
</script>

<style scoped>
.search {
  max-width: 320px;
  min-width: 200px;
}
.item-table :deep(tbody tr) {
  cursor: pointer;
}
.selection {
  border-color: rgb(var(--v-theme-primary)) !important;
}
</style>
