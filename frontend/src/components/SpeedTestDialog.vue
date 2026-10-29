<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 820"
    :fullscreen="$vuetify.display.xs"
    @update:model-value="onUpdateModelValue"
  >
    <v-card class="rounded-lg">
      <v-card-title class="d-flex align-center justify-space-between pa-4 bg-primary text-white">
        <div class="d-flex align-center ga-2" style="gap: 8px">
          <v-icon>mdi-speedometer</v-icon>
          <span>Velocidade do navegador</span>
        </div>
        <v-btn icon variant="text" color="white" @click="close">
          <v-icon>mdi-close</v-icon>
        </v-btn>
      </v-card-title>

      <v-card-text class="pa-6">
        <!-- Seletores de Configuração: Modo, Duração Estimada e Resolução/Unidade -->
        <div class="d-flex flex-wrap justify-center align-center ga-3 mb-6" style="gap: 12px">
          <!-- Seletor de Modo: WAN vs LAN -->
          <v-btn-toggle
            v-model="testMode"
            mandatory
            color="primary"
            variant="outlined"
            density="comfortable"
            rounded="pill"
            :disabled="diagnosticsStore.speedTestRunning"
          >
            <v-btn value="wan" prepend-icon="mdi-web"> Internet (WAN) </v-btn>
            <v-btn value="lan" prepend-icon="mdi-lan"> Rede Local (LAN) </v-btn>
          </v-btn-toggle>

          <!-- Seletor de Duração Estimada -->
          <v-btn-toggle
            v-model="testProfile"
            mandatory
            color="primary"
            variant="outlined"
            density="comfortable"
            rounded="pill"
            :disabled="diagnosticsStore.speedTestRunning"
          >
            <v-btn value="simple">
              <v-icon start size="16">mdi-lightning-bolt</v-icon>
              Simples (~15s)
            </v-btn>
            <v-btn value="medium">
              <v-icon start size="16">mdi-timer-sand</v-icon>
              Médio (~30s)
            </v-btn>
            <v-btn value="complete">
              <v-icon start size="16">mdi-gauge-full</v-icon>
              Completo (~60s)
            </v-btn>
          </v-btn-toggle>

          <!-- Seletor de Resolução / Unidade de Velocidade -->
          <v-btn-toggle
            v-model="speedUnit"
            mandatory
            color="primary"
            variant="outlined"
            density="comfortable"
            rounded="pill"
          >
            <v-btn value="auto"> Auto </v-btn>
            <v-btn value="kbps"> Kbps </v-btn>
            <v-btn value="mbps"> Mbps </v-btn>
            <v-btn value="gbps"> Gbps </v-btn>
          </v-btn-toggle>
        </div>

        <!-- Painel do Gráfico em Tempo Real -->
        <v-card variant="outlined" class="rounded-lg overflow-hidden mb-6 chart-card">
          <!-- HUD Superior do Gráfico -->
          <div
            class="d-flex flex-wrap align-center justify-space-between pa-4 bg-surface-light border-b ga-2"
          >
            <div class="d-flex align-baseline ga-3" style="gap: 12px">
              <span class="text-h3 font-weight-bold font-monospace text-primary">
                {{ formatSpeedByUnit(displaySpeed, effectiveChartUnit) }}
              </span>
              <span class="text-subtitle-1 font-weight-medium text-uppercase">
                {{ getSpeedUnitLabel(effectiveChartUnit) }}
              </span>
              <v-chip
                size="small"
                :color="phaseColor"
                variant="flat"
                class="font-weight-bold text-uppercase"
              >
                {{ phaseLabel }}
              </v-chip>
              <v-chip
                v-if="peakSpeed > 0"
                size="small"
                color="secondary"
                variant="tonal"
                class="font-weight-medium"
              >
                Pico: {{ formatSpeedByUnit(peakSpeed, effectiveChartUnit) }}
                {{ getSpeedUnitLabel(effectiveChartUnit) }}
              </v-chip>

              <!-- Legenda das Séries no Gráfico -->
              <div
                v-if="downloadSamples.length > 0 || uploadSamples.length > 0"
                class="d-flex align-center ga-2"
                style="gap: 8px"
              >
                <span class="d-flex align-center text-caption font-weight-medium">
                  <span
                    class="legend-bar mr-1"
                    :style="{ backgroundColor: SPEED_COLORS.download }"
                  ></span>
                  Download
                </span>
                <span class="d-flex align-center text-caption font-weight-medium">
                  <span
                    class="legend-bar mr-1"
                    :style="{ backgroundColor: SPEED_COLORS.upload }"
                  ></span>
                  Upload
                </span>
              </div>
            </div>

            <div v-if="serverLocation" class="text-caption d-flex align-center">
              <v-icon size="14" class="mr-1">mdi-map-marker-radius</v-icon>
              {{ serverName || 'Padrão' }} ({{ serverLocation }})
            </div>
          </div>

          <!-- Área do Gráfico SVG -->
          <div class="chart-wrapper pa-2 position-relative">
            <div ref="plotRef" class="speed-plot">
              <svg
                v-if="plotWidth > 0"
                :width="plotWidth"
                :height="PLOT_HEIGHT"
                :viewBox="`0 0 ${plotWidth} ${PLOT_HEIGHT}`"
                class="speed-plot__svg"
                role="img"
                aria-label="Velocidade durante o teste"
              >
                <defs>
                  <linearGradient
                    v-for="s in speedSeries"
                    :id="s.gradientId"
                    :key="s.gradientId"
                    x1="0"
                    y1="0"
                    x2="0"
                    y2="1"
                  >
                    <stop offset="0%" :stop-color="s.color" stop-opacity="0.25" />
                    <stop offset="100%" :stop-color="s.color" stop-opacity="0" />
                  </linearGradient>
                </defs>

                <g v-for="tick in speedTicks" :key="tick.y">
                  <line
                    :x1="plotArea.left"
                    :x2="plotArea.left + plotArea.width"
                    :y1="tick.y"
                    :y2="tick.y"
                    class="speed-plot__grid"
                    :class="{ 'speed-plot__grid--base': tick.base }"
                  />
                  <text
                    :x="plotArea.left - 8"
                    :y="tick.y"
                    class="speed-plot__label"
                    text-anchor="end"
                  >
                    {{ tick.label }}
                  </text>
                </g>

                <text
                  v-if="downloadSamples.length === 0 && uploadSamples.length === 0"
                  :x="plotArea.left + plotArea.width / 2"
                  :y="plotArea.top + plotArea.height / 2"
                  class="speed-plot__label"
                  text-anchor="middle"
                >
                  Selecione o perfil e clique em Iniciar Teste para ver o gráfico em tempo real
                </text>

                <template v-for="s in speedSeries" :key="s.id">
                  <path v-if="s.area" :d="s.area" :fill="`url(#${s.gradientId})`" />
                  <path :d="s.line" :stroke="s.color" class="speed-plot__line" />
                </template>

                <circle
                  v-if="activePoint"
                  :cx="activePoint.x"
                  :cy="activePoint.y"
                  r="4"
                  :fill="activePointColor"
                  class="speed-plot__marker"
                />
              </svg>
            </div>

            <!-- Barra de Progresso Integrada -->
            <v-progress-linear
              :model-value="progressPct"
              :color="phaseColor"
              height="4"
              rounded
              class="mt-1"
            />
          </div>
        </v-card>

        <v-alert
          v-if="diagnosticsStore.speedTestError"
          type="error"
          variant="tonal"
          density="compact"
          class="mb-4"
        >
          {{ diagnosticsStore.speedTestError }}
        </v-alert>

        <!-- Grade de Métricas Principais -->
        <v-row class="mb-4">
          <v-col cols="6" sm="3">
            <v-card variant="tonal" color="primary" class="rounded-lg pa-3 text-center">
              <div class="text-caption font-weight-medium">Download</div>
              <div class="text-h5 font-weight-bold font-monospace text-primary">
                {{ formatSpeedByUnit(downloadResult, speedUnit) }}
                <span class="text-caption font-weight-regular">{{
                  getSpeedUnitLabel(speedUnit, downloadResult)
                }}</span>
              </div>
            </v-card>
          </v-col>

          <v-col cols="6" sm="3">
            <v-card variant="tonal" color="purple" class="rounded-lg pa-3 text-center">
              <div class="text-caption font-weight-medium">Upload</div>
              <div class="text-h5 font-weight-bold font-monospace text-purple">
                {{ formatSpeedByUnit(uploadResult, speedUnit) }}
                <span class="text-caption font-weight-regular">{{
                  getSpeedUnitLabel(speedUnit, uploadResult)
                }}</span>
              </div>
            </v-card>
          </v-col>

          <v-col cols="6" sm="3">
            <v-card variant="tonal" color="teal" class="rounded-lg pa-3 text-center">
              <div class="text-caption font-weight-medium">Latência (Ping)</div>
              <div class="text-h5 font-weight-bold font-monospace text-teal">
                {{ formatLatency(pingResult, '--') }}
              </div>
            </v-card>
          </v-col>

          <v-col cols="6" sm="3">
            <v-card variant="tonal" color="amber-darken-3" class="rounded-lg pa-3 text-center">
              <div class="text-caption font-weight-medium">Jitter</div>
              <div class="text-h5 font-weight-bold font-monospace text-amber-darken-3">
                {{ formatLatency(jitterResult, '--') }}
              </div>
            </v-card>
          </v-col>
        </v-row>

        <!-- Botão de Ação -->
        <div class="d-flex justify-center my-4">
          <v-btn
            v-if="!diagnosticsStore.speedTestRunning"
            color="primary"
            size="large"
            rounded="pill"
            prepend-icon="mdi-play"
            class="px-8 font-weight-bold"
            @click="startTest"
          >
            Iniciar Teste
          </v-btn>
          <v-btn
            v-else
            color="error"
            size="large"
            rounded="pill"
            variant="tonal"
            prepend-icon="mdi-stop-circle-outline"
            class="px-8 font-weight-bold"
            @click="cancelTest"
          >
            Cancelar Teste
          </v-btn>
        </div>
      </v-card-text>

      <v-divider></v-divider>
      <v-card-actions class="pa-4 justify-end">
        <v-btn variant="text" @click="close">Fechar</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useDiagnosticsStore } from '@/stores/diagnostics'
