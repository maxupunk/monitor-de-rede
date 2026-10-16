<template>
  <section class="summary" aria-label="Resultado do teste">
    <div class="d-flex align-center flex-wrap ga-2 mb-2">
      <v-icon :color="status.color">{{ status.icon }}</v-icon>
      <span class="text-title-small font-weight-bold">{{ headline }}</span>
    </div>
    <div v-if="totals" class="totals mb-2">
      <div v-if="totals.forward !== null" class="total">
        <span class="text-label-small">↓ Ida (pior enlace)</span>
        <strong>{{ compactBps(totals.forward) }}</strong>
      </div>
      <div v-if="totals.reverse !== null" class="total">
        <span class="text-label-small">↑ Volta (pior enlace)</span>
        <strong>{{ compactBps(totals.reverse) }}</strong>
      </div>
    </div>
    <p v-if="explanation" class="explanation text-body-medium mb-3">{{ explanation }}</p>

    <div v-if="rows.length > 0" class="table-wrap">
      <table class="results">
        <thead>
          <tr>
            <th scope="col">Perna</th>
            <th scope="col">↓ Ida</th>
            <th scope="col">↑ Volta</th>
            <th scope="col" title="Pior sentido sobre a velocidade da porta">% porta</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="row in rows" :key="row.seq" :class="{ bottleneck: row.bottleneck }">
            <th scope="row">
              <span class="d-inline-flex align-center ga-1">
                <v-icon v-if="row.bottleneck" size="14" color="warning">mdi-alert-decagram</v-icon>
                {{ row.title }}
              </span>
            </th>
            <td class="num">{{ row.forward }}</td>
            <td class="num">{{ row.reverse }}</td>
            <td>
              <v-chip
                v-if="row.comparison"
                :color="row.comparison.color"
                size="x-small"
                variant="flat"
                label
                :title="row.comparison.text"
              >
                {{ row.percent }}
              </v-chip>
              <span v-else>—</span>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </section>
</template>

<script setup lang="ts">
/** Resumo do teste concluído: gargalo explicado e tabela por perna. */
import { computed } from 'vue'
import type { LinkTest } from '@/types/linkTests'
import { formatPercent } from '@/utils/formatters'
import { compactBps, legTitle, testStatusPresentation } from '@/utils/linkTests/presentation'
import {
  bottleneckExplanation,
  bottleneckLeg,
  legCapacityBps,
  resultVsSpeed,
} from '@/utils/linkTests/results'

const props = defineProps<{
  test: LinkTest
  speedOf: (edgeId: number) => number | null
}>()

const status = computed(() => testStatusPresentation(props.test.status))

const headline = computed(() => {
  switch (props.test.status) {
    case 'succeeded':
      return 'Teste concluído'
    case 'partial':
      return 'Teste concluído em parte — algumas pernas não rodaram'
    case 'failed':
      return 'O teste falhou'
    case 'cancelled':
      return 'Teste cancelado'
    default:
      return status.value.label
  }
})

const totals = computed(() => {
  const summary = props.test.summary
  if (!summary) return null
  if (summary.minForwardBps === null && summary.minReverseBps === null) return null
  return { forward: summary.minForwardBps, reverse: summary.minReverseBps }
})

const explanation = computed(() => bottleneckExplanation(props.test, props.speedOf))
const worst = computed(() => bottleneckLeg(props.test))

const rows = computed(() =>
  props.test.legs
    .filter((leg) => legCapacityBps(leg) !== null || leg.status !== 'pending')
    .map((leg) => {
      const speed = leg.edgeId !== null ? props.speedOf(leg.edgeId) : null
      const capacity = legCapacityBps(leg)
      const comparison = capacity !== null && speed ? resultVsSpeed(capacity, speed) : null
      return {
        seq: leg.seq,
        title: legTitle(leg),
        forward: compactBps(leg.forwardBps),
        reverse: compactBps(leg.reverseBps),
        comparison,
        percent:
          typeof comparison?.ratio === 'number' ? formatPercent(comparison.ratio * 100, 0) : '—',
        bottleneck: worst.value?.seq === leg.seq && props.test.legs.length > 1,
      }
    })
)
</script>

<style scoped>
.summary {
  border: 1px solid rgba(var(--v-theme-on-surface), 0.22);
  border-radius: 12px;
  padding: 12px;
  background: rgb(var(--v-theme-surface));
  color: rgb(var(--v-theme-on-surface));
}
.totals {
  display: flex;
  flex-wrap: wrap;
  gap: 16px;
}
.total {
  display: flex;
  flex-direction: column;
}
.total strong {
  font-size: 1.5rem;
  font-weight: 800;
  font-variant-numeric: tabular-nums;
}
.explanation {
  margin: 0;
}
.table-wrap {
  overflow-x: auto;
}
.results {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.8125rem;
}
.results th,
.results td {
  text-align: left;
  padding: 6px;
  border-bottom: 1px solid rgba(var(--v-theme-on-surface), 0.16);
  white-space: nowrap;
}
.results thead th {
  font-weight: 700;
}
.results tbody th {
  font-weight: 600;
  white-space: normal;
  min-width: 120px;
}
.results .num {
  font-variant-numeric: tabular-nums;
}
.results tr.bottleneck {
  background: rgba(var(--v-theme-warning), 0.16);
}
</style>
