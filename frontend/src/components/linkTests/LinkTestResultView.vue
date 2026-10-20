<template>
  <section class="result" aria-label="Resultado do teste">
    <div class="headline">
      <v-icon :color="status.color" size="28">{{ status.icon }}</v-icon>
      <div>
        <div class="text-title-medium font-weight-bold">{{ headline }}</div>
        <div v-if="explanation" class="text-body-medium">{{ explanation }}</div>
      </div>
    </div>

    <div v-if="totals" class="totals">
      <div v-if="totals.forward !== null" class="total">
        <span class="total-label"
          ><v-icon size="16" color="success">mdi-arrow-down-bold</v-icon> Ida</span
        >
        <strong>{{ compactBps(totals.forward) }}</strong>
        <span class="total-sub">{{ forwardLabel }}</span>
      </div>
      <div v-if="totals.reverse !== null" class="total">
        <span class="total-label"
          ><v-icon size="16" color="info">mdi-arrow-up-bold</v-icon> Volta</span
        >
        <strong>{{ compactBps(totals.reverse) }}</strong>
        <span class="total-sub">{{ reverseLabel }}</span>
      </div>
    </div>

    <ul v-if="rows.length > 1" class="rows" aria-label="Resultado por enlace">
      <li v-for="row in rows" :key="row.seq" :class="{ bottleneck: row.bottleneck }">
        <v-icon v-if="row.bottleneck" size="16" color="warning">mdi-alert-decagram</v-icon>
        <v-icon v-else size="16" color="success">mdi-check-circle</v-icon>
        <span class="row-title">{{ row.title }}</span>
        <span class="row-value">{{ row.value }}</span>
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
      </li>
    </ul>

    <v-alert v-if="generalError" type="error" variant="tonal" density="compact">
      {{ generalError }}
    </v-alert>

    <ul v-if="missed.length > 0" class="missed" aria-label="O que não rodou">
      <li v-for="item in missed" :key="item.seq">
        <v-icon size="18" :color="item.failed ? 'error' : 'warning'">
          {{ item.failed ? 'mdi-close-circle' : 'mdi-debug-step-over' }}
        </v-icon>
        <div class="flex-grow-1">
          <div class="font-weight-bold">
            {{ item.title }} — {{ item.failed ? 'falhou' : 'não rodou' }}
          </div>
          <div class="text-body-small">{{ item.reason }}</div>
        </div>
      </li>
    </ul>

    <div v-if="perHop || repeat" class="next">
      <v-btn
        v-if="perHop"
        color="primary"
        variant="flat"
        prepend-icon="mdi-transit-connection-variant"
        :disabled="!auth.canWrite"
        @click="request(perHop)"
      >
        Medir enlace por enlace
      </v-btn>
      <v-btn
        v-if="repeat"
        color="primary"
        :variant="perHop ? 'outlined' : 'flat'"
        prepend-icon="mdi-repeat"
        :disabled="!auth.canWrite"
        @click="request(repeat)"
      >
        {{ missed.length > 0 ? 'Resolver e testar de novo' : 'Testar de novo' }}
      </v-btn>
      <span v-if="perHop" class="text-body-small next-hint">
        Mede cada enlace do caminho, um de cada vez, para achar o gargalo.
      </span>
    </div>
  </section>
</template>

<script setup lang="ts">
/**
 * O teste que terminou, resumo primeiro: o estado, os números grandes, o
 * gargalo numa frase, a linha de cada enlace (quando há mais de um) e o que
 * não rodou com o motivo. Os próximos passos ("Medir enlace por enlace",
 * "Testar de novo") abrem o assistente — nada roda sem o plano.
 */
import { computed } from 'vue'
import { useAuthStore } from '@/stores/auth'
import { useDevicesStore } from '@/stores/devices'
import { useLinkTestRequestsStore, type LinkTestRequest } from '@/stores/linkTestRequests'
import type { LinkTest } from '@/types/linkTests'
import { formatPercent } from '@/utils/formatters'
import {
  compactBps,
  endpointLabel,
  flowPairLabel,
  legTitle,
  testStatusPresentation,
} from '@/utils/linkTests/presentation'
import {
  bottleneckExplanation,
  bottleneckLeg,
  legCapacityBps,
  legSpeedBps,
  resultVsSpeed,
} from '@/utils/linkTests/results'
import { missedLegs, perHopRequest, repeatRequest } from '@/utils/linkTests/resultView'

