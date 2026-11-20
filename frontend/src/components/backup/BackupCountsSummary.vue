<template>
  <div>
    <div class="d-flex align-center flex-wrap ga-2 text-caption text-high-emphasis mb-1">
      <span>
        <strong>{{ name }}</strong> — {{ formatInteger(counts.totalRows) }} registros
      </span>
      <v-chip
        size="x-small"
        :color="counts.scope === 'full' ? 'warning' : 'primary'"
        variant="flat"
        data-scope
      >
        {{ counts.scope === 'full' ? 'Backup completo' : 'Configuração' }}
      </v-chip>
    </div>
    <v-chip
      v-for="row in counts.tables"
      :key="row.table"
      size="x-small"
      variant="tonal"
      color="info"
      class="mr-1 mb-1"
    >
      {{ row.label }}: {{ formatInteger(row.rows) }}
    </v-chip>
  </div>
</template>

<script setup lang="ts">
import type { BackupCountsResponse } from '@/bindings/BackupCountsResponse'
import { formatInteger } from '@/utils/formatters'

/** O que um arquivo de backup contém, tabela a tabela — a prévia da restauração. */
defineProps<{
  name: string
  counts: BackupCountsResponse
}>()
</script>
