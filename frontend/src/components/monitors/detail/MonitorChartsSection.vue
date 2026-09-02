<template>
  <div>
    <!-- Gráfico de Tráfego de Rede (IN/OUT bps) -->
    <v-card v-if="isTrafficMonitor" elevation="2" class="rounded-lg pa-3 pa-md-5 mb-3 mb-md-6">
      <div class="d-flex align-center justify-space-between mb-3 mb-md-4 flex-wrap ga-3">
        <div>
          <h2 class="text-h6 font-weight-bold d-flex align-center ga-2">
            <v-icon color="primary">mdi-chart-areaspline</v-icon>
            Histórico de Tráfego de Rede
          </h2>
          <div class="text-subtitle-2 text-grey">
            Throughput de transmissão e recepção coletado via SNMP
          </div>
        </div>
        <v-btn-toggle
          :model-value="trafficTab"
          color="primary"
          variant="outlined"
          mandatory
          density="compact"
          @update:model-value="emit('update:trafficTab', $event)"
        >
          <v-btn value="inBps" size="small" prepend-icon="mdi-arrow-down-bold">
            Download (IN)
          </v-btn>
          <v-btn value="outBps" size="small" prepend-icon="mdi-arrow-up-bold"> Upload (OUT) </v-btn>
          <v-btn value="combined" size="small" prepend-icon="mdi-swap-horizontal">
            Combinado
          </v-btn>
        </v-btn-toggle>
      </div>

      <BaseMetricChart
        v-if="trafficSeries.length > 0 && trafficSeries[0].data.length > 0"
        :series="trafficSeries"
        unit-type="bandwidth"
      />

      <div v-else class="text-center text-grey py-8 border rounded-lg bg-grey-lighten-5">
        <v-icon size="40" color="grey-lighten-1">mdi-chart-line-variant</v-icon>
        <div class="mt-2 text-subtitle-2">
          Histórico de tráfego insuficiente para gerar o gráfico.
        </div>
        <div class="text-caption">
          As taxas são calculadas pela varredura SNMP periódica do dispositivo.
        </div>
      </div>
    </v-card>

    <!-- Linha do Tempo de Status (Bar Timeline - Estilo Uptime Kuma) -->
    <v-card
      v-if="!isGaugeMonitor && !isTrafficMonitor"
      elevation="2"
      class="rounded-lg pa-3 pa-md-5 mb-3 mb-md-6"
    >
      <div class="d-flex align-center justify-space-between mb-3 mb-md-4 flex-wrap ga-2">
        <div>
          <h2 class="text-h6 font-weight-bold d-flex align-center ga-2">
            <v-icon color="primary">mdi-chart-timeline-variant</v-icon>
            Linha do Tempo de Status
          </h2>
          <div class="text-subtitle-2 text-grey">Histórico recente de verificações de status</div>
        </div>
        <div class="d-flex align-center ga-3 text-caption flex-wrap">
          <span v-if="statusBreakdown.up" class="d-flex align-center ga-1">
            <span class="status-indicator-dot bg-success"></span> UP ({{ statusBreakdown.up }})
          </span>
          <span v-if="statusBreakdown.down" class="d-flex align-center ga-1">
            <span class="status-indicator-dot bg-error"></span> DOWN ({{ statusBreakdown.down }})
          </span>
          <span v-if="statusBreakdown.warning" class="d-flex align-center ga-1">
            <span class="status-indicator-dot bg-warning"></span> INSTÁVEL ({{
              statusBreakdown.warning
            }})
          </span>
          <span v-if="statusBreakdown.disabled" class="d-flex align-center ga-1">
            <span class="status-indicator-dot" style="background-color: #9e9e9e"></span>
            DESABILITADA ({{ statusBreakdown.disabled }})
          </span>
          <span v-if="statusBreakdown.unknown" class="d-flex align-center ga-1">
            <span class="status-indicator-dot" style="background-color: #b0bec5"></span>
            DESCONHECIDO ({{ statusBreakdown.unknown }})
          </span>
          <span class="text-grey font-weight-bold">Total: {{ totalChecks }}</span>
        </div>
      </div>

      <div class="pa-2 pa-sm-3 bg-grey-lighten-4 rounded-lg d-flex w-100">
        <MonitorTimelineBar
          :results="recentResults"
          :max-blocks="60"
          :height="36"
          :responsive="true"
        />
      </div>
    </v-card>

    <!-- Gráfico de Uso ao Longo do Tempo (CPU/Memória) -->
    <v-card
      v-if="isGaugeMonitor && !isTrafficMonitor"
      elevation="2"
      class="rounded-lg pa-3 pa-md-5 mb-3 mb-md-6"
    >
      <div class="d-flex align-center justify-space-between mb-3 mb-md-4">
        <div>
          <h2 class="text-h6 font-weight-bold d-flex align-center ga-2">
            <v-icon color="info">mdi-sine-wave</v-icon>
            Gráfico de Uso de {{ gaugeType === 'MEMÓRIA' ? 'Memória' : 'CPU' }}
          </h2>
          <div class="text-subtitle-2 text-grey">
            {{
              gaugeType === 'MEMÓRIA'
                ? 'Quantidade usada coletada via SNMP no dispositivo ao longo do tempo'
                : 'Percentual de uso coletado via SNMP no dispositivo ao longo do tempo'
            }}
          </div>
        </div>
        <v-chip v-if="gaugeAvg !== null" color="info" size="small" variant="outlined">
          Média: {{ gaugeAvgText }}
        </v-chip>
      </div>

      <BaseMetricChart
        v-if="gaugeSeries.length > 0 && gaugeSeries[0].data.length > 0"
        :series="gaugeSeries"
        :unit-type="gaugeUnitType"
        :show-avg-line="true"
        :avg-value="gaugeAvg ?? undefined"
      />

      <div v-else class="text-center text-grey py-8 border rounded-lg bg-grey-lighten-5">
        <v-icon size="40" color="grey-lighten-1">mdi-chart-line-variant</v-icon>
        <div class="mt-2 text-subtitle-2">Histórico insuficiente para gerar o gráfico de uso.</div>
        <div class="text-caption">As amostras vêm da varredura SNMP periódica do dispositivo.</div>
      </div>
    </v-card>

    <!-- Gráfico Unificado de Latência / Tempo de Resposta -->
    <v-card
      v-if="!isGaugeMonitor && !isInterfaceMonitor && !isTrafficMonitor"
      elevation="2"
      class="rounded-lg pa-3 pa-md-5 mb-3 mb-md-6"
    >
      <div class="d-flex align-center justify-space-between mb-3 mb-md-4">
        <div>
          <h2 class="text-h6 font-weight-bold d-flex align-center ga-2">
            <v-icon color="info">mdi-sine-wave</v-icon>
            Gráfico de Tempo de Resposta (Ping Latency)
          </h2>
          <div class="text-subtitle-2 text-grey">
            Variação da latência em milissegundos (ms) ao longo do tempo
          </div>
        </div>
        <v-chip v-if="avgLatency" color="info" size="small" variant="outlined">
          Média: {{ formatLatency(avgLatency) }}
        </v-chip>
      </div>

      <BaseMetricChart
        v-if="latencySeries.length > 0 && latencySeries[0].data.length > 0"
        :series="latencySeries"
        unit-type="latency"
        :show-avg-line="true"
        :avg-value="avgLatency || undefined"
      />

      <div v-else class="text-center text-grey py-8 border rounded-lg bg-grey-lighten-5">
        <v-icon size="40" color="grey-lighten-1">mdi-chart-line-variant</v-icon>
        <div class="mt-2 text-subtitle-2">
          Histórico insuficiente para gerar o gráfico de latência.
        </div>
        <div class="text-caption">Execute mais verificações clicando em "Testar Agora".</div>
      </div>
    </v-card>

    <!-- Gráfico de Consumo de Banda da Interface de Link (WAN/Uplink) Alinhado ao Tempo de Resposta -->
    <v-card
      v-if="!isGaugeMonitor && !isInterfaceMonitor && !isTrafficMonitor && linkInterfaceLabel"
      elevation="2"
      class="rounded-lg pa-3 pa-md-5 mb-3 mb-md-6"
    >
      <div class="d-flex align-center justify-space-between mb-3 mb-md-4 flex-wrap ga-3">
        <div>
          <div class="d-flex align-center ga-2 flex-wrap mb-1">
            <h2 class="text-h6 font-weight-bold d-flex align-center ga-2 mb-0">
              <v-icon color="primary">mdi-wan</v-icon>
              Consumo de Banda — Interface de Link
            </h2>
            <v-chip color="primary" variant="flat" size="small" prepend-icon="mdi-ethernet">
              {{ linkInterfaceLabel }}
            </v-chip>
          </div>
          <div class="text-subtitle-2 text-grey">
            Throughput de download e upload da interface de entrada principal coletado via SNMP
          </div>
        </div>

        <div class="d-flex align-center ga-2 flex-wrap">
          <v-chip v-if="latestLinkInBps !== null" color="success" size="small" variant="tonal">
            Download: {{ formatBps(latestLinkInBps) }}
          </v-chip>
          <v-chip v-if="latestLinkOutBps !== null" color="info" size="small" variant="tonal">
            Upload: {{ formatBps(latestLinkOutBps) }}
          </v-chip>
          <v-btn-toggle
            :model-value="linkTrafficTab"
            color="primary"
            variant="outlined"
            mandatory
            density="compact"
            @update:model-value="emit('update:linkTrafficTab', $event)"
          >
            <v-btn value="inBps" size="small" prepend-icon="mdi-arrow-down-bold">
              Download (IN)
            </v-btn>
            <v-btn value="outBps" size="small" prepend-icon="mdi-arrow-up-bold">
              Upload (OUT)
            </v-btn>
            <v-btn value="combined" size="small" prepend-icon="mdi-swap-horizontal">
              Combinado
            </v-btn>
          </v-btn-toggle>
        </div>
      </div>

      <BaseMetricChart
        v-if="
          linkTrafficSeries && linkTrafficSeries.length > 0 && linkTrafficSeries[0].data.length > 0
        "
        :series="linkTrafficSeries"
        unit-type="bandwidth"
      />

      <div v-else class="text-center text-grey py-8 border rounded-lg bg-grey-lighten-5">
        <v-icon size="40" color="grey-lighten-1">mdi-chart-line-variant</v-icon>
        <div class="mt-2 text-subtitle-2">
          Histórico de tráfego ainda não registrado para a interface {{ linkInterfaceLabel }}.
        </div>
        <div class="text-caption">
          As taxas serão calculadas a cada coleta SNMP periódica do dispositivo.
        </div>
      </div>
    </v-card>
  </div>
