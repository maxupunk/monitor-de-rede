<template>
  <v-card elevation="2" class="rounded-lg pa-3 pa-md-5 mb-3 mb-md-6">
    <!-- Cabeçalho do Card -->
    <div class="d-flex align-center justify-space-between mb-3 mb-md-4 flex-wrap ga-3">
      <div>
        <h2 class="text-h6 font-weight-bold d-flex align-center ga-2">
          <v-icon color="primary">mdi-chart-bell-curve-cumulative</v-icon>
          Linha de Base Estatística & Anomalias
        </h2>
        <div class="text-subtitle-2 text-grey">
          Média móvel (μ), desvio padrão (σ) e detecção de anomalias estatísticas (Z-Score de 7
          dias)
        </div>
      </div>

      <div class="d-flex align-center ga-2 flex-wrap">
        <!-- Status Badge -->
        <v-chip
          v-if="!hasData"
          color="grey"
          size="small"
          variant="tonal"
          prepend-icon="mdi-clock-outline"
        >
          Coletando Amostras (&lt; 6h de histórico)
        </v-chip>
        <v-chip
          v-else-if="isAnomaly"
          color="error"
          size="small"
          variant="flat"
          prepend-icon="mdi-alert-decagram"
          class="anomaly-pulse-chip"
        >
          Anomalia Estatística Detectada (Z &gt; 3.0σ)
        </v-chip>
        <v-chip v-else color="success" size="small" variant="tonal" prepend-icon="mdi-check-circle">
          Comportamento Normal (Dentro de 3σ)
        </v-chip>

        <v-btn
          v-if="hasData"
          size="small"
          variant="outlined"
          color="primary"
          prepend-icon="mdi-bell-plus"
          @click="$emit('createAnomalyRule')"
        >
          Criar Regra de Anomalia
        </v-btn>
      </div>
    </div>

    <!-- Conteúdo com Dados Suficientes -->
    <div v-if="hasData">
      <v-alert
        v-if="adaptive?.applies"
        :type="adaptiveAlertType"
        variant="tonal"
        density="compact"
        class="mb-4"
        :icon="adaptiveAlertIcon"
      >
        <div class="font-weight-medium">{{ adaptiveTitle }}</div>
        <div class="text-caption mt-1">
          Atual {{ formatLatency(adaptive.currentLatencyMs) }} · esperado
          {{ formatLatency(adaptive.expectedLatencyMs) }} · alerta a partir de
          {{ formatLatency(adaptive.alertThresholdMs) }} por
          {{ adaptive.requiredConsecutiveChecks }} leituras consecutivas. Confirmações atuais:
          {{ adaptive.observedConsecutiveChecks }}.
          <template v-if="adaptive.linkUtilizationPercent !== null">
            WAN em {{ formatPercent(adaptive.linkUtilizationPercent)
            }}<template v-if="adaptive.linkInterfaceName">
              ({{ adaptive.linkInterfaceName }}) </template
            >.
          </template>
          <template v-else>
            Sem telemetria WAN suficiente; a latência continua sendo avaliada, sem presumir
            saturação.
          </template>
        </div>
      </v-alert>

      <v-row class="mb-2">
        <!-- KPI: Latência Estatística -->
        <v-col cols="12" sm="6" md="4">
          <div class="pa-4 rounded-lg bg-surface-variant h-100 border">
            <div class="d-flex align-center justify-space-between mb-1">
              <span class="text-caption font-weight-bold text-medium-emphasis">
                LATÊNCIA (MÉDIA & DESVIO)
              </span>
              <v-chip
                v-if="baseline?.latencyZScore !== null && baseline?.latencyZScore !== undefined"
                :color="latencyZColor"
                size="x-small"
                variant="flat"
                class="font-weight-bold"
              >
                Z: {{ formatZScore(baseline.latencyZScore) }}
              </v-chip>
            </div>
            <div class="d-flex align-baseline ga-2 my-1">
              <span class="text-h5 font-weight-bold text-primary">
                {{ formatLatency(baseline?.latencyBaselineMs) }}
              </span>
              <span class="text-caption text-grey">
                ± {{ formatLatency(baseline?.latencyStddevMs) }} (1σ)
              </span>
            </div>
            <div class="text-caption text-grey-darken-1 mt-2">
              <div class="d-flex justify-space-between">
                <span>Faixa Normal (3σ):</span>
                <strong>
                  {{ formatLatency(baseline?.latencyLowerBandMs) }} –
                  {{ formatLatency(baseline?.latencyUpperBandMs) }}
                </strong>
              </div>
            </div>
          </div>
        </v-col>

        <!-- KPI: Perda de Pacotes Estatística -->
        <v-col cols="12" sm="6" md="4">
          <div class="pa-4 rounded-lg bg-surface-variant h-100 border">
            <div class="d-flex align-center justify-space-between mb-1">
              <span class="text-caption font-weight-bold text-medium-emphasis">
                PERDA DE PACOTES (HISTÓRICO)
              </span>
              <v-chip
                v-if="
                  baseline?.packetLossZScore !== null && baseline?.packetLossZScore !== undefined
                "
                :color="lossZColor"
                size="x-small"
                variant="flat"
                class="font-weight-bold"
              >
                Z: {{ formatZScore(baseline.packetLossZScore) }}
              </v-chip>
            </div>
            <div class="d-flex align-baseline ga-2 my-1">
              <span class="text-h5 font-weight-bold text-info">
                {{ formatPercent(baseline?.packetLossBaselinePercent) }}
              </span>
              <span class="text-caption text-grey">
                ± {{ formatPercent(baseline?.packetLossStddevPercent) }}
              </span>
            </div>
            <div class="text-caption text-grey-darken-1 mt-2">
              <div class="d-flex justify-space-between">
                <span>Teto Normal (3σ):</span>
                <strong>{{ formatPercent(baseline?.packetLossUpperBandPercent) }}</strong>
              </div>
            </div>
          </div>
        </v-col>

        <!-- KPI: Disponibilidade Histórica -->
        <v-col cols="12" sm="12" md="4">
          <div class="pa-4 rounded-lg bg-surface-variant h-100 border">
            <div class="d-flex align-center justify-space-between mb-1">
              <span class="text-caption font-weight-bold text-medium-emphasis">
                UPTIME HISTÓRICO (7 DIAS)
              </span>
              <v-chip size="x-small" color="success" variant="outlined">
                {{ baseline?.sampleCount }} horas
              </v-chip>
            </div>
            <div class="d-flex align-baseline ga-2 my-1">
              <span class="text-h5 font-weight-bold text-success">
                {{ formatPercent(baseline?.uptimeBaselinePercent) }}
              </span>
              <span class="text-caption text-grey">
                ± {{ formatPercent(baseline?.uptimeStddevPercent) }}
              </span>
            </div>
            <div class="text-caption text-grey-darken-1 mt-2">
              <div class="d-flex justify-space-between">
                <span>Janela Amostral:</span>
                <strong>{{ baseline?.windowDays }} dias históricos</strong>
              </div>
            </div>
          </div>
        </v-col>
      </v-row>
    </div>

    <!-- Estado de Coleta Inicial -->
    <div v-else class="text-center text-grey py-6 border rounded-lg bg-grey-lighten-5">
      <v-icon size="40" color="grey-lighten-1">mdi-chart-scatter-plot</v-icon>
      <div class="mt-2 text-subtitle-2 font-weight-medium">
        Histórico insuficiente para cálculo de desvio padrão e Z-Score.
      </div>
      <div class="text-caption text-grey">
        A linha de base estatística exige pelo menos 6 buckets horários de dados para estimar médias
        e bandas de 3σ com precisão estatística.
      </div>
    </div>
  </v-card>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { AdaptiveLatencyAssessment } from '@/stores/monitors'

