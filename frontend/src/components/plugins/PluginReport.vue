<template>
  <div class="d-flex flex-column ga-3">
    <template v-for="section in sections" :key="section.key">
      <div v-if="section.kind === 'stats'" class="d-flex flex-wrap ga-2">
        <v-card
          v-for="stat in section.stats"
          :key="stat.label"
          border
          flat
          class="rounded-lg px-4 py-2 stat"
        >
          <div class="text-caption font-weight-bold text-primary">{{ stat.label }}</div>
          <div class="text-body-1 font-weight-bold">{{ stat.value }}</div>
        </v-card>
      </div>

      <div v-else-if="section.kind === 'kv'" class="report-kv">
        <div v-if="section.title" class="text-subtitle-2 font-weight-bold mb-1">
          {{ section.title }}
        </div>
        <div class="d-flex flex-wrap ga-2">
          <v-card
            v-for="[key, value] in section.entries"
            :key="key"
            border
            flat
            class="rounded-lg px-3 py-2"
          >
            <div class="text-caption">{{ outputLabel(key, presentation?.labels) }}</div>
            <v-chip
              v-if="isState(key, value)"
              :color="stateColor(value)"
              size="small"
              variant="flat"
              class="font-weight-bold"
            >
              {{ stateLabel(value) }}
            </v-chip>
            <div v-else class="text-body-1 font-weight-bold">{{ display(key, value) }}</div>
          </v-card>
        </div>
      </div>

      <div v-else-if="section.kind === 'table'">
        <div class="text-subtitle-2 font-weight-bold mb-1">
          {{ section.title }}
          <v-chip size="x-small" color="primary" variant="tonal" class="ml-1">
            {{ section.rows.length }}
          </v-chip>
        </div>
        <v-table density="compact" class="border rounded-lg">
          <thead>
            <tr>
              <th v-for="column in section.columns" :key="column" class="font-weight-bold">
                {{ outputLabel(column, presentation?.labels) }}
              </th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(row, index) in section.rows" :key="index">
              <td v-for="column in section.columns" :key="column" class="text-body-small">
                <v-chip
                  v-if="isState(column, row[column])"
                  :color="stateColor(row[column])"
                  size="x-small"
                  variant="flat"
                >
                  {{ stateLabel(row[column]) }}
                </v-chip>
                <PluginReport
                  v-else-if="isRowList(row[column]) && depth < 1"
                  :output="{ [column]: row[column] }"
                  :depth="depth + 1"
                  :presentation="presentation"
                  class="py-2"
                />
                <span v-else>{{ display(column, row[column]) }}</span>
              </td>
            </tr>
          </tbody>
        </v-table>
      </div>

      <v-expansion-panels
        v-else-if="section.kind === 'lines'"
        :model-value="linesOpen ? [section.key] : []"
        variant="accordion"
        flat
        class="border rounded-lg"
      >
        <v-expansion-panel :value="section.key">
          <v-expansion-panel-title class="text-subtitle-2 font-weight-bold">
            {{ section.title }}
            <v-chip size="x-small" color="primary" variant="tonal" class="ml-2">
              {{ section.lines.length }}
            </v-chip>
          </v-expansion-panel-title>
          <v-expansion-panel-text>
            <pre class="report-lines pa-3 rounded-lg font-mono text-body-small">{{
              section.lines.join('\n')
            }}</pre>
          </v-expansion-panel-text>
        </v-expansion-panel>
      </v-expansion-panels>

      <v-alert
        v-else
        type="info"
        variant="tonal"
        density="compact"
        :title="section.title || undefined"
        :text="section.text"
      ></v-alert>
    </template>
    <div v-if="sections.length === 0" class="text-body-2">Nada a mostrar deste equipamento.</div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import {
  formatOutputValue,
  outputFormat,
  orderedKeys,
  outputLabel,
  stateColor,
  stateLabel,
  type OutputPresentation,
} from '@/utils/pluginPresentation'

defineOptions({ name: 'PluginReport' })

const props = withDefaults(
  defineProps<{
    output: unknown
    depth?: number
    /** Títulos e formatos das chaves — a própria ação serve. */
    presentation?: OutputPresentation
  }>(),
  { depth: 0, presentation: undefined }
)

type Row = Record<string, unknown>
type Stat = { label: string; value: string }
type Section =
  | { kind: 'stats'; key: string; stats: Stat[] }
  | { kind: 'kv'; key: string; title: string; entries: [string, unknown][] }
  | { kind: 'table'; key: string; title: string; rows: Row[]; columns: string[] }
  | { kind: 'lines'; key: string; title: string; lines: string[] }
  | { kind: 'text'; key: string; title: string; text: string }

