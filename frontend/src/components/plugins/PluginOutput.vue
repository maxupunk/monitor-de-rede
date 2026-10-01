<template>
  <div class="plugin-output">
    <PluginReport v-if="kind === 'report'" :output="output" :labels="labels" />
    <v-table v-else-if="rows" density="compact" class="border rounded-lg">
      <thead>
        <tr>
          <th v-for="column in columns" :key="column" class="font-weight-bold">
            {{ outputLabel(column, labels) }}
          </th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="(row, index) in rows" :key="index">
          <td v-for="column in columns" :key="column" class="font-mono text-body-small">
            {{ cell(row[column]) }}
          </td>
        </tr>
      </tbody>
    </v-table>

    <v-list v-else-if="entries" density="compact" border class="rounded-lg">
      <v-list-item
        v-for="[key, value] in entries"
        :key="key"
        :title="cell(value)"
        :subtitle="outputLabel(key, labels)"
      ></v-list-item>
    </v-list>

    <pre v-else class="output-text pa-3 rounded-lg font-mono text-body-small">{{ text }}</pre>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { OutputKind } from '@/bindings/OutputKind'
import { outputLabel, type OutputLabels } from '@/utils/pluginPresentation'
import PluginReport from './PluginReport.vue'

const props = defineProps<{
  output: unknown
  kind?: OutputKind
  /** Títulos das chaves declarados pela ação (`labels`). */
  labels?: OutputLabels
}>()

type Row = Record<string, unknown>

function isRow(value: unknown): value is Row {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

const rows = computed<Row[] | null>(() => {
  const value = props.output
  if (!Array.isArray(value) || value.length === 0 || !value.every(isRow)) return null
  if (props.kind && props.kind !== 'table' && props.kind !== 'json') return null
  return value
})

const columns = computed(() => {
  const keys = new Set<string>()
  for (const row of rows.value ?? []) for (const key of Object.keys(row)) keys.add(key)
  return [...keys]
})

const entries = computed<[string, unknown][] | null>(() => {
  const value = props.output
  if (!isRow(value) || props.kind === 'json') return null
  return Object.entries(value)
})

const text = computed(() => {
  const value = props.output
  if (typeof value === 'string') return value
  if (value === null || value === undefined) return '(sem resultado)'
  return JSON.stringify(value, null, 2)
})

function cell(value: unknown): string {
  if (value === null || value === undefined || value === '') return '—'
  if (typeof value === 'object') return JSON.stringify(value)
  return String(value)
}
</script>

<style scoped>
.output-text {
  background-color: rgb(var(--v-theme-surface));
  border: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 360px;
  overflow: auto;
}
</style>
