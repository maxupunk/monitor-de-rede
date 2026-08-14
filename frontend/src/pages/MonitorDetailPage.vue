<template>
  <div>
    <!-- Botão de Voltar -->
    <v-btn variant="text" prepend-icon="mdi-arrow-left" class="mb-4" to="/monitors">
      Voltar para Monitores
    </v-btn>

    <!-- Loading State -->
    <v-card
      v-if="monitorsStore.loading && !monitorsStore.currentMonitor"
      elevation="2"
      class="pa-8 text-center rounded-lg"
    >
      <v-progress-circular indeterminate color="primary" size="48"></v-progress-circular>
      <div class="mt-4 text-subtitle-1 text-grey">
        Carregando métricas e histórico do monitor...
      </div>
    </v-card>

    <div v-else-if="monitorsStore.currentMonitor">
      <!-- Header do Monitor -->
      <v-card elevation="2" class="rounded-lg pa-4 pa-md-6 mb-6">
        <div
          class="d-flex flex-column flex-md-row align-start align-md-center justify-space-between ga-4"
        >
          <div class="d-flex align-center ga-3">
            <v-avatar :color="headerChip.color" size="48" size-md="56" class="text-white mr-2">
              <v-icon size="28" size-md="32">{{ typeIcon }}</v-icon>
            </v-avatar>
            <div>
              <div class="d-flex align-center ga-2 flex-wrap">
                <h1 class="text-h6 text-md-h4 font-weight-bold">{{ monitor.name }}</h1>
                <v-chip
                  :color="headerChip.color"
                  size="small"
                  variant="flat"
                  class="font-weight-bold px-3"
                >
                  <v-icon start size="14">{{ headerChip.icon }}</v-icon>
                  {{ headerChip.label }}
                </v-chip>
                <v-chip size="small" color="info" variant="tonal" class="px-3">
                  {{ typeText }}
                </v-chip>
              </div>
              <div class="text-caption text-md-subtitle-1 text-grey-darken-1 mt-1 text-break">
                Alvo: <strong class="text-high-emphasis">{{ formattedTarget }}</strong> · Intervalo:
                {{ monitor.intervalSeconds }}s
                <span v-if="monitor.device">
                  · Dispositivo: <strong>{{ monitor.device.name }}</strong></span
                >
              </div>
            </div>
          </div>

          <div class="d-flex flex-wrap align-center ga-2 w-100 w-md-auto">
            <v-btn
              v-if="monitor.device"
              variant="tonal"
              prepend-icon="mdi-router-network"
              size="small"
              class="flex-grow-1 flex-md-grow-0"
              :to="{ name: 'device-detail', params: { id: monitor.device.id } }"
            >
              <span class="hidden-sm-and-down">Ver dispositivo</span>
              <span class="hidden-md-and-up">Dispositivo</span>
            </v-btn>
            <v-btn
              color="primary"
              prepend-icon="mdi-play"
              size="small"
              class="flex-grow-1 flex-md-grow-0"
              :loading="monitorsStore.runningId === monitor.id"
              @click="monitorsStore.runMonitor(monitor.id)"
            >
              <span class="hidden-sm-and-down">Testar Agora</span>
              <span class="hidden-md-and-up">Testar</span>
            </v-btn>
            <v-btn
              variant="outlined"
              color="primary"
              prepend-icon="mdi-pencil"
              size="small"
              class="flex-grow-1 flex-md-grow-0"
              @click="editDialog = true"
            >
              Editar
            </v-btn>
            <v-btn
              :color="monitor.isEnabled ? 'warning' : 'success'"
              variant="outlined"
              size="small"
              class="flex-grow-1 flex-md-grow-0"
              :prepend-icon="monitor.isEnabled ? 'mdi-pause' : 'mdi-play-outline'"
              @click="monitorsStore.toggleMonitorEnabled(monitor.id, !monitor.isEnabled)"
            >
              {{ monitor.isEnabled ? 'Pausar' : 'Ativar' }}
            </v-btn>
            <v-btn icon color="error" variant="text" size="small" @click="confirmDelete">
              <v-icon>mdi-delete</v-icon>
              <v-tooltip activator="parent" location="top">Excluir Monitor</v-tooltip>
            </v-btn>
          </div>
        </div>
      </v-card>

      <!-- Cards de Métricas KPI: variam conforme o tipo de monitor (Tráfego, CPU/Memória, Interface RFC 2863 ou Ping/HTTP/TCP/DNS) -->
      <v-row v-if="isTrafficMonitor" class="mb-6">
        <v-col cols="12" sm="6" md="3">
          <v-card elevation="2" class="rounded-lg pa-4 h-100">
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
          <v-card elevation="2" class="rounded-lg pa-4 h-100">
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
          <v-card elevation="2" class="rounded-lg pa-4 h-100">
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
          <v-card elevation="2" class="rounded-lg pa-4 h-100">
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

      <v-row v-else-if="isGaugeMonitor" class="mb-6">
        <v-col cols="12" sm="6" md="3">
          <v-card elevation="2" class="rounded-lg pa-4 h-100">
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
          <v-card elevation="2" class="rounded-lg pa-4 h-100">
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
          <v-card elevation="2" class="rounded-lg pa-4 h-100">
            <div class="d-flex align-center justify-space-between mb-2">
              <span class="text-subtitle-2 text-grey-darken-1 font-weight-medium"
              >Uso Mín / Máx</span
              >
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
          <v-card elevation="2" class="rounded-lg pa-4 h-100">
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

      <v-row v-else-if="isInterfaceMonitor" class="mb-6">
        <v-col cols="12" sm="6" md="3">
          <v-card elevation="2" class="rounded-lg pa-4 h-100">
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
          <v-card elevation="2" class="rounded-lg pa-4 h-100">
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
          <v-card elevation="2" class="rounded-lg pa-4 h-100">
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
          <v-card elevation="2" class="rounded-lg pa-4 h-100">
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

      <v-row v-else class="mb-6">
        <!-- Tempo de Resposta / Ping / Tempo de Consulta Atual -->
        <v-col cols="12" sm="6" md="3">
          <v-card elevation="2" class="rounded-lg pa-4 h-100">
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

        <!-- Tempo Médio -->
        <v-col cols="12" sm="6" md="3">
          <v-card elevation="2" class="rounded-lg pa-4 h-100">
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

        <!-- Tempo Mínimo / Máximo -->
        <v-col cols="12" sm="6" md="3">
          <v-card elevation="2" class="rounded-lg pa-4 h-100">
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

        <!-- Uptime / Taxa de Sucesso -->
        <v-col cols="12" sm="6" md="3">
          <v-card elevation="2" class="rounded-lg pa-4 h-100">
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

      <!-- Gráfico de Tráfego de Rede (IN/OUT bps) -->
      <v-card v-if="isTrafficMonitor" elevation="2" class="rounded-lg pa-6 mb-6">
        <div class="d-flex align-center justify-space-between mb-4 flex-wrap ga-3">
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
            v-model="trafficTab"
            color="primary"
            variant="outlined"
            mandatory
            density="compact"
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
      <!-- Monitores de uso e tráfego não têm uma noção de up/down por execução, então a timeline não se aplica -->
      <v-card
        v-if="!isGaugeMonitor && !isTrafficMonitor"
        elevation="2"
        class="rounded-lg pa-6 mb-6"
      >
        <div class="d-flex align-center justify-space-between mb-4 flex-wrap ga-2">
          <div>
            <h2 class="text-h6 font-weight-bold d-flex align-center ga-2">
              <v-icon color="primary">mdi-chart-timeline-variant</v-icon>
              Linha do Tempo de Status
            </h2>
            <div class="text-subtitle-2 text-grey">Histórico recente de verificações de status</div>
          </div>
          <div class="d-flex align-center ga-4 text-caption flex-wrap">
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
            <span class="text-grey font-weight-bold">Total: {{ stats.totalChecks }}</span>
          </div>
        </div>

        <div
          class="pa-3 bg-grey-lighten-4 rounded-lg monitor-timeline-scroll d-flex justify-center"
        >
          <MonitorTimelineBar
            :results="monitor.recentResults"
            :max-blocks="60"
            :height="36"
            :width="10"
          />
        </div>
      </v-card>

      <!-- Gráfico de Uso ao Longo do Tempo (CPU/Memória) -->
      <v-card v-if="isGaugeMonitor && !isTrafficMonitor" elevation="2" class="rounded-lg pa-6 mb-6">
        <div class="d-flex align-center justify-space-between mb-4">
          <div>
            <h2 class="text-h6 font-weight-bold d-flex align-center ga-2">
              <v-icon color="info">mdi-sine-wave</v-icon>
              Gráfico de Uso de {{ gaugeTypeLabel(monitor) === 'MEMÓRIA' ? 'Memória' : 'CPU' }}
            </h2>
            <div class="text-subtitle-2 text-grey">
              Percentual de uso coletado via SNMP no dispositivo ao longo do tempo
            </div>
          </div>
          <v-chip v-if="gaugeStats.avg !== null" color="info" size="small" variant="outlined">
            Média: {{ gaugeStats.avg }}%
          </v-chip>
        </div>

        <BaseMetricChart
          v-if="gaugeSeries.length > 0 && gaugeSeries[0].data.length > 0"
          :series="gaugeSeries"
          unit-type="percentage"
          :show-avg-line="true"
          :avg-value="gaugeStats.avg ?? undefined"
        />

        <div v-else class="text-center text-grey py-8 border rounded-lg bg-grey-lighten-5">
          <v-icon size="40" color="grey-lighten-1">mdi-chart-line-variant</v-icon>
          <div class="mt-2 text-subtitle-2">
            Histórico insuficiente para gerar o gráfico de uso.
          </div>
          <div class="text-caption">
            As amostras vêm da varredura SNMP periódica do dispositivo.
          </div>
        </div>
      </v-card>

      <!-- Gráfico Unificado de Latência / Tempo de Resposta (não se aplica a monitores de uso, tráfego ou de interface) -->
      <v-card
        v-if="!isGaugeMonitor && !isInterfaceMonitor && !isTrafficMonitor"
        elevation="2"
        class="rounded-lg pa-6 mb-6"
      >
        <div class="d-flex align-center justify-space-between mb-4">
          <div>
            <h2 class="text-h6 font-weight-bold d-flex align-center ga-2">
              <v-icon color="info">mdi-sine-wave</v-icon>
              Gráfico de Tempo de Resposta (Ping Latency)
            </h2>
            <div class="text-subtitle-2 text-grey">
              Variação da latência em milissegundos (ms) ao longo do tempo
            </div>
          </div>
          <v-chip v-if="stats.avgLatency" color="info" size="small" variant="outlined">
            Média: {{ formatLatency(stats.avgLatency) }}
          </v-chip>
        </div>

        <!-- Renderização do Gráfico Unificado BaseMetricChart -->
        <BaseMetricChart
          v-if="latencySeries.length > 0 && latencySeries[0].data.length > 0"
          :series="latencySeries"
          unit-type="latency"
          :show-avg-line="true"
          :avg-value="stats.avgLatency || undefined"
        />

        <div v-else class="text-center text-grey py-8 border rounded-lg bg-grey-lighten-5">
          <v-icon size="40" color="grey-lighten-1">mdi-chart-line-variant</v-icon>
          <div class="mt-2 text-subtitle-2">
            Histórico insuficiente para gerar o gráfico de latência.
          </div>
          <div class="text-caption">Execute mais verificações clicando em "Testar Agora".</div>
        </div>
      </v-card>

      <!-- Tabela com Histórico de Verificações Recentes -->
      <v-card elevation="2" class="rounded-lg pa-6">
        <div class="d-flex align-center justify-space-between mb-4">
          <div>
            <h2 class="text-h6 font-weight-bold d-flex align-center ga-2">
              <v-icon color="primary">mdi-history</v-icon>
              Histórico de Execuções Recentes
            </h2>
            <div class="text-subtitle-2 text-grey">Resultados das últimas verificações</div>
          </div>
          <div class="d-flex align-center ga-2">
            <v-btn
              size="small"
              variant="text"
              prepend-icon="mdi-refresh"
              :loading="monitorsStore.loading"
              @click="refreshData"
            >
              Atualizar
            </v-btn>
            <v-btn icon size="small" variant="text" @click="toggleShowHistory">
              <v-icon>{{ showHistory ? 'mdi-chevron-up' : 'mdi-chevron-down' }}</v-icon>
              <v-tooltip activator="parent" location="top">
                {{ showHistory ? 'Ocultar Histórico' : 'Mostrar Histórico' }}
              </v-tooltip>
            </v-btn>
          </div>
        </div>

        <v-expand-transition>
          <div v-if="showHistory">
            <div
              class="history-scroll-container rounded-lg border overflow-y-auto"
              style="max-height: 450px"
            >
              <v-infinite-scroll :key="history.scrollKey.value" :height="420" @load="history.load">
                <div class="table-responsive">
                  <v-table density="comfortable" hover>
                    <thead>
                      <tr>
                        <th style="width: 110px">Status</th>
                        <th style="width: 140px">Latência (Ping)</th>
                        <th style="width: 120px">Duração</th>
                        <th style="width: 180px">Data e Hora</th>
                        <th>Mensagem</th>
                      </tr>
                    </thead>
                    <tbody>
                      <tr v-for="item in history.items.value" :key="item.id">
                        <td>
                          <v-chip
                            :color="getStatusColor(item.status)"
                            size="x-small"
                            variant="flat"
                          >
                            {{ item.status ? item.status.toUpperCase() : 'UNKNOWN' }}
                          </v-chip>
                        </td>
                        <td>
                          <span
                            v-if="item.latencyMs !== undefined && item.latencyMs !== null"
                            class="font-weight-medium"
                          >
                            {{ formatLatency(item.latencyMs) }}
                          </span>
                          <span v-else class="text-grey">-</span>
                        </td>
                        <td>
                          <span class="text-grey">{{ item.durationMs }} ms</span>
                        </td>
                        <td>
                          <span>{{ formatDateTime(item.finishedAt, '-') }}</span>
                        </td>
                        <td>
                          <span
                            :class="
                              item.status === 'down'
                                ? 'text-error font-weight-medium'
                                : 'text-body-2'
                            "
                          >
                            {{ item.message || '-' }}
                          </span>
                        </td>
                      </tr>
                    </tbody>
                  </v-table>
                </div>
                <template #empty>
                  <div class="text-caption text-grey text-center py-3">
                    Nenhum outro registro no histórico.
                  </div>
                </template>
              </v-infinite-scroll>
            </div>
          </div>
        </v-expand-transition>
      </v-card>

      <!-- Tabela com Histórico de Alertas -->
      <v-card elevation="2" class="rounded-lg pa-6 mt-6">
        <div class="d-flex align-center justify-space-between mb-4">
          <div>
            <h2 class="text-h6 font-weight-bold d-flex align-center ga-2">
              <v-icon color="warning">mdi-bell-alert</v-icon>
              Histórico de Alertas
            </h2>
            <div class="text-subtitle-2 text-grey">
              Alertas disparados e normalizações deste monitor
            </div>
          </div>
          <div class="d-flex align-center ga-2">
            <v-btn
              size="small"
              variant="text"
              prepend-icon="mdi-refresh"
              :loading="monitorsStore.loading"
              @click="refreshData"
            >
              Atualizar
            </v-btn>
            <v-btn icon size="small" variant="text" @click="toggleShowAlerts">
              <v-icon>{{ showAlerts ? 'mdi-chevron-up' : 'mdi-chevron-down' }}</v-icon>
              <v-tooltip activator="parent" location="top">
                {{ showAlerts ? 'Ocultar Alertas' : 'Mostrar Alertas' }}
              </v-tooltip>
            </v-btn>
          </div>
        </div>

        <v-expand-transition>
          <div v-if="showAlerts">
            <div
              class="history-scroll-container rounded-lg border overflow-y-auto"
              style="max-height: 450px"
            >
              <v-infinite-scroll
                :key="alertHistory.scrollKey.value"
                :height="420"
                @load="alertHistory.load"
              >
                <div class="table-responsive">
                  <v-table density="comfortable" hover>
                    <thead>
                      <tr>
                        <th style="width: 120px">Severidade</th>
                        <th style="width: 120px">Status</th>
                        <th>Título / Regra</th>
                        <th>Mensagem</th>
                        <th style="width: 180px">Início</th>
                        <th style="width: 180px">Normalizado em</th>
                        <th style="width: 160px">Ações</th>
                      </tr>
                    </thead>
                    <tbody>
                      <tr v-for="item in alertHistory.items.value" :key="item.id">
                        <td>
                          <v-chip
                            :color="severityColor(item.severity)"
                            size="x-small"
                            variant="flat"
                          >
                            {{ severityLabel(item.severity).toUpperCase() }}
                          </v-chip>
                        </td>
                        <td>
                          <v-chip
                            :color="
                              item.status === 'resolved' ? 'success' : severityColor(item.severity)
                            "
                            size="x-small"
                            variant="tonal"
                          >
                            {{ statusLabel(item.status).toUpperCase() }}
                          </v-chip>
                        </td>
                        <td class="font-weight-medium">
                          {{ item.title || '—' }}
                        </td>
                        <td>
                          <span
                            :class="
                              item.status === 'active'
                                ? 'text-error font-weight-medium'
                                : 'text-body-2'
                            "
                          >
                            {{ item.message || '—' }}
                          </span>
                        </td>
                        <td>
                          {{ formatDateTime(item.startedAt, '—') }}
                        </td>
                        <td>
                          <span v-if="item.resolvedAt" class="text-success font-weight-medium">
                            {{ formatDateTime(item.resolvedAt, '—') }}
                          </span>
                          <span v-else class="text-grey">—</span>
                        </td>
                        <td>
                          <div v-if="item.status === 'active'" class="d-flex ga-1">
                            <v-btn
                              size="x-small"
                              variant="text"
                              prepend-icon="mdi-check-circle"
                              color="success"
                              :loading="alertsStore.loading"
                              @click="acknowledgeAlertItem(item)"
                            >
                              Reconhecer
                            </v-btn>
                            <v-menu location="bottom end">
                              <template #activator="{ props: menuProps }">
                                <v-btn
                                  size="x-small"
                                  variant="text"
                                  prepend-icon="mdi-bell-off"
                                  color="warning"
                                  v-bind="menuProps"
                                >
                                  Silenciar
                                </v-btn>
                              </template>
                              <v-list density="compact">
                                <v-list-item
                                  v-for="duration in silenceDurations"
                                  :key="duration.minutes"
                                  :title="duration.label"
                                  :disabled="alertsStore.loading"
                                  @click="silenceAlertItem(item, duration.minutes)"
                                ></v-list-item>
                              </v-list>
                            </v-menu>
                          </div>
                          <v-chip
                            v-else-if="item.status === 'acknowledged'"
                            size="x-small"
                            color="info"
                            variant="tonal"
                            prepend-icon="mdi-check-circle"
                          >
                            Reconhecido
                          </v-chip>
                          <v-chip
                            v-else-if="item.status === 'silenced'"
                            size="x-small"
                            color="warning"
                            variant="tonal"
                            prepend-icon="mdi-bell-off"
                          >
                            Silenciado
                          </v-chip>
                          <span v-else class="text-grey">—</span>
                        </td>
                      </tr>
                    </tbody>
                  </v-table>
                </div>
                <template #empty>
                  <div class="text-caption text-grey text-center py-3">
                    Nenhum outro registro no histórico de alertas.
                  </div>
                </template>
              </v-infinite-scroll>
            </div>
          </div>
        </v-expand-transition>
      </v-card>
    </div>

    <!-- State de Erro / Não Encontrado -->
    <v-card v-else elevation="2" class="pa-8 text-center rounded-lg">
      <v-icon size="64" color="error" class="mb-4">mdi-alert-circle-outline</v-icon>
      <div class="text-h6 text-error">Monitor não encontrado</div>
      <div class="text-body-2 text-grey mt-1">O monitor solicitado não existe ou foi removido.</div>
      <v-btn color="primary" class="mt-4" to="/monitors">Voltar para Monitores</v-btn>
    </v-card>

    <!-- Dialog de Edição do Monitor -->
    <MonitorFormDialog
      v-model="editDialog"
      :monitor="monitorsStore.currentMonitor"
      @saved="refreshData"
    ></MonitorFormDialog>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useMonitorsStore, type Monitor, type MonitorResult } from '@/stores/monitors'
