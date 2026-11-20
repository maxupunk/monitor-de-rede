<template>
  <div class="border rounded-lg">
    <div
      v-for="(row, index) in rows"
      :key="row.table"
      class="px-3 py-2"
      :class="{ 'border-t': index > 0 }"
      :data-table="row.table"
    >
      <div class="d-flex align-center flex-wrap ga-2">
        <div class="flex-grow-1 min-w-0">
          <div class="font-weight-bold text-body-2">{{ row.label }}</div>
          <div class="text-caption text-high-emphasis">
            <code>{{ row.table }}</code>
            · {{ rowsLabel(row) }}
            <template v-if="row.dataBytes != null && row.indexBytes != null">
              · dados {{ formatBinaryBytes(row.dataBytes) }} · índices
              {{ formatBinaryBytes(row.indexBytes) }}
            </template>
          </div>
        </div>
        <v-chip v-if="showArea" size="x-small" :color="AREA_COLORS[row.area]" variant="tonal">
          {{ row.areaTitle }}
        </v-chip>
        <v-chip v-if="row.retentionDays != null" size="x-small" color="success" variant="tonal">
          guarda {{ countNoun(row.retentionDays, 'dia') }}
        </v-chip>
        <div class="text-right usage-size">
          <div class="font-weight-bold text-body-2">
            {{ row.totalBytes != null ? formatBinaryBytes(row.totalBytes) : '—' }}
          </div>
          <div class="text-caption">{{ formatPercent(row.share) }}</div>
        </div>
      </div>
      <v-progress-linear
        :model-value="row.share ?? 0"
        :color="AREA_COLORS[row.area]"
        height="4"
        rounded
        class="mt-1"
      ></v-progress-linear>
    </div>
  </div>
</template>

<script setup lang="ts">
import { AREA_COLORS, type UsageRow } from '@/utils/databaseUsage'
import { countNoun, formatBinaryBytes, formatInteger, formatPercent } from '@/utils/formatters'

defineProps<{
  rows: UsageRow[]
  /** Mostra a área em cada linha (fora do agrupamento por área). */
  showArea: boolean
}>()

function rowsLabel(row: UsageRow): string {
  const count = `${formatInteger(row.rows)} ${row.rows === 1 ? 'linha' : 'linhas'}`
  return row.rowsEstimated ? `≈ ${count}` : count
}
</script>

<style scoped>
.usage-size {
  min-width: 84px;
}
</style>
