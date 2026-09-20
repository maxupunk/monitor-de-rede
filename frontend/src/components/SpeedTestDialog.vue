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
          <span>Teste de Velocidade</span>
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
              <span class="text-subtitle-1 font-weight-medium text-grey text-uppercase">
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
                <span class="d-flex align-center text-caption text-primary font-weight-medium">
                  <span class="legend-bar bg-primary mr-1"></span>
                  Download
                </span>
                <span class="d-flex align-center text-caption text-purple font-weight-medium">
                  <span class="legend-bar bg-purple mr-1"></span>
                  Upload
                </span>
              </div>
            </div>

            <div v-if="serverLocation" class="text-caption text-grey-darken-1 d-flex align-center">
              <v-icon size="14" class="mr-1">mdi-map-marker-radius</v-icon>
              {{ serverName || 'Padrão' }} ({{ serverLocation }})
            </div>
          </div>

          <!-- Área do Gráfico SVG -->
          <div class="chart-wrapper pa-2 position-relative">
            <svg class="w-100 speed-chart-svg" viewBox="0 0 760 190" preserveAspectRatio="none">
              <defs>
                <linearGradient id="grad-down" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stop-color="#1976D2" stop-opacity="0.35" />
                  <stop offset="100%" stop-color="#1976D2" stop-opacity="0.03" />
                </linearGradient>
                <linearGradient id="grad-up" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stop-color="#9C27B0" stop-opacity="0.35" />
                  <stop offset="100%" stop-color="#9C27B0" stop-opacity="0.03" />
                </linearGradient>
              </defs>

              <!-- Linhas de Grade e Marcadores Y -->
              <line
                x1="60"
                y1="25"
                x2="745"
                y2="25"
                stroke="#BDBDBD"
                stroke-dasharray="3,3"
                stroke-width="0.8"
              />
              <text
                x="54"
                y="29"
                font-size="11"
                fill="#757575"
                text-anchor="end"
                font-family="monospace"
              >
                {{ formatSpeedValue(yScaleMax, effectiveChartUnit) }}
              </text>

              <line
                x1="60"
                y1="85"
                x2="745"
                y2="85"
                stroke="#BDBDBD"
                stroke-dasharray="3,3"
                stroke-width="0.8"
              />
              <text
                x="54"
                y="89"
                font-size="11"
                fill="#757575"
                text-anchor="end"
                font-family="monospace"
              >
                {{ formatSpeedValue(yScaleMid, effectiveChartUnit) }}
              </text>

              <line
                x1="60"
                y1="145"
                x2="745"
                y2="145"
                stroke="#BDBDBD"
                stroke-dasharray="3,3"
                stroke-width="0.8"
              />
              <text
                x="54"
                y="149"
                font-size="11"
                fill="#757575"
                text-anchor="end"
                font-family="monospace"
              >
                {{ formatSpeedValue(yScaleLow, effectiveChartUnit) }}
              </text>

              <line x1="60" y1="170" x2="745" y2="170" stroke="#9E9E9E" stroke-width="1.2" />
              <text
                x="54"
                y="174"
                font-size="11"
                fill="#757575"
                text-anchor="end"
                font-family="monospace"
              >
                0
              </text>

              <!-- Estado Ocioso: Linha Guia e Mensagem -->
              <g v-if="downloadSamples.length === 0 && uploadSamples.length === 0">
                <text x="400" y="105" font-size="13" fill="#9E9E9E" text-anchor="middle">
                  Selecione o perfil e clique em Iniciar Teste para visualizar o gráfico em tempo
                  real
                </text>
              </g>

              <!-- Área e Linha de Download -->
              <polygon
                v-if="downloadAreaPoints"
                :points="downloadAreaPoints"
                fill="url(#grad-down)"
              />
              <polyline
                v-if="downloadLinePoints"
                :points="downloadLinePoints"
                fill="none"
                stroke="#1976D2"
                stroke-width="2.5"
                stroke-linecap="round"
                stroke-linejoin="round"
              />

              <!-- Área e Linha de Upload -->
              <polygon v-if="uploadAreaPoints" :points="uploadAreaPoints" fill="url(#grad-up)" />
              <polyline
                v-if="uploadLinePoints"
                :points="uploadLinePoints"
                fill="none"
                stroke="#9C27B0"
                stroke-width="2.5"
                stroke-linecap="round"
                stroke-linejoin="round"
              />

              <!-- Ponto Ativo Mais Recente -->
              <circle
                v-if="activePoint"
                :cx="activePoint.x"
                :cy="activePoint.y"
                r="5"
                :fill="activePointColor"
                stroke="#FFFFFF"
                stroke-width="2"
              />
            </svg>

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
              <div class="text-caption text-grey font-weight-medium">Download</div>
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
              <div class="text-caption text-grey font-weight-medium">Upload</div>
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
              <div class="text-caption text-grey font-weight-medium">Latência (Ping)</div>
              <div class="text-h5 font-weight-bold font-monospace text-teal">
                {{ formatLatency(pingResult, '--') }}
              </div>
            </v-card>
          </v-col>

          <v-col cols="6" sm="3">
            <v-card variant="tonal" color="amber-darken-3" class="rounded-lg pa-3 text-center">
              <div class="text-caption text-grey font-weight-medium">Jitter</div>
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