import { useEventsStore } from '@/stores/events'
import { useAlertsStore } from '@/stores/alerts'
import { apiService } from '@/services/apiService'
import { useInfiniteList } from '@/composables/useInfiniteList'
import type { DeviceMetric } from '@/stores/deviceDetail'
import MonitorTimelineBar from '@/components/MonitorTimelineBar.vue'
import BaseMetricChart, { type ChartSeriesInput } from '@/components/BaseMetricChart.vue'
import MonitorFormDialog from '@/components/MonitorFormDialog.vue'
import {
  isGaugeMonitor as isGaugeMonitorFn,
  isTrafficMonitor as isTrafficMonitorFn,
  gaugeMetricName,
  gaugeTypeLabel,
  gaugeColor as gaugeColorFn,
  isInterfaceMonitor as isInterfaceMonitorFn,
  interfaceStatusInfo,
  latestResultData,
  getStatusColor,
} from '@/utils/monitorPresentation'
import { severityColor, severityLabel, statusLabel } from '@/utils/alertPresentation'
import { formatDateTime, formatLatency, formatBps } from '@/utils/formatters'
import type { AlertEvent } from '@/stores/alerts'

const route = useRoute()
const router = useRouter()
const monitorsStore = useMonitorsStore()
const eventsStore = useEventsStore()
const alertsStore = useAlertsStore()

