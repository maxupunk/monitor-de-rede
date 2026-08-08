<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 850"
    :fullscreen="$vuetify.display.xs"
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card class="rounded-lg">
      <!-- Header do Modal -->
      <v-card-title class="d-flex align-center justify-space-between pa-4 bg-primary text-white">
        <div class="d-flex align-center ga-2" style="gap: 8px">
          <v-icon>mdi-chart-areaspline</v-icon>
          <span>Histórico de Tráfego: {{ interfaceName || 'Interface' }}</span>
        </div>
        <v-btn icon variant="text" color="white" @click="emit('update:modelValue', false)">
          <v-icon>mdi-close</v-icon>
        </v-btn>
      </v-card-title>

      <v-card-text class="pa-6">
        <!-- Seletor de Tipo de Métrica -->
        <div class="d-flex align-center justify-space-between flex-wrap mb-6 ga-3">
          <v-btn-toggle
            v-model="selectedTab"
            color="primary"
            variant="outlined"
            mandatory
            density="comfortable"
          >
            <v-btn value="inBps" prepend-icon="mdi-arrow-down-bold"> Download (IN) </v-btn>
            <v-btn value="outBps" prepend-icon="mdi-arrow-up-bold"> Upload (OUT) </v-btn>
            <v-btn value="combined" prepend-icon="mdi-swap-horizontal"> Combinado </v-btn>
            <v-btn value="inOctets" prepend-icon="mdi-database-arrow-down"> Volumetria IN </v-btn>
            <v-btn value="outOctets" prepend-icon="mdi-database-arrow-up"> Volumetria OUT </v-btn>
          </v-btn-toggle>

          <v-chip size="small" color="info" variant="tonal" class="px-3">
            <v-icon start size="14">mdi-history</v-icon>
            {{ sampleCount }} amostra(s)
          </v-chip>
        </div>

        <!-- Cards KPI de Estatísticas -->
        <v-row class="mb-6">
          <v-col cols="12" sm="3">
            <v-card border flat class="pa-3 rounded-lg text-center">
              <div class="text-caption text-grey">Atual</div>
              <div class="text-subtitle-1 font-weight-bold text-primary">
                {{ formattedStats.current }}
              </div>
            </v-card>
          </v-col>
          <v-col cols="12" sm="3">
            <v-card border flat class="pa-3 rounded-lg text-center">
              <div class="text-caption text-grey">Média</div>
              <div class="text-subtitle-1 font-weight-bold text-info">
                {{ formattedStats.avg }}
              </div>
            </v-card>
          </v-col>
          <v-col cols="12" sm="3">
            <v-card border flat class="pa-3 rounded-lg text-center">
              <div class="text-caption text-grey">Mínimo</div>
              <div class="text-subtitle-1 font-weight-bold text-success">
                {{ formattedStats.min }}
              </div>
            </v-card>
          </v-col>
          <v-col cols="12" sm="3">
            <v-card border flat class="pa-3 rounded-lg text-center">
              <div class="text-caption text-grey">Máximo</div>
              <div class="text-subtitle-1 font-weight-bold text-error">
                {{ formattedStats.max }}
              </div>
            </v-card>
          </v-col>
        </v-row>

        <!-- Componente Unificado de Gráfico de Métricas -->
        <BaseMetricChart
          v-if="chartSeries.length > 0 && sampleCount > 0"
          :series="chartSeries"
          :unit-type="isRateMetric ? 'bandwidth' : 'bytes'"
        />

        <div v-else class="text-center text-grey py-10 border rounded-lg bg-grey-lighten-5">
          <v-icon size="48" color="grey-lighten-1">mdi-chart-line-variant</v-icon>
          <div class="mt-2 text-subtitle-1 font-weight-medium">
            Nenhuma métrica capturada ainda para este filtro.
          </div>
          <div class="text-caption">
            Clique em "Poll SNMP Agora" na página do equipamento para gerar amostras.
          </div>
        </div>
      </v-card-text>

      <v-divider></v-divider>

      <v-card-actions class="pa-4 justify-end">
        <v-btn variant="text" color="grey-darken-1" @click="emit('update:modelValue', false)">
          Fechar
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import type { DeviceMetric } from '@/stores/deviceDetail'
import BaseMetricChart, { type ChartSeriesInput } from '@/components/BaseMetricChart.vue'
import { formatBps, formatBytes } from '@/utils/formatters'

interface Props {
  modelValue: boolean
  interfaceId: number | null
  interfaceName: string
  initialMetric?: 'inBps' | 'outBps' | 'inOctets' | 'outOctets' | 'combined'
  metrics: DeviceMetric[]
}