import { useElementWidth } from '@/composables/useElementWidth'
import {
  areaPath,
  axisTicks,
  linePath,
  paletteColor,
  yAt,
  type PlotArea,
} from '@/utils/charts/scale'
import {
  convertSpeedFromMbps,
  formatLatency,
  formatSpeedByUnit,
  formatSpeedValue,
  getSpeedUnitLabel,
  resolveSpeedUnit,
  type SpeedUnit,
} from '@/utils/formatters'

interface SpeedSample {
  speed: number
  ratio: number
}

const props = defineProps<{
  modelValue: boolean
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
}>()

const diagnosticsStore = useDiagnosticsStore()

const testMode = ref<'wan' | 'lan'>('wan')
const testProfile = ref<'simple' | 'medium' | 'complete'>('medium')
const speedUnit = ref<SpeedUnit>('auto')
const currentPhase = ref<'idle' | 'ping' | 'download' | 'upload' | 'complete'>('idle')
const progressPct = ref(0)
const displaySpeed = ref(0)

const pingResult = ref<number | null>(null)
const jitterResult = ref<number | null>(null)
const downloadResult = ref<number | null>(null)
const uploadResult = ref<number | null>(null)
const serverName = ref<string | null>(null)
const serverLocation = ref<string | null>(null)

const downloadSamples = ref<SpeedSample[]>([])
const uploadSamples = ref<SpeedSample[]>([])