const monitorId = computed(() => Number(route.params.id))
const editDialog = ref(false)

const emptyMonitor: Monitor = {
  id: 0,
  deviceId: 0,
  name: '',
  type: 'ping',
  target: '',
  port: undefined,
  configuration: {},
  intervalSeconds: 60,
  status: 'unknown',
  isEnabled: true,
  device: undefined,
  recentResults: [],
  stats: undefined,
  gaugeMetric: null,
}

const monitor = computed<Monitor>(() => monitorsStore.currentMonitor || emptyMonitor)

const stats = computed(
  () =>
    monitor.value.stats || {
      avgLatency: null,
      minLatency: null,
      maxLatency: null,
      lastLatency: null,
      uptimePercentage: 100,
      totalChecks: 0,
      upChecks: 0,
    }
)

const showHistory = ref(false)
const history = useInfiniteList<MonitorResult>(() => `/monitors/${monitorId.value}/results`, {
  label: 'histórico de verificações',
})

const showAlerts = ref(false)
const alertHistory = useInfiniteList<AlertEvent>(() => `/monitors/${monitorId.value}/alerts`, {
  label: 'histórico de alertas',
})

const silenceDurations = [
  { minutes: 30, label: '30 minutos' },
  { minutes: 60, label: '1 hora' },
  { minutes: 240, label: '4 horas' },
  { minutes: 1440, label: '24 horas' },
]

