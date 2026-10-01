<template>
  <div class="d-flex flex-column ga-3">
    <template v-for="section in sections" :key="section.key">
      <div v-if="section.kind === 'kv'" class="report-kv">
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
            <div class="text-caption">{{ outputLabel(key, labels) }}</div>
            <v-chip
              v-if="isState(key, value)"
              :color="stateColor(value)"
              size="small"
              variant="flat"
              class="font-weight-bold"
            >
              {{ stateLabel(value) }}
            </v-chip>
            <div v-else class="text-body-1 font-weight-bold">{{ display(value) }}</div>
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
                {{ outputLabel(column, labels) }}
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
                  :labels="labels"
                  class="py-2"
                />
                <span v-else>{{ display(row[column]) }}</span>
              </td>
            </tr>
          </tbody>
        </v-table>
      </div>

      <div v-else-if="section.kind === 'lines'">
        <div class="text-subtitle-2 font-weight-bold mb-1">{{ section.title }}</div>
        <pre class="report-lines pa-3 rounded-lg font-mono text-body-small">{{
          section.lines.join('\n')
        }}</pre>
      </div>

      <v-alert
        v-else
        type="info"
        variant="tonal"
        density="compact"
        :title="section.title || undefined"
        :text="section.text"
      ></v-alert>
    </template>
    <div v-if="sections.length === 0" class="text-body-2">(sem resultado)</div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { outputLabel, stateColor, stateLabel, type OutputLabels } from '@/utils/pluginPresentation'

defineOptions({ name: 'PluginReport' })

const props = withDefaults(
  defineProps<{ output: unknown; depth?: number; labels?: OutputLabels }>(),
  { depth: 0, labels: undefined }
)

type Row = Record<string, unknown>
type Section =
  | { kind: 'kv'; key: string; title: string; entries: [string, unknown][] }
  | { kind: 'table'; key: string; title: string; rows: Row[]; columns: string[] }
  | { kind: 'lines'; key: string; title: string; lines: string[] }
  | { kind: 'text'; key: string; title: string; text: string }

/** Chaves cujo valor é um estado (vira chip colorido). */
const STATE_KEYS = new Set(['state', 'status', 'change', 'estado'])
/** Chaves que a tela trata à parte (ex.: a sugestão de configuração do `reduce`). */
const HIDDEN_KEYS = new Set(['settings_patch'])

function isRow(value: unknown): value is Row {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function isRowList(value: unknown): value is Row[] {
  return Array.isArray(value) && value.length > 0 && value.every(isRow)
}

function isState(key: string, value: unknown): boolean {
  return STATE_KEYS.has(key) && typeof value === 'string' && value !== ''
}

function display(value: unknown): string {
  if (value === null || value === undefined || value === '') return '—'
  if (typeof value === 'boolean') return value ? 'Sim' : 'Não'
  if (Array.isArray(value)) {
    if (value.length === 0) return '—'
    return value.every((item) => !isRow(item)) ? value.join(', ') : `${value.length} itens`
  }
  if (isRow(value)) return JSON.stringify(value)
  return String(value)
}

function columnsOf(rows: Row[]): string[] {
  const keys = new Set<string>()
  for (const row of rows) for (const key of Object.keys(row)) keys.add(key)
  return [...keys]
}

const SECTION_RANK: Record<Section['kind'], number> = { kv: 0, table: 1, text: 2, lines: 3 }

const sections = computed<Section[]>(() => {
  const value = props.output
  if (typeof value === 'string') return [{ kind: 'text', key: 'text', title: '', text: value }]
  if (isRowList(value)) {
    return [{ kind: 'table', key: 'rows', title: '', rows: value, columns: columnsOf(value) }]
  }
  if (!isRow(value)) return []

  const result: Section[] = []
  const scalars: [string, unknown][] = []
  for (const [key, item] of Object.entries(value)) {
    if (HIDDEN_KEYS.has(key)) continue
    if (isRowList(item)) {
      result.push({
        kind: 'table',
        key,
        title: outputLabel(key, props.labels),
        rows: item,
        columns: columnsOf(item),
      })
    } else if (Array.isArray(item) && item.length > 0) {
      result.push({
        kind: 'lines',
        key,
        title: outputLabel(key, props.labels),
        lines: item.map(String),
      })
    } else if (Array.isArray(item)) {
      continue
    } else if (isRow(item)) {
      result.push({
        kind: 'kv',
        key,
        title: outputLabel(key, props.labels),
        entries: Object.entries(item),
      })
    } else if (typeof item === 'string' && item.length > 80) {
      result.push({ kind: 'text', key, title: outputLabel(key, props.labels), text: item })
    } else {
      scalars.push([key, item])
    }
  }
  if (scalars.length > 0) result.unshift({ kind: 'kv', key: '', title: '', entries: scalars })
  // O JSON chega com as chaves em ordem alfabética: o resumo vem primeiro,
  // depois as tabelas e os textos, e os comandos por último.
  return result.sort((a, b) => SECTION_RANK[a.kind] - SECTION_RANK[b.kind])
})
</script>

<style scoped>
.report-lines {
  background-color: rgb(var(--v-theme-surface));
  border: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 280px;
  overflow: auto;
}
</style>
