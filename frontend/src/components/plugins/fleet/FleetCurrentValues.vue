<template>
  <v-card border flat class="rounded-lg mb-3">
    <v-card-title class="d-flex align-center ga-2 text-subtitle-1 font-weight-bold">
      <v-icon color="primary">mdi-clipboard-text-outline</v-icon>
      Como está agora
    </v-card-title>
    <v-card-subtitle v-if="readAt">Lido {{ formatRelativeTime(readAt) }}</v-card-subtitle>
    <v-card-text>
      <v-table density="compact" class="current-values">
        <thead>
          <tr>
            <th class="font-weight-bold">Equipamento</th>
            <th v-for="field in columns" :key="field.name" class="font-weight-bold">
              {{ field.label }}
            </th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="member in members" :key="member.deviceId">
            <td class="font-weight-bold text-no-wrap">{{ member.name }}</td>
            <td v-for="field in columns" :key="field.name" class="text-no-wrap">
              {{ display(current.get(member.deviceId)?.[field.name]) }}
            </td>
          </tr>
        </tbody>
      </v-table>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { FleetMember } from '@/bindings/FleetMember'
import { formatRelativeTime } from '@/utils/formatters'
import { fieldsOf, type Schema } from '@/utils/pluginSettings'

/** Os valores atuais (`current`) de cada equipamento, nas colunas do formulário. */
const props = defineProps<{
  members: FleetMember[]
  current: Map<number, Record<string, unknown>>
  schema: Schema
  readAt?: string | null
}>()

/** Só as colunas que algum equipamento tem. */
const columns = computed(() =>
  fieldsOf(props.schema).filter((field) =>
    [...props.current.values()].some((values) => values[field.name] !== undefined)
  )
)

function display(value: unknown): string {
  if (value === undefined || value === null || value === '') return '—'
  if (typeof value === 'boolean') return value ? 'Sim' : 'Não'
  return String(value)
}
</script>

<style scoped>
.current-values {
  overflow-x: auto;
}
</style>