function toggleShowHistory() {
  showHistory.value = !showHistory.value
  // Reabrir o card recomeça da primeira página: o histórico pode ter crescido
  // enquanto ele estava recolhido.
  if (showHistory.value) history.reset()
}

function toggleShowAlerts() {
  showAlerts.value = !showAlerts.value
  if (showAlerts.value) alertHistory.reset()
}

const formattedTarget = computed(() => {
  if (monitor.value.port) {
    return `${monitor.value.target}:${monitor.value.port}`
  }
  return monitor.value.target
})

const statusText = computed(() => (monitor.value.status || 'UNKNOWN').toUpperCase())

const isTrafficMonitor = computed(() => isTrafficMonitorFn(monitor.value))
const isGaugeMonitor = computed(() => isGaugeMonitorFn(monitor.value) && !isTrafficMonitor.value)
const isInterfaceMonitor = computed(() => isInterfaceMonitorFn(monitor.value))

const typeText = computed(() => {
  if (isTrafficMonitor.value) return 'TRÁFEGO'
  if (isGaugeMonitor.value) return gaugeTypeLabel(monitor.value)
  if (isInterfaceMonitor.value) return 'INTERFACE'
  return (monitor.value.type || 'PING').toUpperCase()
})

const gaugeColorValue = computed(() =>
  gaugeColorFn(monitor.value.gaugeMetric?.value ?? null, gaugeMetricName(monitor.value))
)