watch(
  () => props.modelValue,
  (open) => {
    if (!open && diagnosticsStore.speedTestRunning) {
      diagnosticsStore.cancelSpeedTest()
    }
  }
)

const phaseLabel = computed(() => {
  switch (currentPhase.value) {
    case 'ping':
      return 'Medindo Latência'
    case 'download':
      return 'Testando Download'
    case 'upload':
      return 'Testando Upload'
    case 'complete':
      return 'Concluído'
    default:
      return 'Pronto'
  }
})

const phaseColor = computed(() => {
  switch (currentPhase.value) {
    case 'ping':
      return 'teal'
    case 'download':
      return 'primary'
    case 'upload':
      return 'purple'
    case 'complete':
      return 'success'
    default:
      return 'grey'
  }
})

const peakSpeed = computed(() => {
  const downMax = downloadSamples.value.reduce((max, s) => Math.max(max, s.speed), 0)
  const upMax = uploadSamples.value.reduce((max, s) => Math.max(max, s.speed), 0)
  return Math.max(downMax, upMax)
})

// Unidade efetiva aplicada ao gráfico (em modo 'auto', deriva do pico/velocidade atual)
const effectiveChartUnit = computed<'kbps' | 'mbps' | 'gbps'>(() => {
  const referenceSpeed = Math.max(peakSpeed.value, displaySpeed.value)
  return resolveSpeedUnit(referenceSpeed, speedUnit.value)
})

