<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 900"
    :fullscreen="$vuetify.display.xs"
    scrollable
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card class="rounded-lg">
      <v-card-title class="d-flex align-center justify-space-between pa-4 bg-primary text-white">
        <div class="d-flex align-center ga-2">
          <v-icon>{{ icon || 'mdi-chart-areaspline' }}</v-icon>
          <span>Histórico: {{ title }}</span>
        </div>
        <v-btn
          icon
          variant="text"
          color="white"
          aria-label="Fechar"
          @click="emit('update:modelValue', false)"
        >
          <v-icon>mdi-close</v-icon>
        </v-btn>
      </v-card-title>

      <v-card-text class="pa-6">
        <div class="d-flex flex-wrap align-center ga-4 mb-4">
          <div>
            <div class="text-caption text-grey">Leitura atual</div>
            <div class="text-h5 font-weight-bold">{{ atual }}</div>
          </div>
          <v-divider vertical class="d-none d-sm-block" />
          <div>
            <div class="text-caption text-grey">Mínimo · Médio · Máximo</div>
            <div class="text-body-1 font-weight-medium">
              {{ minimo }} · {{ medio }} · {{ maximo }}
            </div>
          </div>
          <v-divider vertical class="d-none d-sm-block" />
          <div>
            <div class="text-caption text-grey">Amostras</div>
            <div class="text-body-1 font-weight-medium">{{ pontos.length }}</div>
          </div>
        </div>

        <BaseMetricChart
          v-if="pontos.length > 1"
          :series="[serie]"
          :unit-type="unitType ?? 'generic'"
          :custom-unit="customUnit ?? ''"
          show-avg-line
          :avg-value="mediaBruta"
        />

        <v-alert v-else type="info" variant="tonal" density="comfortable" class="rounded-lg">
          Ainda não há amostras suficientes para desenhar um gráfico. A série precisa de pelo menos
          duas coletas.
        </v-alert>
      </v-card-text>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
/**
 * O gráfico de **uma série de dispositivo**, aberto a partir do card que a
 * originou.
 *
 * É o par do `TrafficChartDialog` para as séries que não são de interface:
 * CPU, memória, armazenamento, carga, memória do processo e uptime. Os dois
 * usam o mesmo `BaseMetricChart` — o que muda é o contexto que cerca o
 * gráfico, e é justamente por isso que não são o mesmo componente: o de
 * tráfego oferece incluir/remover a interface do monitoramento, ação que não
 * existe para uma série de host.
 *
 * A regra de layout do roadmap é literal aqui: "gráficos detalhados são
 * abertos a partir do card, monitor ou interface que os originou; não existe
 * uma aba depósito para todas as séries".
 */
import { computed } from 'vue'
import BaseMetricChart, { type ChartSeriesInput } from '@/components/BaseMetricChart.vue'
import type { DeviceMetric } from '@/stores/deviceDetail'

const props = defineProps<{
  modelValue: boolean
  /** Nome da série em `metrics.name`, mais os nomes herdados equivalentes. */
  metricNames: string[]
  title: string
  icon?: string
  color?: string
  unitType?: 'bandwidth' | 'bytes' | 'latency' | 'percentage' | 'generic'
  customUnit?: string
  metrics: DeviceMetric[]
  /** Formatador do resumo numérico, o mesmo usado no card. */
  format?: (valor: number) => string
}>()

const emit = defineEmits<{ (e: 'update:modelValue', value: boolean): void }>()

/**
 * As amostras, do mais antigo para o mais novo.
 *
 * `detailStore.metrics` chega do mais recente para o mais antigo; o gráfico
 * precisa do sentido contrário para o tempo fluir da esquerda para a direita.
 */
const pontos = computed(() =>
  props.metrics
    .filter((metrica) => props.metricNames.includes(metrica.metricName))
    .map((metrica) => ({
      time: metrica.createdAt,
      value: Number(metrica.metricValue) || 0,
      formattedValue: props.format ? props.format(Number(metrica.metricValue) || 0) : undefined,
    }))
    .reverse()
)

/** A série no formato do `BaseMetricChart`. */
const serie = computed<ChartSeriesInput>(() => ({
  id: props.metricNames[0] ?? 'metric',
  label: props.title,
  color: props.color ?? '#1976d2',
  fillArea: true,
  data: pontos.value,
}))

const valores = computed(() => pontos.value.map((ponto) => ponto.value))

function formata(valor: number | null): string {
  if (valor === null) return '—'
  return props.format ? props.format(valor) : `${Math.round(valor * 10) / 10}%`
}

const atual = computed(() =>
  formata(valores.value.length ? valores.value[valores.value.length - 1] : null)
)
const minimo = computed(() => formata(valores.value.length ? Math.min(...valores.value) : null))
const maximo = computed(() => formata(valores.value.length ? Math.max(...valores.value) : null))

const mediaBruta = computed(() =>
  valores.value.length
    ? valores.value.reduce((soma, valor) => soma + valor, 0) / valores.value.length
    : 0
)
const medio = computed(() => formata(valores.value.length ? mediaBruta.value : null))
</script>