const yScaleMid = computed(() => yScaleMax.value * 0.66)
const yScaleLow = computed(() => yScaleMax.value * 0.33)

// Mapeamento dos pontos SVG para o gráfico
const chartBounds = {
  left: 60,
  right: 745,
  top: 25,
  bottom: 170,
}

function calculatePoint(sample: SpeedSample) {
  const width = chartBounds.right - chartBounds.left
  const height = chartBounds.bottom - chartBounds.top
  const x = chartBounds.left + sample.ratio * width
  const speedInUnit = convertSpeedFromMbps(sample.speed, effectiveChartUnit.value)
  const normalizedSpeed = Math.min(Math.max(speedInUnit, 0), yScaleMax.value)
  const y = chartBounds.bottom - (normalizedSpeed / yScaleMax.value) * height
  return { x: Math.round(x * 10) / 10, y: Math.round(y * 10) / 10 }
}

const downloadPoints = computed(() => {
  return downloadSamples.value.map((s) => calculatePoint(s))
})

const uploadPoints = computed(() => {
  return uploadSamples.value.map((s) => calculatePoint(s))
})

const downloadLinePoints = computed(() => {
  if (downloadPoints.value.length === 0) return ''
  return downloadPoints.value.map((p) => `${p.x},${p.y}`).join(' ')
})

const downloadAreaPoints = computed(() => {
  if (downloadPoints.value.length === 0) return ''
  const first = downloadPoints.value[0]
  const last = downloadPoints.value[downloadPoints.value.length - 1]
  const line = downloadPoints.value.map((p) => `${p.x},${p.y}`).join(' ')
  return `${first.x},${chartBounds.bottom} ${line} ${last.x},${chartBounds.bottom}`
})

const uploadLinePoints = computed(() => {
  if (uploadPoints.value.length === 0) return ''
  return uploadPoints.value.map((p) => `${p.x},${p.y}`).join(' ')
})

const uploadAreaPoints = computed(() => {
  if (uploadPoints.value.length === 0) return ''
  const first = uploadPoints.value[0]
  const last = uploadPoints.value[uploadPoints.value.length - 1]
  const line = uploadPoints.value.map((p) => `${p.x},${p.y}`).join(' ')
  return `${first.x},${chartBounds.bottom} ${line} ${last.x},${chartBounds.bottom}`
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

const activePointColor = computed(() => {
  if (currentPhase.value === 'upload') return '#9C27B0'
  return '#1976D2'
})

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
.speed-chart-svg {
  height: 180px;
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