// Unifica a apresentação do status no header: tráfego mostra taxa, gauge mostra %,
// interface mostra o estado real e monitor tradicional mostra status textual.
const headerChip = computed(() => {
  if (isTrafficMonitor.value) {
    const info = interfaceStatusInfo(
      monitor.value.status,
      latestResultData(monitor.value.recentResults)
    )
    return {
      label: trafficInText.value !== 'N/D' ? `↓ ${trafficInText.value}` : info.label.toUpperCase(),
      color: info.color,
      icon: 'mdi-swap-vertical-bold',
    }
  }
  if (isGaugeMonitor.value) {
    const val = monitor.value.gaugeMetric?.value
    return {
      label: val !== null && val !== undefined ? `${Math.round(val)}%` : 'SEM DADOS',
      color: gaugeColorValue.value,
      icon: 'mdi-gauge',
    }
  }
  if (isInterfaceMonitor.value) {
    const info = interfaceStatusInfo(
      monitor.value.status,
      latestResultData(monitor.value.recentResults)
    )
    return { label: info.label.toUpperCase(), color: info.color, icon: info.icon }
  }
  return {
    label: statusText.value,
    color: getStatusColor(monitor.value.status),
    icon: 'mdi-circle',
  }
})

const typeIcon = computed(() => {
  if (isTrafficMonitor.value) return 'mdi-swap-vertical-bold'
  if (isGaugeMonitor.value) {
    return gaugeMetricName(monitor.value) === 'memory_usage' ? 'mdi-memory' : 'mdi-chip'
  }
  if (isInterfaceMonitor.value) return headerChip.value.icon
  switch (monitor.value.type) {
    case 'http':
    case 'https':
      return 'mdi-web'
    case 'tcp':
      return 'mdi-ethernet-cable'
    case 'dns':
      return 'mdi-dns'
    default:
      return 'mdi-ping'
  }
})