const props = defineProps<{
  test: LinkTest
  speedOf: (edgeId: number) => number | null
}>()

const emit = defineEmits<{
  /** Um próximo passo foi pedido (quem mostra o resultado pode se fechar). */
  requested: []
}>()

const auth = useAuthStore()
const devices = useDevicesStore()
const requests = useLinkTestRequestsStore()

const status = computed(() => testStatusPresentation(props.test.status))

const headline = computed(() => {
  switch (props.test.status) {
    case 'succeeded':
      return 'Teste concluído'
    case 'partial':
      return 'Concluído em parte'
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

const ends = computed(() => {
  const legs = props.test.legs
  if (legs.length === 0) return null
  return { a: endpointLabel(legs[0].a), b: endpointLabel(legs[legs.length - 1].b) }
})
const forwardLabel = computed(() => (ends.value ? `${ends.value.a} → ${ends.value.b}` : ''))
const reverseLabel = computed(() => (ends.value ? `${ends.value.b} → ${ends.value.a}` : ''))

const explanation = computed(() => bottleneckExplanation(props.test, props.speedOf))
const worst = computed(() => bottleneckLeg(props.test))

const rows = computed(() =>
  props.test.legs
    .filter((leg) => legCapacityBps(leg) !== null)
    .map((leg) => {
      const speed = legSpeedBps(leg, props.speedOf)
      const capacity = legCapacityBps(leg)
      const comparison = capacity !== null && speed ? resultVsSpeed(capacity, speed) : null
      return {
        seq: leg.seq,
        title: legTitle(leg),
        value: flowPairLabel(leg.forwardBps, leg.reverseBps) ?? '—',
        comparison,
        percent:
          typeof comparison?.ratio === 'number' ? formatPercent(comparison.ratio * 100, 0) : '—',
        bottleneck: worst.value?.seq === leg.seq,
      }
    })
)

const missed = computed(() => missedLegs(props.test))
/** O erro do teste, só quando as linhas do que não rodou não o dizem já. */
const generalError = computed(() => {
  const error = props.test.error
  if (!error) return null
  return missed.value.some((item) => item.reason.includes(error)) ? null : error
})

const perHop = computed(() =>
  props.test.status === 'succeeded' || props.test.status === 'partial'
    ? perHopRequest(props.test, (id) =>
        Boolean(devices.devices.find((device) => device.id === id)?.ipAddress)
      )
    : null
)
const repeat = computed(() => repeatRequest(props.test))

function request(next: LinkTestRequest) {
  requests.requestLinkTest(next)
  emit('requested')
}
</script>

<style scoped>
.result {
  display: flex;
  flex-direction: column;
  gap: 16px;
  color: rgb(var(--v-theme-on-surface));
}
.headline {
  display: flex;
  align-items: flex-start;
  gap: 10px;
}
.totals {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
  gap: 12px;
}
.total {
  display: flex;
  flex-direction: column;
  padding: 12px 14px;
  border-radius: 12px;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.2);
  background: rgb(var(--v-theme-surface));
}
.total-label {
  display: flex;
  align-items: center;
  gap: 4px;
  font-weight: 700;
  font-size: 0.85rem;
}
.total strong {
  font-size: 2rem;
  font-weight: 800;
  line-height: 1.2;
  font-variant-numeric: tabular-nums;
}
.total-sub {
  font-size: 0.8125rem;
  word-break: break-word;
}
.rows,
.missed {
  list-style: none;
  padding: 0;
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.rows li {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
  border-radius: 10px;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.16);
}
.rows li.bottleneck {
  border-color: rgb(var(--v-theme-warning));
  background: rgba(var(--v-theme-warning), 0.12);
}
.row-title {
  flex: 1;
  min-width: 0;
  font-weight: 600;
  word-break: break-word;
}
.row-value {
  font-variant-numeric: tabular-nums;
  font-weight: 700;
  white-space: nowrap;
}
.missed li {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 10px;
  border-radius: 10px;
  border: 1px solid rgb(var(--v-theme-warning));
}
.next {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
}
.next-hint {
  flex-basis: 100%;
}
</style>
