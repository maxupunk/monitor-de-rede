<template>
  <div class="quick-ping-result">
    <v-alert
      :type="verdictType"
      variant="tonal"
      density="comfortable"
      class="mb-4"
      :text="report.verdict"
    ></v-alert>

    <div v-if="report.samples > 0" class="qp-grid mb-4">
      <section class="qp-tile">
        <div class="qp-label">Latência (pacote pequeno)</div>
        <div v-if="report.small" class="qp-value">{{ formatLatency(report.small.minMs) }}</div>
        <div v-if="report.small" class="qp-detail">
          p50 {{ formatLatency(report.small.p50Ms) }} · p95 {{ formatLatency(report.small.p95Ms) }}
        </div>
        <div v-else class="qp-detail">Sem resposta</div>
      </section>

      <section class="qp-tile">
        <div class="qp-label">Latência (pacote de {{ largeBytes }} B)</div>
        <div v-if="report.large" class="qp-value">{{ formatLatency(report.large.minMs) }}</div>
        <div v-if="report.large" class="qp-detail">
          p50 {{ formatLatency(report.large.p50Ms) }} · p95 {{ formatLatency(report.large.p95Ms) }}
        </div>
        <div v-else class="qp-detail">Sem resposta aos pacotes grandes</div>
      </section>

      <section class="qp-tile">
        <div class="qp-label">Jitter</div>
        <div class="qp-value">{{ formatLatency(report.jitterMs, '—') }}</div>
        <div class="qp-detail">Variação entre respostas seguidas</div>
      </section>

      <section class="qp-tile">
        <div class="qp-label">Perda</div>
        <div class="qp-value" :class="`text-${lossTone}`">{{ lossLabel(report.lossPct) }}</div>
        <div class="qp-detail">{{ report.samples }} de {{ report.sent }} respostas</div>
      </section>
    </div>

    <div v-if="queueing" class="d-flex flex-wrap align-center ga-2 mb-4">
      <v-chip
        :color="queueing.color"
        variant="flat"
        size="small"
        label
        :prepend-icon="queueing.icon"
      >
        {{ queueing.label }}
      </v-chip>
      <span v-if="report.queueing" class="qp-detail">
        p95 {{ formatLatency(report.queueing.inflationMs) }} acima do mínimo
      </span>
    </div>

    <section class="qp-capacity" :class="`qp-capacity--${outcome}`">
      <div class="d-flex align-center ga-2 mb-1">
        <v-icon :color="outcome === 'range' ? 'info' : 'warning'" size="20">
          {{ outcome === 'range' ? 'mdi-speedometer-medium' : 'mdi-speedometer-slow' }}
        </v-icon>
        <span class="qp-label">Capacidade estimada do caminho</span>
      </div>
      <template v-if="report.capacity">
        <div class="d-flex flex-wrap align-center ga-2">
          <span class="qp-value" data-test="capacity-range">
            {{ capacityRangeLabel(report.capacity) }}
          </span>
          <v-chip color="warning" variant="tonal" size="small" label>
            {{ confidenceLabel(report.capacity.confidence) }}
          </v-chip>
        </div>
      </template>
      <div v-else class="qp-value qp-value--note" data-test="capacity-none">Não estimada</div>
      <p class="qp-note mt-1 mb-0">{{ report.capacityNote }}</p>
    </section>
  </div>
</template>

<script setup lang="ts">
/**
 * Resultado do diagnóstico rápido por ping (apresentação pura). O veredito,
 * o nível de fila e a decisão de dar ou não um número de capacidade vêm do
 * backend; aqui só se mostra.
 */
import { computed } from 'vue'
import type { QuickPingReport } from '@/bindings/QuickPingReport'
import { formatLatency } from '@/utils/formatters'
import {
  capacityOutcome,
  capacityRangeLabel,
  confidenceLabel,
  lossColor,
  lossLabel,
  queueingBadge,
} from '@/utils/diagnostics/quickPing'

const props = defineProps<{ report: QuickPingReport }>()

const outcome = computed(() => capacityOutcome(props.report))
const queueing = computed(() =>
  props.report.queueing ? queueingBadge(props.report.queueing.level) : null
)
const lossTone = computed(() => lossColor(props.report.lossPct))
const largeBytes = computed(() => props.report.large?.bytes ?? 1472)

const verdictType = computed<'success' | 'warning' | 'error'>(() => {
  const level = props.report.queueing?.level
  if (!level) return 'error'
  if (level === 'ok') return 'success'
  return level === 'moderado' ? 'warning' : 'error'
})
</script>

<style scoped>
.qp-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
  gap: 12px;
}

.qp-tile,
.qp-capacity {
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgba(var(--v-border-color), 0.24);
  border-radius: 12px;
  padding: 12px 14px;
}

.qp-capacity--range {
  border-color: rgb(var(--v-theme-info));
}

.qp-capacity--not_estimated,
.qp-capacity--no_reply {
  border-color: rgb(var(--v-theme-warning));
}

.qp-label {
  font-size: 0.8rem;
  font-weight: 600;
  color: rgb(var(--v-theme-on-surface));
}

.qp-value {
  font-size: 1.35rem;
  font-weight: 700;
  line-height: 1.3;
  color: rgb(var(--v-theme-on-surface));
}

.qp-value--note {
  font-size: 1.05rem;
}

.qp-detail,
.qp-note {
  font-size: 0.82rem;
  color: rgb(var(--v-theme-on-surface));
}
</style>