export interface MonitorBaselinePayload {
  monitorId: number
  monitorName: string
  monitorType: string
  hasSufficientData: boolean
  baseline: {
    latencyBaselineMs?: number | null
    latencyStddevMs?: number | null
    latencyDeviationPercent?: number | null
    latencyZScore?: number | null
    latencyUpperBandMs?: number | null
    latencyLowerBandMs?: number | null
    isLatencyAnomaly?: boolean | null
    packetLossBaselinePercent?: number | null
    packetLossStddevPercent?: number | null
    packetLossDeviationPercent?: number | null
    packetLossZScore?: number | null
    packetLossUpperBandPercent?: number | null
    isPacketLossAnomaly?: boolean | null
    uptimeBaselinePercent?: number | null
    uptimeStddevPercent?: number | null
    uptimeDeviationPercent?: number | null
    uptimeZScore?: number | null
    sampleCount?: number
    windowDays?: number
  }
  current: {
    latencyMs?: number | null
    packetLossPercent?: number | null
    uptimePercent?: number | null
  }
  adaptiveLatency?: AdaptiveLatencyAssessment
}

const props = defineProps<{
  baselineData: MonitorBaselinePayload | null
}>()

defineEmits<{
  (e: 'createAnomalyRule'): void
}>()

const hasData = computed(() => {
  return Boolean(
    props.baselineData?.hasSufficientData &&
    (props.baselineData?.baseline?.latencyBaselineMs !== undefined ||
      props.baselineData?.baseline?.packetLossBaselinePercent !== undefined)
  )
})

