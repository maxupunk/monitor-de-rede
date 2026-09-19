<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 800"
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
        <!-- Seletor de Modo: WAN vs LAN -->
        <div class="d-flex justify-center mb-6">
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
        </div>

        <!-- Velocímetro Central / Indicador de Status -->
        <div class="d-flex flex-column align-center justify-center my-6">
          <div class="gauge-container position-relative d-flex align-center justify-center">
            <v-progress-circular
              :model-value="progressPct"
              :size="220"
              :width="14"
              color="primary"
              class="gauge-circle"
            >
              <div class="d-flex flex-column align-center">
                <span class="text-h3 font-weight-bold text-primary font-monospace">
                  {{ displaySpeed.toFixed(1) }}
                </span>
                <span
                  class="text-caption text-grey font-weight-medium text-uppercase tracking-wide"
                >
                  Mbps
                </span>
                <v-chip
                  size="x-small"
                  :color="phaseColor"
                  variant="flat"
                  class="mt-2 font-weight-bold text-uppercase"
                >
                  {{ phaseLabel }}
                </v-chip>
              </div>
            </v-progress-circular>
          </div>

          <div
            v-if="serverLocation"
            class="text-caption text-grey-darken-1 mt-4 d-flex align-center"
          >
            <v-icon size="14" class="mr-1">mdi-map-marker-radius</v-icon>
            Servidor: <strong>{{ serverName || 'Padrão' }}</strong> ({{ serverLocation }})
          </div>
        </div>

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
                {{ downloadResult !== null ? `${downloadResult.toFixed(1)}` : '--' }}
                <span class="text-caption font-weight-regular">Mbps</span>
              </div>
            </v-card>
          </v-col>

          <v-col cols="6" sm="3">
            <v-card variant="tonal" color="purple" class="rounded-lg pa-3 text-center">
              <div class="text-caption text-grey font-weight-medium">Upload</div>
              <div class="text-h5 font-weight-bold font-monospace text-purple">
                {{ uploadResult !== null ? `${uploadResult.toFixed(1)}` : '--' }}
                <span class="text-caption font-weight-regular">Mbps</span>
              </div>
            </v-card>
          </v-col>

          <v-col cols="6" sm="3">
            <v-card variant="tonal" color="teal" class="rounded-lg pa-3 text-center">
              <div class="text-caption text-grey font-weight-medium">Latência (Ping)</div>
              <div class="text-h5 font-weight-bold font-monospace text-teal">
                {{ pingResult !== null ? `${pingResult.toFixed(1)}` : '--' }}
                <span class="text-caption font-weight-regular">ms</span>
              </div>
            </v-card>
          </v-col>

          <v-col cols="6" sm="3">
            <v-card variant="tonal" color="amber-darken-3" class="rounded-lg pa-3 text-center">
              <div class="text-caption text-grey font-weight-medium">Jitter</div>
              <div class="text-h5 font-weight-bold font-monospace text-amber-darken-3">
                {{ jitterResult !== null ? `${jitterResult.toFixed(1)}` : '--' }}
                <span class="text-caption font-weight-regular">ms</span>
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

const props = defineProps<{
  modelValue: boolean
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
}>()

const diagnosticsStore = useDiagnosticsStore()

const testMode = ref<'wan' | 'lan'>('wan')
const currentPhase = ref<'idle' | 'ping' | 'download' | 'upload' | 'complete'>('idle')
const progressPct = ref(0)
const displaySpeed = ref(0)

const pingResult = ref<number | null>(null)
const jitterResult = ref<number | null>(null)
const downloadResult = ref<number | null>(null)
const uploadResult = ref<number | null>(null)
const serverName = ref<string | null>(null)
const serverLocation = ref<string | null>(null)

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
          if (progress.currentMbps) displaySpeed.value = progress.currentMbps
          if (progress.downloadMbps) downloadResult.value = progress.downloadMbps
        } else if (progress.phase === 'upload') {
          currentPhase.value = 'upload'
          if (progress.currentMbps) displaySpeed.value = progress.currentMbps
          if (progress.uploadMbps) uploadResult.value = progress.uploadMbps
        } else if (progress.phase === 'complete' || progress.phase === 'done') {
          currentPhase.value = 'complete'
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
      }
    )
  } else {
    // Modo LAN
    const result = await diagnosticsStore.runLanTest((progress) => {
      progressPct.value = progress.progressPct
      if (progress.phase === 'ping') {
        currentPhase.value = 'ping'
      } else if (progress.phase === 'download') {
        currentPhase.value = 'download'
        if (progress.currentMbps) displaySpeed.value = progress.currentMbps
      } else if (progress.phase === 'upload') {
        currentPhase.value = 'upload'
        if (progress.currentMbps) displaySpeed.value = progress.currentMbps
      } else if (progress.phase === 'complete') {
        currentPhase.value = 'complete'
      }

      if (progress.pingMs) pingResult.value = progress.pingMs
      if (progress.jitterMs) jitterResult.value = progress.jitterMs
      if (progress.downloadMbps) downloadResult.value = progress.downloadMbps
      if (progress.uploadMbps) uploadResult.value = progress.uploadMbps
      if (progress.serverName) serverName.value = progress.serverName
      if (progress.serverLocation) serverLocation.value = progress.serverLocation
    })

    if (result) {
      currentPhase.value = 'complete'
      displaySpeed.value = result.downloadMbps
    }
  }
}

function cancelTest() {
  diagnosticsStore.cancelSpeedTest()
  currentPhase.value = 'idle'
}
</script>

<style scoped>
.gauge-container {
  width: 220px;
  height: 220px;
}
.font-monospace {
  font-family: 'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace;
}
</style>