const lastLatencyText = computed(() => formatLatency(stats.value.lastLatency))
const avgLatencyText = computed(() => formatLatency(stats.value.avgLatency))
const minLatencyText = computed(() => formatLatency(stats.value.minLatency))
const maxLatencyText = computed(() => {
  return stats.value.maxLatency !== null ? `${stats.value.maxLatency}ms` : 'N/A'
})

const latencyKpiTitles = computed(() => {
  if (monitor.value?.type === 'ping') {
    return {
      current: 'Ping Atual',
      avg: 'Ping Médio',
      minMax: 'Ping Mín / Máx',
      currentCaption: 'Última resposta registrada',
      avgCaption: 'Média das verificações recentes',
      minMaxCaption: 'Mínima e máxima de latência',
    }
  }
  if (monitor.value?.type === 'dns') {
    return {
      current: 'Tempo de Consulta Atual',
      avg: 'Tempo de Consulta Médio',
      minMax: 'Consulta Mín / Máx',
      currentCaption: 'Última consulta DNS registrada',
      avgCaption: 'Média dos tempos de resolução',
      minMaxCaption: 'Mínimo e máximo tempo de consulta',
    }
  }
  return {
    current: 'Tempo de Resposta Atual',
    avg: 'Tempo de Resposta Médio',
    minMax: 'Resposta Mín / Máx',
    currentCaption: 'Última resposta registrada',
    avgCaption: 'Média das verificações recentes',
    minMaxCaption: 'Mínimo e máximo tempo de resposta',
  }
})

// --- Monitores de Interface / Tráfego ---
const interfaceLatestData = computed(() => latestResultData(monitor.value.recentResults))

const interfaceSpeedText = computed(() => {
  const data = interfaceLatestData.value
  return (data?.speedFormatted as string | undefined) || headerChip.value.label
})

const interfaceOperText = computed(() => {
  const data = interfaceLatestData.value
  return (data?.operStatusText as string | undefined) || headerChip.value.label
})

const interfaceFlapCount = computed(() => {
  const results = monitor.value.recentResults || []
  let flaps = 0
  for (let i = 1; i < results.length; i++) {
    if (results[i].status !== results[i - 1].status) flaps++
  }
  return flaps
})