</template>

<script setup lang="ts">
import BaseMetricChart, { type ChartSeriesInput } from '@/components/BaseMetricChart.vue'
import MonitorTimelineBar from '@/components/MonitorTimelineBar.vue'
import type { MonitorResult } from '@/stores/monitors'
import { formatLatency, formatBps } from '@/utils/formatters'

withDefaults(
  defineProps<{
    isTrafficMonitor: boolean
    isGaugeMonitor: boolean
    isInterfaceMonitor: boolean
    trafficTab: 'inBps' | 'outBps' | 'combined'
    trafficSeries: ChartSeriesInput[]
    recentResults: MonitorResult[]
    statusBreakdown: {
      up: number
      down: number
      warning: number
      disabled: number
      unknown: number
    }
    totalChecks: number
    gaugeType: string
    gaugeAvg: number | null
    gaugeAvgText: string
    gaugeUnitType: 'bytes' | 'percentage'
    gaugeSeries: ChartSeriesInput[]
    avgLatency: number | null
    latencySeries: ChartSeriesInput[]
    linkInterfaceLabel?: string | null
    linkTrafficTab?: 'inBps' | 'outBps' | 'combined'
    linkTrafficSeries?: ChartSeriesInput[]
    latestLinkInBps?: number | null
    latestLinkOutBps?: number | null
  }>(),
  {
    linkInterfaceLabel: null,
    linkTrafficTab: 'combined',
    linkTrafficSeries: () => [],
    latestLinkInBps: null,
    latestLinkOutBps: null,
  }
)

const emit = defineEmits<{
  (e: 'update:trafficTab', value: 'inBps' | 'outBps' | 'combined'): void
  (e: 'update:linkTrafficTab', value: 'inBps' | 'outBps' | 'combined'): void
}>()
</script>

<style scoped>
.status-indicator-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  display: inline-block;
}
</style>
