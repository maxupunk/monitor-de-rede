<template>
  <div>
    <!-- Cards de Métricas KPI: Tráfego SNMP -->
    <v-row v-if="isTrafficMonitor" dense class="mb-3 mb-md-6">
      <v-col cols="12" sm="6" md="3">
        <v-card elevation="2" class="rounded-lg pa-3 pa-sm-4 h-100">
          <div class="d-flex align-center justify-space-between mb-2">
            <span class="text-subtitle-2 text-grey-darken-1 font-weight-medium"
            >Download Atual (IN)</span
            >
            <v-avatar color="success" variant="tonal" size="36">
              <v-icon size="20">mdi-arrow-down-bold</v-icon>
            </v-avatar>
          </div>
          <div class="text-h4 font-weight-bold my-1 text-success">
            {{ trafficInText }}
          </div>
          <div class="text-caption text-grey">Última taxa de recepção</div>
        </v-card>
      </v-col>

      <v-col cols="12" sm="6" md="3">
        <v-card elevation="2" class="rounded-lg pa-3 pa-sm-4 h-100">
          <div class="d-flex align-center justify-space-between mb-2">
            <span class="text-subtitle-2 text-grey-darken-1 font-weight-medium"
            >Upload Atual (OUT)</span
            >
            <v-avatar color="primary" variant="tonal" size="36">
              <v-icon size="20">mdi-arrow-up-bold</v-icon>
            </v-avatar>
          </div>
          <div class="text-h4 font-weight-bold my-1 text-primary">
            {{ trafficOutText }}
          </div>
          <div class="text-caption text-grey">Última taxa de transmissão</div>
        </v-card>
      </v-col>

      <v-col cols="12" sm="6" md="3">
        <v-card elevation="2" class="rounded-lg pa-3 pa-sm-4 h-100">
          <div class="d-flex align-center justify-space-between mb-2">
            <span class="text-subtitle-2 text-grey-darken-1 font-weight-medium"
            >Velocidade da Interface</span
            >
            <v-avatar color="info" variant="tonal" size="36">
              <v-icon size="20">mdi-speedometer</v-icon>
            </v-avatar>
          </div>
          <div class="text-h4 font-weight-bold my-1 text-info">
            {{ interfaceSpeedText }}
          </div>
          <div class="text-caption text-grey">Capacidade da porta negociada</div>
        </v-card>
      </v-col>

      <v-col cols="12" sm="6" md="3">
        <v-card elevation="2" class="rounded-lg pa-3 pa-sm-4 h-100">
          <div class="d-flex align-center justify-space-between mb-2">
            <span class="text-subtitle-2 text-grey-darken-1 font-weight-medium"
            >Status Operacional</span
            >
            <v-avatar :color="headerChip.color" variant="tonal" size="36">
              <v-icon size="20">{{ headerChip.icon }}</v-icon>
            </v-avatar>
          </div>
          <div class="text-h5 font-weight-bold my-1" :class="`text-${headerChip.color}`">
            {{ interfaceOperText }}
          </div>
          <div class="text-caption text-grey">Estado da interface no agente</div>
        </v-card>
      </v-col>
    </v-row>

    <!-- Cards de Métricas KPI: CPU / Memória (Gauge) -->
    <v-row v-else-if="isGaugeMonitor" dense class="mb-3 mb-md-6">
      <v-col cols="12" sm="6" md="3">
        <v-card elevation="2" class="rounded-lg pa-3 pa-sm-4 h-100">
          <div class="d-flex align-center justify-space-between mb-2">
            <span class="text-subtitle-2 text-grey-darken-1 font-weight-medium">Uso Atual</span>
            <v-avatar :color="gaugeColorValue" variant="tonal" size="36">
              <v-icon size="20">mdi-gauge</v-icon>
            </v-avatar>
          </div>
          <div class="text-h4 font-weight-bold my-1" :class="`text-${gaugeColorValue}`">
            {{ gaugeCurrentText }}
          </div>
          <div class="text-caption text-grey">Última leitura SNMP</div>
        </v-card>
      </v-col>

      <v-col cols="12" sm="6" md="3">
        <v-card elevation="2" class="rounded-lg pa-3 pa-sm-4 h-100">
          <div class="d-flex align-center justify-space-between mb-2">
            <span class="text-subtitle-2 text-grey-darken-1 font-weight-medium">Uso Médio</span>
            <v-avatar color="info" variant="tonal" size="36">
              <v-icon size="20">mdi-chart-line</v-icon>
            </v-avatar>
          </div>
          <div class="text-h4 font-weight-bold my-1 text-info">
            {{ gaugeAvgText }}
          </div>
          <div class="text-caption text-grey">Média do histórico coletado</div>
        </v-card>
      </v-col>

      <v-col cols="12" sm="6" md="3">
        <v-card elevation="2" class="rounded-lg pa-3 pa-sm-4 h-100">
          <div class="d-flex align-center justify-space-between mb-2">
            <span class="text-subtitle-2 text-grey-darken-1 font-weight-medium">Uso Mín / Máx</span>
            <v-avatar color="purple" variant="tonal" size="36">
              <v-icon size="20">mdi-swap-vertical</v-icon>
            </v-avatar>
          </div>
          <div class="text-h5 font-weight-bold my-1 text-purple">
            <span>{{ gaugeMinText }}</span>
            <span class="text-grey-darken-1 font-weight-regular text-subtitle-1 mx-1">/</span>
            <span>{{ gaugeMaxText }}</span>
          </div>
          <div class="text-caption text-grey">Mínimo e máximo do período</div>
        </v-card>
      </v-col>

      <v-col cols="12" sm="6" md="3">
        <v-card elevation="2" class="rounded-lg pa-3 pa-sm-4 h-100">
          <div class="d-flex align-center justify-space-between mb-2">
            <span class="text-subtitle-2 text-grey-darken-1 font-weight-medium"
            >Agente SNMP Disponível</span
            >
            <v-avatar color="success" variant="tonal" size="36">
              <v-icon size="20">mdi-check-decagram</v-icon>
            </v-avatar>
          </div>
          <div class="text-h4 font-weight-bold my-1 text-success">
            {{ stats.uptimePercentage }}%
          </div>
          <div class="text-caption text-grey">% de coletas SNMP com resposta</div>
        </v-card>
      </v-col>
    </v-row>

    <!-- Cards de Métricas KPI: Interface RFC 2863 -->
    <v-row v-else-if="isInterfaceMonitor" dense class="mb-3 mb-md-6">
      <v-col cols="12" sm="6" md="3">
        <v-card elevation="2" class="rounded-lg pa-3 pa-sm-4 h-100">
          <div class="d-flex align-center justify-space-between mb-2">
            <span class="text-subtitle-2 text-grey-darken-1 font-weight-medium"
            >Velocidade Negociada</span
            >
            <v-avatar :color="headerChip.color" variant="tonal" size="36">
              <v-icon size="20">{{ headerChip.icon }}</v-icon>
            </v-avatar>
          </div>
          <div class="text-h4 font-weight-bold my-1" :class="`text-${headerChip.color}`">
            {{ interfaceSpeedText }}
          </div>
          <div class="text-caption text-grey">Última verificação SNMP</div>
        </v-card>
      </v-col>

      <v-col cols="12" sm="6" md="3">
        <v-card elevation="2" class="rounded-lg pa-3 pa-sm-4 h-100">
          <div class="d-flex align-center justify-space-between mb-2">
            <span class="text-subtitle-2 text-grey-darken-1 font-weight-medium"
            >Status Operacional</span
            >
            <v-avatar color="info" variant="tonal" size="36">
              <v-icon size="20">mdi-information-outline</v-icon>
            </v-avatar>
          </div>
          <div class="text-h5 font-weight-bold my-1 text-info">
            {{ interfaceOperText }}
          </div>
          <div class="text-caption text-grey">ifOperStatus / ifAdminStatus (SNMP)</div>
        </v-card>
      </v-col>

      <v-col cols="12" sm="6" md="3">
        <v-card elevation="2" class="rounded-lg pa-3 pa-sm-4 h-100">
          <div class="d-flex align-center justify-space-between mb-2">
            <span class="text-subtitle-2 text-grey-darken-1 font-weight-medium"
            >Estabilidade do Link</span
            >
            <v-avatar color="success" variant="tonal" size="36">
              <v-icon size="20">mdi-check-decagram</v-icon>
            </v-avatar>
          </div>
          <div class="text-h4 font-weight-bold my-1 text-success">
            {{ stats.uptimePercentage }}%
          </div>
          <div class="text-caption text-grey">% de verificações com link Up</div>
        </v-card>
      </v-col>

      <v-col cols="12" sm="6" md="3">
        <v-card elevation="2" class="rounded-lg pa-3 pa-sm-4 h-100">
          <div class="d-flex align-center justify-space-between mb-2">
            <span class="text-subtitle-2 text-grey-darken-1 font-weight-medium"
            >Alterações de Estado</span
            >
            <v-avatar
              :color="interfaceFlapCount > 0 ? 'warning' : 'grey'"
              variant="tonal"
              size="36"
            >
              <v-icon size="20">mdi-swap-horizontal</v-icon>
            </v-avatar>
          </div>
          <div
            class="text-h4 font-weight-bold my-1"
            :class="interfaceFlapCount > 0 ? 'text-warning' : 'text-grey'"
          >
            {{ interfaceFlapCount }}
          </div>
          <div class="text-caption text-grey">Trocas de status no período exibido</div>
        </v-card>
      </v-col>
    </v-row>

    <!-- Cards de Métricas KPI: Ping / HTTP / TCP / DNS -->
    <v-row v-else dense class="mb-3 mb-md-6">
      <v-col cols="12" sm="6" md="3">
        <v-card elevation="2" class="rounded-lg pa-3 pa-sm-4 h-100">
          <div class="d-flex align-center justify-space-between mb-2">
            <span class="text-subtitle-2 text-grey-darken-1 font-weight-medium">{{
              latencyKpiTitles.current
            }}</span>
            <v-avatar color="primary" variant="tonal" size="36">
              <v-icon size="20">mdi-speedometer</v-icon>
            </v-avatar>
          </div>
          <div
            class="text-h4 font-weight-bold my-1"
            :class="stats.lastLatency !== null ? 'text-primary' : 'text-grey'"
          >
            {{ lastLatencyText }}
          </div>
          <div class="text-caption text-grey">{{ latencyKpiTitles.currentCaption }}</div>
        </v-card>
      </v-col>

      <v-col cols="12" sm="6" md="3">
        <v-card elevation="2" class="rounded-lg pa-3 pa-sm-4 h-100">
          <div class="d-flex align-center justify-space-between mb-2">
            <span class="text-subtitle-2 text-grey-darken-1 font-weight-medium">{{
              latencyKpiTitles.avg
            }}</span>
            <v-avatar color="info" variant="tonal" size="36">
              <v-icon size="20">mdi-chart-line</v-icon>
            </v-avatar>
          </div>
          <div class="text-h4 font-weight-bold my-1 text-info">
            {{ avgLatencyText }}
          </div>
          <div class="text-caption text-grey">{{ latencyKpiTitles.avgCaption }}</div>
        </v-card>
      </v-col>

      <v-col cols="12" sm="6" md="3">
        <v-card elevation="2" class="rounded-lg pa-3 pa-sm-4 h-100">
          <div class="d-flex align-center justify-space-between mb-2">
            <span class="text-subtitle-2 text-grey-darken-1 font-weight-medium">{{
              latencyKpiTitles.minMax
            }}</span>
            <v-avatar color="purple" variant="tonal" size="36">
              <v-icon size="20">mdi-swap-vertical</v-icon>
            </v-avatar>
          </div>
          <div class="text-h5 font-weight-bold my-1 text-purple">
            <span>{{ minLatencyText }}</span>
            <span class="text-grey-darken-1 font-weight-regular text-subtitle-1 mx-1">/</span>
            <span>{{ maxLatencyText }}</span>
          </div>
          <div class="text-caption text-grey">{{ latencyKpiTitles.minMaxCaption }}</div>
        </v-card>
      </v-col>

      <v-col cols="12" sm="6" md="3">
        <v-card elevation="2" class="rounded-lg pa-3 pa-sm-4 h-100">
          <div class="d-flex align-center justify-space-between mb-2">
            <span class="text-subtitle-2 text-grey-darken-1 font-weight-medium"
            >Taxa de Uptime</span
            >
            <v-avatar color="success" variant="tonal" size="36">
              <v-icon size="20">mdi-check-decagram</v-icon>
            </v-avatar>
          </div>
          <div class="text-h4 font-weight-bold my-1 text-success">
            {{ stats.uptimePercentage }}%
          </div>
          <v-progress-linear
            :model-value="stats.uptimePercentage"
            color="success"
            height="6"
            rounded
            class="mt-2"
          ></v-progress-linear>
        </v-card>
      </v-col>
    </v-row>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  isTrafficMonitor: boolean
  isGaugeMonitor: boolean
  isInterfaceMonitor: boolean
  trafficInText: string
  trafficOutText: string
  interfaceSpeedText: string
  interfaceOperText: string
  headerChip: { color: string; icon: string; label: string }
  gaugeColorValue: string
  gaugeCurrentText: string
  gaugeAvgText: string
  gaugeMinText: string
  gaugeMaxText: string
  stats: {
    uptimePercentage: number
    lastLatency: number | null
    avgLatency: number | null
    minLatency: number | null
    maxLatency: number | null
  }
  interfaceFlapCount: number
  latencyKpiTitles: {
    current: string
    currentCaption: string
    avg: string
    avgCaption: string
    minMax: string
    minMaxCaption: string
  }
  lastLatencyText: string
  avgLatencyText: string
  minLatencyText: string
  maxLatencyText: string
}>()
</script>