const statusBreakdown = computed(() => {
  const results = monitor.value.recentResults || []
  const counts = { up: 0, down: 0, warning: 0, disabled: 0, unknown: 0 }
  for (const r of results) {
    const key = (r.status && r.status in counts ? r.status : 'unknown') as keyof typeof counts
    counts[key]++
  }
  return counts
})

// --- Histórico de Métricas (CPU/Memória/Tráfego de Interface) ---
const gaugeHistory = ref<DeviceMetric[]>([])

const trafficTab = ref<'inBps' | 'outBps' | 'combined'>('combined')

const trafficMetrics = computed(() => {
  const ifName = monitor.value.configuration?.ifName as string | undefined
  return gaugeHistory.value
    .filter((m) => {
      if (m.metricName !== 'inBps' && m.metricName !== 'outBps') return false
      if (ifName && m.interfaceName) {
        return m.interfaceName.toLowerCase() === ifName.toLowerCase()
      }
      return true
    })
    .slice()
    .sort((a, b) => new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime())
})

const latestInBps = computed(() => {
  const list = trafficMetrics.value.filter((m) => m.metricName === 'inBps')
  if (list.length === 0) return null
  return Number(list[list.length - 1].metricValue)
})

const latestOutBps = computed(() => {
  const list = trafficMetrics.value.filter((m) => m.metricName === 'outBps')
  if (list.length === 0) return null
  return Number(list[list.length - 1].metricValue)
})

const trafficInText = computed(() => {
  if (latestInBps.value !== null && Number.isFinite(latestInBps.value)) {
    return formatBps(latestInBps.value)
  }
  if (monitor.value.gaugeMetric?.value) {
    return formatBps(monitor.value.gaugeMetric.value)
  }
  return 'N/D'
})

const trafficOutText = computed(() => {
  if (latestOutBps.value !== null && Number.isFinite(latestOutBps.value)) {
    return formatBps(latestOutBps.value)
  }
  return 'N/D'
})

const trafficSeries = computed<ChartSeriesInput[]>(() => {
  const inList = trafficMetrics.value.filter((m) => m.metricName === 'inBps')
  const outList = trafficMetrics.value.filter((m) => m.metricName === 'outBps')

  if (trafficTab.value === 'combined') {
    const series: ChartSeriesInput[] = []
    if (inList.length > 0) {
      series.push({
        id: 'inBps',
        label: 'Download (IN)',
        color: '#4CAF50',
        fillArea: true,
        data: inList.map((m) => {
          const val = Number(m.metricValue) || 0
          return {
            time: formatDateTime(m.createdAt, '-'),
            value: val,
            formattedValue: formatBps(val),
          }
        }),
      })
    }
    if (outList.length > 0) {
      series.push({
        id: 'outBps',
        label: 'Upload (OUT)',
        color: '#2196F3',
        fillArea: false,
        data: outList.map((m) => {
          const val = Number(m.metricValue) || 0
          return {
            time: formatDateTime(m.createdAt, '-'),
            value: val,
            formattedValue: formatBps(val),
          }
        }),
      })
    }
    return series
  }

  const isDownload = trafficTab.value === 'inBps'
  const targetList = isDownload ? inList : outList
  if (targetList.length === 0) return []

  return [
    {
      id: trafficTab.value,
      label: isDownload ? 'Download (IN)' : 'Upload (OUT)',
      color: isDownload ? '#4CAF50' : '#2196F3',
      fillArea: true,
      data: targetList.map((m) => {
        const val = Number(m.metricValue) || 0
        return {
          time: formatDateTime(m.createdAt, '-'),
          value: val,
          formattedValue: formatBps(val),
        }
      }),
    },
  ]
})

const gaugeHistoryFiltered = computed(() => {
  const name = gaugeMetricName(monitor.value)
  return gaugeHistory.value
    .filter((m) => m.metricName === name)
    .slice()
    .sort((a, b) => new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime())
})

const gaugeStats = computed(() => {
  const values = gaugeHistoryFiltered.value
    .map((m) => Number(m.metricValue))
    .filter((v) => !isNaN(v))
  const current =
    monitor.value.gaugeMetric?.value ?? (values.length > 0 ? values[values.length - 1] : null)
  if (values.length === 0) {
    return {
      current,
      avg: null as number | null,
      min: null as number | null,
      max: null as number | null,
    }
  }
  const avg = Number((values.reduce((a, b) => a + b, 0) / values.length).toFixed(1))
  return { current, avg, min: Math.min(...values), max: Math.max(...values) }
})

const gaugeCurrentText = computed(() => {
  const v = gaugeStats.value.current
  return v !== null && v !== undefined ? `${Number(v).toFixed(1)}%` : 'N/A'
})
const gaugeAvgText = computed(() =>
  gaugeStats.value.avg !== null ? `${gaugeStats.value.avg}%` : 'N/A'
)
const gaugeMinText = computed(() =>
  gaugeStats.value.min !== null ? `${gaugeStats.value.min!.toFixed(1)}%` : 'N/A'
)
const gaugeMaxText = computed(() =>
  gaugeStats.value.max !== null ? `${gaugeStats.value.max!.toFixed(1)}%` : 'N/A'
)