// Escala dinâmica do eixo Y do gráfico adaptada à unidade selecionada/resolvida
const yScaleMax = computed(() => {
  const currentMaxMbps = Math.max(peakSpeed.value, displaySpeed.value)
  const maxVal = convertSpeedFromMbps(currentMaxMbps, effectiveChartUnit.value)

  if (effectiveChartUnit.value === 'kbps') {
    if (maxVal <= 250) return 250
    if (maxVal <= 500) return 500
    if (maxVal <= 1000) return 1000
    if (maxVal <= 2500) return 2500
    if (maxVal <= 5000) return 5000
    if (maxVal <= 10000) return 10000
    if (maxVal <= 25000) return 25000
    if (maxVal <= 50000) return 50000
    if (maxVal <= 100000) return 100000
    if (maxVal <= 250000) return 250000
    if (maxVal <= 500000) return 500000
    if (maxVal <= 1000000) return 1000000
    return Math.ceil(maxVal / 100000) * 100000
  }

  if (effectiveChartUnit.value === 'gbps') {
    if (maxVal <= 0.05) return 0.05
    if (maxVal <= 0.1) return 0.1
    if (maxVal <= 0.25) return 0.25
    if (maxVal <= 0.5) return 0.5
    if (maxVal <= 1.0) return 1.0
    if (maxVal <= 2.5) return 2.5
    if (maxVal <= 5.0) return 5.0
    if (maxVal <= 10.0) return 10.0
    return Math.ceil(maxVal)
  }

  // mbps
  if (maxVal <= 25) return 25
  if (maxVal <= 50) return 50
  if (maxVal <= 100) return 100
  if (maxVal <= 250) return 250
  if (maxVal <= 500) return 500
  if (maxVal <= 1000) return 1000
  return Math.ceil(maxVal / 100) * 100
})

/*
 * Gráfico em pixels reais (sem esticar a caixa): o eixo X é o progresso da
 * fase (0 → 1), não o relógio — download e upload ocupam a mesma largura.
 */
const SPEED_COLORS = { download: paletteColor(0), upload: paletteColor(6) }
const PLOT_HEIGHT = 180
const plotRef = ref<HTMLElement | null>(null)
const plotWidth = useElementWidth(plotRef)

const plotArea = computed<PlotArea>(() => ({
  left: 64,
  top: 10,
  width: Math.max(10, plotWidth.value - 64 - 12),
  height: PLOT_HEIGHT - 10 - 12,
}))

const speedDomain = computed(() => ({ min: 0, max: yScaleMax.value }))

const speedTicks = computed(() =>
  axisTicks(0, yScaleMax.value, 3).map((value) => ({
    y: yAt(value, speedDomain.value, plotArea.value),
    label: value === 0 ? '0' : formatSpeedValue(value, effectiveChartUnit.value),
    base: value === 0,
  }))
)

function calculatePoint(sample: SpeedSample) {
  const speedInUnit = convertSpeedFromMbps(sample.speed, effectiveChartUnit.value)
  return {
    x: plotArea.value.left + Math.min(1, Math.max(0, sample.ratio)) * plotArea.value.width,
    y: yAt(speedInUnit, speedDomain.value, plotArea.value),
  }
}

const downloadPoints = computed(() => downloadSamples.value.map((s) => calculatePoint(s)))
const uploadPoints = computed(() => uploadSamples.value.map((s) => calculatePoint(s)))

const speedSeries = computed(() => {
  const baseY = plotArea.value.top + plotArea.value.height
  return [
    { id: 'download', points: downloadPoints.value, color: SPEED_COLORS.download },
    { id: 'upload', points: uploadPoints.value, color: SPEED_COLORS.upload },
  ]
    .filter((s) => s.points.length > 0)
    .map((s) => ({
      id: s.id,
      color: s.color,
      gradientId: `speed-grad-${s.id}`,
      line: linePath(s.points),
      area: areaPath(s.points, baseY),
    }))
})

const activePoint = computed(() => {
  if (currentPhase.value === 'download' && downloadPoints.value.length > 0) {
    return downloadPoints.value[downloadPoints.value.length - 1]
  }
  if (currentPhase.value === 'upload' && uploadPoints.value.length > 0) {
    return uploadPoints.value[uploadPoints.value.length - 1]
  }
  if (currentPhase.value === 'complete' && uploadPoints.value.length > 0) {
    return uploadPoints.value[uploadPoints.value.length - 1]
  }
  return null
})