const props = withDefaults(defineProps<Props>(), {
  initialMetric: 'inBps',
})

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
}>()

const selectedTab = ref<'inBps' | 'outBps' | 'inOctets' | 'outOctets' | 'combined'>('inBps')

watch(
  () => props.initialMetric,
  (newVal) => {
    if (newVal) selectedTab.value = newVal
  },
  { immediate: true }
)

// Filtra e ordena as métricas correspondentes à interface selecionada
const filteredMetrics = computed(() => {
  if (!props.interfaceId) return []
  return props.metrics
    .filter((m) => m.interfaceId === props.interfaceId)
    .sort((a, b) => {
      return new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime()
    })
})

const isRateMetric = computed(() => {
  return (
    selectedTab.value === 'inBps' ||
    selectedTab.value === 'outBps' ||
    selectedTab.value === 'combined'
  )
})

// Constrói a estrutura unificada do componente BaseMetricChart
const chartSeries = computed<ChartSeriesInput[]>(() => {
  if (selectedTab.value === 'combined') {
    const inList = filteredMetrics.value.filter((m) => m.metricName === 'inBps')
    const outList = filteredMetrics.value.filter((m) => m.metricName === 'outBps')

    const series: ChartSeriesInput[] = []
    if (inList.length > 0) {
      series.push({
        id: 'inBps',
        label: 'Taxa Download (IN)',
        color: '#4CAF50',
        fillArea: false,
        data: inList.map((m) => ({
          time: m.createdAt,
          value: Number(m.metricValue) || 0,
          formattedValue: formatBps(Number(m.metricValue) || 0),
        })),
      })
    }
    if (outList.length > 0) {
      series.push({
        id: 'outBps',
        label: 'Taxa Upload (OUT)',
        color: '#2196F3',
        fillArea: false,
        data: outList.map((m) => ({
          time: m.createdAt,
          value: Number(m.metricValue) || 0,
          formattedValue: formatBps(Number(m.metricValue) || 0),
        })),
      })
    }
    return series
  }

  const targetNames =
    selectedTab.value === 'inBps'
      ? ['inBps']
      : selectedTab.value === 'outBps'
        ? ['outBps']
        : selectedTab.value === 'inOctets'
          ? ['ifHCInOctets', 'ifInOctets']
          : ['ifHCOutOctets', 'ifOutOctets']

  const list = filteredMetrics.value.filter((m) => targetNames.includes(m.metricName))
  if (list.length === 0) return []

  const isRate = isRateMetric.value
  const label = formatMetricLabel(selectedTab.value)
  const color = selectedTab.value.toLowerCase().includes('out') ? '#2196F3' : '#4CAF50'

  return [
    {
      id: selectedTab.value,
      label,
      color,
      fillArea: true,
      data: list.map((m) => {
        const val = Number(m.metricValue) || 0
        return {
          time: m.createdAt,
          value: val,
          formattedValue: isRate ? formatBps(val) : formatBytes(val),
        }
      }),
    },
  ]
})

const sampleCount = computed(() => {
  if (chartSeries.value.length === 0) return 0
  return chartSeries.value[0].data.length
})

const numericValues = computed(() => {
  const vals: number[] = []
  for (const s of chartSeries.value) {
    for (const d of s.data) {
      vals.push(d.value)
    }
  }
  return vals
})

const maxVal = computed(() => {
  if (numericValues.value.length === 0) return 0
  return Math.max(...numericValues.value)
})

const minVal = computed(() => {
  if (numericValues.value.length === 0) return 0
  return Math.min(...numericValues.value)
})

const avgVal = computed(() => {
  if (numericValues.value.length === 0) return 0
  const sum = numericValues.value.reduce((a, b) => a + b, 0)
  return Math.round(sum / numericValues.value.length)
})

const currentVal = computed(() => {
  if (numericValues.value.length === 0) return 0
  return numericValues.value[numericValues.value.length - 1]
})

const formattedStats = computed(() => {
  const formatter = isRateMetric.value ? formatBps : formatBytes
  return {
    current: formatter(currentVal.value),
    avg: formatter(avgVal.value),
    min: formatter(minVal.value),
    max: formatter(maxVal.value),
  }
})

function formatMetricLabel(name: string): string {
  switch (name) {
    case 'inBps':
      return 'Taxa Download (IN)'
    case 'outBps':
      return 'Taxa Upload (OUT)'
    case 'inOctets':
      return 'Volumetria Entrada'
    case 'outOctets':
      return 'Volumetria Saída'
    default:
      return name
  }
}
</script>