const gaugeSeries = computed<ChartSeriesInput[]>(() => {
  const list = gaugeHistoryFiltered.value
  if (list.length === 0) return []
  const name = gaugeMetricName(monitor.value)
  return [
    {
      id: name,
      label: name === 'memory_usage' ? 'Uso de Memória' : 'Uso de CPU',
      color: name === 'memory_usage' ? '#9C27B0' : '#2196F3',
      fillArea: true,
      data: list.map((m) => {
        const val = Number(m.metricValue) || 0
        return {
          time: formatDateTime(m.createdAt, '-'),
          value: val,
          formattedValue: `${val.toFixed(1)}%`,
        }
      }),
    },
  ]
})

async function loadGaugeHistory() {
  if (!monitor.value.deviceId) {
    gaugeHistory.value = []
    return
  }
  try {
    gaugeHistory.value = await apiService.get<DeviceMetric[]>(
      `/devices/${monitor.value.deviceId}/metrics`
    )
  } catch {
    gaugeHistory.value = []
  }
}

/**
 * O histórico de uso e tráfego é estado local desta tela, então ela mesma
 * assina o evento de coleta SNMP para acompanhar as novas amostras ao vivo.
 * Os demais dados (timeline, latência, KPIs) vêm patchados pela store.
 */
let stopMetricsListener: (() => void) | null = null

onMounted(async () => {
  if (!monitorId.value) return

  await monitorsStore.fetchMonitorById(monitorId.value)
  if (isGaugeMonitor.value || isTrafficMonitor.value) await loadGaugeHistory()

  stopMetricsListener = eventsStore.onEvent('metric:recorded', (data) => {
    if (!isGaugeMonitor.value && !isTrafficMonitor.value) return
    if (Number(data.deviceId) !== monitor.value.deviceId) return

    const samples = (data.metrics as Array<Record<string, unknown>>) || []

    for (const sample of samples) {
      gaugeHistory.value.push({
        id: Date.now() + gaugeHistory.value.length,
        deviceId: monitor.value.deviceId,
        interfaceId: sample.interfaceId as number | undefined,
        interfaceName: sample.interfaceName as string | undefined,
        metricName: String(sample.name),
        metricValue: Number(sample.value),
        unit: String(sample.unit ?? ''),
        createdAt: String(sample.recordedAt ?? new Date().toISOString()),
      })
    }
  })

  eventsStore.onEvent(
    ['alert:triggered', 'alert:resolved', 'alert:acknowledged', 'alert:silenced'],
    (data) => {
      if (Number(data.monitorId) !== monitorId.value) return
      if (showAlerts.value) alertHistory.reset()
    }
  )
})

onUnmounted(() => {
  stopMetricsListener?.()
})

async function refreshData() {
  if (monitorId.value) {
    await monitorsStore.fetchMonitorById(monitorId.value)
    if (isGaugeMonitor.value || isTrafficMonitor.value) await loadGaugeHistory()
    if (showHistory.value) history.reset()
    if (showAlerts.value) alertHistory.reset()
  }
}

async function acknowledgeAlertItem(item: AlertEvent) {
  const success = await alertsStore.acknowledgeAlert(item.id)
  if (success && showAlerts.value) {
    alertHistory.reset()
  }
}

async function silenceAlertItem(item: AlertEvent, minutes: number) {
  const success = await alertsStore.silenceAlert(item.id, minutes)
  if (success && showAlerts.value) {
    alertHistory.reset()
  }
}

async function confirmDelete() {
  if (confirm('Tem certeza de que deseja excluir este monitor?')) {
    const success = await monitorsStore.deleteMonitor(monitorId.value)
    if (success) {
      router.push('/monitors')
    }
  }
}

// Estrutura unificada de dados para o componente BaseMetricChart
const latencySeries = computed<ChartSeriesInput[]>(() => {
  // `recentResults` já vem do mais antigo para o mais recente (ver stores/monitors.ts) —
  // não reverter aqui, ou o gráfico plota o tempo andando para trás.
  const results = monitor.value.recentResults || []
  if (results.length === 0) return []

  return [
    {
      id: 'latency',
      label: 'Tempo de Resposta',
      color: '#2196F3',
      fillArea: true,
      data: results.map((r) => {
        const val = r.latencyMs || 0
        const status = r.status || (val > 0 ? 'up' : 'down')
        return {
          time: formatDateTime(r.finishedAt, '-'),
          value: val,
          formattedValue: `${val} ms`,
          status,
          color: status === 'down' ? '#F44336' : '#2196F3',
        }
      }),
    },
  ]
})
</script>

<style scoped>
.status-indicator-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  display: inline-block;
}
</style>