const activePointColor = computed(() =>
  currentPhase.value === 'upload' ? SPEED_COLORS.upload : SPEED_COLORS.download
)

function onUpdateModelValue(val: boolean) {
  emit('update:modelValue', val)
}

function close() {
  emit('update:modelValue', false)
}

function resetResults() {
  displaySpeed.value = 0
  progressPct.value = 0
  pingResult.value = null
  jitterResult.value = null
  downloadResult.value = null
  uploadResult.value = null
  serverName.value = null
  serverLocation.value = null
  currentPhase.value = 'idle'
  downloadSamples.value = []
  uploadSamples.value = []
}

async function startTest() {
  resetResults()

  if (testMode.value === 'wan') {
    await diagnosticsStore.runSpeedTest(
      (progress) => {
        progressPct.value = progress.progressPct
        if (progress.phase === 'ping') {
          currentPhase.value = 'ping'
          if (progress.pingMs) pingResult.value = progress.pingMs
        } else if (progress.phase === 'download') {
          currentPhase.value = 'download'
          if (progress.currentMbps !== undefined && progress.currentMbps !== null) {
            displaySpeed.value = progress.currentMbps
            const ratio = Math.min(1, Math.max(0, (progress.progressPct - 20) / 45))
            if (downloadSamples.value.length === 0 && ratio > 0) {
              downloadSamples.value.push({ speed: progress.currentMbps, ratio: 0 })
            }
            downloadSamples.value.push({ speed: progress.currentMbps, ratio })
          }
          if (progress.downloadMbps) downloadResult.value = progress.downloadMbps
        } else if (progress.phase === 'upload') {
          currentPhase.value = 'upload'
          // Garante que o download preenche até a borda direita (ratio: 1)
          if (
            downloadSamples.value.length > 0 &&
            downloadSamples.value[downloadSamples.value.length - 1].ratio < 1
          ) {
            downloadSamples.value.push({
              speed:
                downloadResult.value ??
                downloadSamples.value[downloadSamples.value.length - 1].speed,
              ratio: 1,
            })
          }
          if (progress.currentMbps !== undefined && progress.currentMbps !== null) {
            displaySpeed.value = progress.currentMbps
            const ratio = Math.min(1, Math.max(0, (progress.progressPct - 68) / 28))
            if (uploadSamples.value.length === 0 && ratio > 0) {
              uploadSamples.value.push({ speed: progress.currentMbps, ratio: 0 })
            }
            uploadSamples.value.push({ speed: progress.currentMbps, ratio })
          }
          if (progress.uploadMbps) uploadResult.value = progress.uploadMbps
        } else if (progress.phase === 'complete' || progress.phase === 'done') {
          currentPhase.value = 'complete'
          if (
            uploadSamples.value.length > 0 &&
            uploadSamples.value[uploadSamples.value.length - 1].ratio < 1
          ) {
            uploadSamples.value.push({
              speed:
                uploadResult.value ?? uploadSamples.value[uploadSamples.value.length - 1].speed,
              ratio: 1,
            })
          }
        }

        if (progress.pingMs) pingResult.value = progress.pingMs
        if (progress.jitterMs) jitterResult.value = progress.jitterMs
        if (progress.serverName) serverName.value = progress.serverName
        if (progress.serverLocation) serverLocation.value = progress.serverLocation
      },
      (result) => {
        currentPhase.value = 'complete'
        pingResult.value = result.pingMs
        jitterResult.value = result.jitterMs
        downloadResult.value = result.downloadMbps
        uploadResult.value = result.uploadMbps
        displaySpeed.value = result.downloadMbps
        serverName.value = result.serverName || null
        serverLocation.value = result.serverLocation || null
        if (
          downloadSamples.value.length > 0 &&
          downloadSamples.value[downloadSamples.value.length - 1].ratio < 1
        ) {
          downloadSamples.value.push({ speed: result.downloadMbps, ratio: 1 })
        }
        if (
          uploadSamples.value.length > 0 &&
          uploadSamples.value[uploadSamples.value.length - 1].ratio < 1
        ) {
          uploadSamples.value.push({ speed: result.uploadMbps, ratio: 1 })
        }
      },
      testProfile.value
    )
  } else {
    // Modo LAN
    const result = await diagnosticsStore.runLanTest((progress) => {
      progressPct.value = progress.progressPct
      if (progress.phase === 'ping') {
        currentPhase.value = 'ping'
      } else if (progress.phase === 'download') {
        currentPhase.value = 'download'
        if (progress.currentMbps !== undefined && progress.currentMbps !== null) {
          displaySpeed.value = progress.currentMbps
          const ratio = Math.min(1, Math.max(0, (progress.progressPct - 22) / 45))
          if (downloadSamples.value.length === 0 && ratio > 0) {
            downloadSamples.value.push({ speed: progress.currentMbps, ratio: 0 })
          }
          downloadSamples.value.push({ speed: progress.currentMbps, ratio })
        }
      } else if (progress.phase === 'upload') {
        currentPhase.value = 'upload'
        if (
          downloadSamples.value.length > 0 &&
          downloadSamples.value[downloadSamples.value.length - 1].ratio < 1
        ) {
          downloadSamples.value.push({
            speed:
              downloadResult.value ?? downloadSamples.value[downloadSamples.value.length - 1].speed,
            ratio: 1,
          })
        }
        if (progress.currentMbps !== undefined && progress.currentMbps !== null) {
          displaySpeed.value = progress.currentMbps
          const ratio = Math.min(1, Math.max(0, (progress.progressPct - 68) / 28))
          if (uploadSamples.value.length === 0 && ratio > 0) {
            uploadSamples.value.push({ speed: progress.currentMbps, ratio: 0 })
          }
          uploadSamples.value.push({ speed: progress.currentMbps, ratio })
        }
      } else if (progress.phase === 'complete') {
        currentPhase.value = 'complete'
        if (
          uploadSamples.value.length > 0 &&
          uploadSamples.value[uploadSamples.value.length - 1].ratio < 1
        ) {
          uploadSamples.value.push({
            speed: uploadResult.value ?? uploadSamples.value[uploadSamples.value.length - 1].speed,
            ratio: 1,
          })
        }
      }

      if (progress.pingMs) pingResult.value = progress.pingMs
      if (progress.jitterMs) jitterResult.value = progress.jitterMs
      if (progress.downloadMbps) downloadResult.value = progress.downloadMbps
      if (progress.uploadMbps) uploadResult.value = progress.uploadMbps
      if (progress.serverName) serverName.value = progress.serverName
      if (progress.serverLocation) serverLocation.value = progress.serverLocation
    }, testProfile.value)

    if (result) {
      currentPhase.value = 'complete'
      displaySpeed.value = result.downloadMbps
      if (
        downloadSamples.value.length > 0 &&
        downloadSamples.value[downloadSamples.value.length - 1].ratio < 1
      ) {
        downloadSamples.value.push({ speed: result.downloadMbps, ratio: 1 })
      }
      if (
        uploadSamples.value.length > 0 &&
        uploadSamples.value[uploadSamples.value.length - 1].ratio < 1
      ) {
        uploadSamples.value.push({ speed: result.uploadMbps, ratio: 1 })
      }
    }
  }
}

function cancelTest() {
  diagnosticsStore.cancelSpeedTest()
  currentPhase.value = 'idle'
}
</script>

<style scoped>
.chart-card {
  background: rgb(var(--v-theme-surface));
}
.chart-wrapper {
  height: 200px;
}
.speed-plot {
  width: 100%;
  height: 180px;
}
.speed-plot__svg {
  display: block;
  overflow: visible;
}
.speed-plot__grid {
  stroke: rgba(var(--v-theme-on-surface), 0.1);
  stroke-width: 1;
  shape-rendering: crispEdges;
}
.speed-plot__grid--base {
  stroke: rgba(var(--v-theme-on-surface), 0.28);
}
.speed-plot__label {
  fill: rgba(var(--v-theme-on-surface), 0.72);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  dominant-baseline: middle;
}
.speed-plot__line {
  fill: none;
  stroke-width: 1.5;
  stroke-linecap: round;
  stroke-linejoin: round;
}
.speed-plot__marker {
  stroke: rgb(var(--v-theme-surface));
  stroke-width: 1.5;
}
.legend-bar {
  display: inline-block;
  width: 14px;
  height: 3px;
  border-radius: 2px;
}
.font-monospace {
  font-family: 'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace;
}
</style>