const baseline = computed(() => props.baselineData?.baseline)
const adaptive = computed(() => props.baselineData?.adaptiveLatency)

const isAnomaly = computed(() => {
  const adaptiveLatencyAnomaly = adaptive.value?.applies
    ? adaptive.value.alertEligible
    : Boolean(
        baseline.value?.isLatencyAnomaly ||
        (baseline.value?.latencyZScore !== null &&
          baseline.value?.latencyZScore !== undefined &&
          baseline.value.latencyZScore >= 3.0)
      )
  return Boolean(
    adaptiveLatencyAnomaly ||
    baseline.value?.isPacketLossAnomaly ||
    (baseline.value?.packetLossZScore !== null &&
      baseline.value?.packetLossZScore !== undefined &&
      baseline.value.packetLossZScore >= 3.0)
  )
})

const adaptiveAlertType = computed<'success' | 'info' | 'warning' | 'error'>(() => {
  if (adaptive.value?.alertEligible) return 'error'
  if (adaptive.value?.reason === 'link_saturated') return 'warning'
  if (adaptive.value?.reason === 'within_expected_range') return 'success'
  return 'info'
})

const adaptiveAlertIcon = computed(() => {
  if (adaptive.value?.alertEligible) return 'mdi-bell-alert'
  if (adaptive.value?.reason === 'link_saturated') return 'mdi-speedometer-slow'
  if (adaptive.value?.reason === 'within_expected_range') return 'mdi-check-circle-outline'
  return 'mdi-chart-timeline-variant-shimmer'
})

const adaptiveTitle = computed(() => {
  switch (adaptive.value?.reason) {
    case 'alert_ready':
      return 'Degradação confirmada: alerta de latência liberado'
    case 'link_saturated':
      return 'Alerta de latência suprimido: WAN saturada nesta leitura'
    case 'collecting_confirmations':
      return 'Desvio detectado: aguardando confirmações consecutivas'
    case 'within_expected_range':
      return 'Latência dentro do comportamento esperado para este destino'
    case 'latency_unavailable':
      return 'Esta leitura não possui uma medida de latência'
    case 'evaluation_unavailable':
      return 'Contexto adaptativo indisponível; regras existentes foram mantidas por segurança'
    default:
      return 'Aprendendo a latência normal deste destino'
  }
})

const latencyZColor = computed(() => {
  const z = baseline.value?.latencyZScore
  if (z === null || z === undefined) return 'grey'
  if (z >= 3.0) return 'error'
  if (z >= 2.0) return 'warning'
  return 'success'
})

const lossZColor = computed(() => {
  const z = baseline.value?.packetLossZScore
  if (z === null || z === undefined) return 'grey'
  if (z >= 3.0) return 'error'
  if (z >= 2.0) return 'warning'
  return 'success'
})

function formatLatency(val?: number | null): string {
  if (val === null || val === undefined || isNaN(val)) return '—'
  return `${val.toFixed(1)} ms`
}

function formatPercent(val?: number | null): string {
  if (val === null || val === undefined || isNaN(val)) return '—'
  return `${val.toFixed(1)}%`
}

function formatZScore(z?: number | null): string {
  if (z === null || z === undefined || isNaN(z)) return '0.0σ'
  const sign = z > 0 ? '+' : ''
  return `${sign}${z.toFixed(1)}σ`
}
</script>

<style scoped>
.anomaly-pulse-chip {
  animation: anomalyPulse 2s infinite ease-in-out;
}

@keyframes anomalyPulse {
  0% {
    box-shadow: 0 0 0 0 rgba(244, 67, 54, 0.4);
  }
  70% {
    box-shadow: 0 0 0 8px rgba(244, 67, 54, 0);
  }
  100% {
    box-shadow: 0 0 0 0 rgba(244, 67, 54, 0);
  }
}
</style>