/** Chaves cujo valor é um estado (vira chip colorido). */
const STATE_KEYS = new Set(['state', 'status', 'change', 'estado'])
/** Chaves que a tela trata à parte (ex.: a sugestão de configuração do `reduce`). */
const HIDDEN_KEYS = new Set(['settings_patch', 'next'])

function isRow(value: unknown): value is Row {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function isRowList(value: unknown): value is Row[] {
  return Array.isArray(value) && value.length > 0 && value.every(isRow)
}

function isState(key: string, value: unknown): boolean {
  const declared = outputFormat(key, props.presentation) === 'state'
  return (declared || STATE_KEYS.has(key)) && typeof value === 'string' && value !== ''
}

function display(key: string, value: unknown): string {
  if (value === null || value === undefined || value === '') return '—'
  const format = outputFormat(key, props.presentation)
  if (format && typeof value !== 'object') return formatOutputValue(value, format)
  return plain(value)
}

function plain(value: unknown): string {
  if (typeof value === 'boolean') return value ? 'Sim' : 'Não'
  if (Array.isArray(value)) {
    if (value.length === 0) return '—'
    return value.every((item) => !isRow(item)) ? value.join(', ') : `${value.length} itens`
  }
  if (isRow(value)) return JSON.stringify(value)
  return String(value)
}

/** `_chave` é dado para a tela (ex.: a seção UCI de uma mudança), não coluna. */
function isVisibleKey(key: string): boolean {
  return !HIDDEN_KEYS.has(key) && !key.startsWith('_')
}

function columnsOf(rows: Row[]): string[] {
  const keys = new Set<string>()
  for (const row of rows) for (const key of Object.keys(row)) if (isVisibleKey(key)) keys.add(key)
  return orderedKeys([...keys], props.presentation)
}

const SECTION_RANK: Record<Section['kind'], number> = {
  stats: -1,
  kv: 0,
  table: 1,
  text: 2,
  lines: 3,
}

/**
 * `_card`: as linhas de resumo que o plugin dá para o cartão do equipamento
 * (`[#{ label, value }]`) — aqui, cartões no topo do relatório.
 */
function statsOf(value: unknown): Stat[] {
  if (!Array.isArray(value)) return []
  return value
    .filter(isRow)
    .filter((row) => typeof row.label === 'string')
    .map((row) => ({ label: String(row.label), value: plain(row.value ?? '—') }))
}

const sections = computed<Section[]>(() => {
  const value = props.output
  if (typeof value === 'string') return [{ kind: 'text', key: 'text', title: '', text: value }]
  if (isRowList(value)) {
    return [{ kind: 'table', key: 'rows', title: '', rows: value, columns: columnsOf(value) }]
  }
  if (!isRow(value)) return []

  const result: Section[] = []
  const stats = props.depth === 0 ? statsOf(value._card) : []
  if (stats.length > 0) result.push({ kind: 'stats', key: '_card', stats })
  const scalars: [string, unknown][] = []
  for (const key of orderedKeys(Object.keys(value), props.presentation)) {
    const item = value[key]
    if (!isVisibleKey(key)) continue
    if (isRowList(item)) {
      result.push({
        kind: 'table',
        key,
        title: outputLabel(key, props.presentation?.labels),
        rows: item,
        columns: columnsOf(item),
      })
    } else if (Array.isArray(item) && item.length > 0) {
      result.push({
        kind: 'lines',
        key,
        title: outputLabel(key, props.presentation?.labels),
        lines: item.map(String),
      })
    } else if (Array.isArray(item)) {
      continue
    } else if (isRow(item)) {
      result.push({
        kind: 'kv',
        key,
        title: outputLabel(key, props.presentation?.labels),
        entries: orderedKeys(Object.keys(item), props.presentation).map((name) => [
          name,
          item[name],
        ]),
      })
    } else if (typeof item === 'string' && item.length > 80) {
      result.push({
        kind: 'text',
        key,
        title: outputLabel(key, props.presentation?.labels),
        text: item,
      })
    } else {
      scalars.push([key, item])
    }
  }
  if (scalars.length > 0) result.unshift({ kind: 'kv', key: '', title: '', entries: scalars })
  // O resumo vem primeiro, depois as tabelas e os textos, e os comandos por
  // último; dentro de cada tipo, a ordem que a ação declara (`order`).
  return result.sort((a, b) => SECTION_RANK[a.kind] - SECTION_RANK[b.kind])
})

/**
 * Listas de linhas (comandos, log) são o detalhe técnico: recolhidas quando há
 * um resumo ou uma tabela para ler antes; abertas quando são o resultado todo.
 */
const linesOpen = computed(() => sections.value.every((section) => section.kind === 'lines'))
</script>

<style scoped>
.stat {
  min-width: 140px;
}
.report-lines {
  background-color: rgb(var(--v-theme-surface));
  border: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 280px;
  overflow: auto;
}
</style>
