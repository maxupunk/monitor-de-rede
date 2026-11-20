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
      v-for="row in filled"
      :key="row.table"
      size="x-small"
      variant="tonal"
      color="info"
      class="mr-1 mb-1"
      data-table
    >
      {{ row.label }}: {{ formatInteger(row.rows) }}
    </v-chip>
    <div v-if="emptyCount > 0" class="text-caption text-high-emphasis mt-1" data-empty>
      {{ filled.length ? 'e ' : '' }}{{ emptyCount }}
      {{ emptyCount === 1 ? 'tabela vazia' : 'tabelas vazias' }}
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { BackupCountsResponse } from '@/bindings/BackupCountsResponse'
import { formatInteger } from '@/utils/formatters'

/**
 * O que um arquivo de backup contém — a prévia da restauração. Só as tabelas com
 * registros viram chip: no backup completo são dezenas, e as vazias esconderiam
 * o que importa (e o botão de confirmar).
 */
const props = defineProps<{
  name: string
  counts: BackupCountsResponse
}>()

const filled = computed(() => props.counts.tables.filter((row) => row.rows > 0))
const emptyCount = computed(() => props.counts.tables.length - filled.value.length)
</script>
