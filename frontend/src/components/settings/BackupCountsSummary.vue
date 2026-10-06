<template>
  <div>
    <div class="text-caption text-high-emphasis mb-1">
      <strong>{{ name }}</strong> — {{ counts.totalRows }} registros
    </div>
    <v-chip
      v-for="row in counts.tables"
      :key="row.table"
      size="x-small"
      variant="tonal"
      color="info"
      class="mr-1 mb-1"
    >
      {{ tableLabel(row.table) }}: {{ row.rows }}
    </v-chip>
  </div>
</template>

<script setup lang="ts">
import type { BackupCountsResponse } from '@/bindings/BackupCountsResponse'
import { tableLabel } from '@/stores/backup'

/** O que um arquivo de backup contém, tabela a tabela — a prévia da restauração. */
defineProps<{
  name: string
  counts: BackupCountsResponse
}>()
</script>
