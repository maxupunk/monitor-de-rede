<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 620"
    :fullscreen="$vuetify.display.xs"
    scrollable
    @update:model-value="$emit('update:modelValue', $event)"
  >
    <v-card class="rounded-xl" :aria-busy="diagnostics.quickPingRunning">
      <v-card-item class="py-4 px-6">
        <div class="d-flex align-start ga-3">
          <v-avatar color="info" variant="flat" size="38">
            <v-icon size="22">mdi-pulse</v-icon>
          </v-avatar>
          <div class="flex-grow-1 min-w-0">
            <v-card-title class="text-h6 font-weight-bold pa-0 text-wrap">
              {{ QUICK_PING_TITLE }} — estimativa de baixa precisão
            </v-card-title>
            <div class="qp-target text-truncate">{{ targetLabel }}</div>
          </div>
          <v-btn
            icon="mdi-close"
            variant="text"
            density="comfortable"
            aria-label="Fechar"
            @click="close"
          ></v-btn>
        </div>
      </v-card-item>

      <v-card-text class="pa-4 pa-sm-6">
        <v-chip
          color="info"
          variant="tonal"
          size="small"
          label
          prepend-icon="mdi-information-outline"
          class="mb-2"
        >
          Caminho todo, não um enlace
        </v-chip>
        <p class="qp-scope mb-4">{{ QUICK_PING_SCOPE }}</p>

        <div v-if="diagnostics.quickPingRunning" class="py-4" role="status">
          <div class="d-flex align-center ga-2 mb-2">
            <v-progress-circular
              indeterminate
              size="18"
              width="2"
              color="info"
            ></v-progress-circular>
            <span>Enviando 160 pings intercalados (pequenos e grandes)… cerca de 4 s.</span>
          </div>
          <v-progress-linear indeterminate color="info" height="4" rounded></v-progress-linear>
        </div>

        <v-alert
          v-else-if="diagnostics.quickPingError"
          type="error"
          variant="tonal"
          density="comfortable"
          :text="diagnostics.quickPingError"
        ></v-alert>

        <QuickPingResult v-else-if="report" :report="report" />
      </v-card-text>

      <v-card-actions class="px-6 pb-4 flex-wrap ga-2">
        <v-btn
          v-if="canFullTest"
          color="primary"
          variant="flat"
          prepend-icon="mdi-speedometer"
          :disabled="diagnostics.quickPingRunning"
          @click="fullTest"
        >
          Fazer o teste de velocidade completo
        </v-btn>
        <v-spacer></v-spacer>
        <v-btn
          color="info"
          variant="outlined"
          prepend-icon="mdi-refresh"
          :disabled="diagnostics.quickPingRunning"
          @click="run"
        >
          Repetir
        </v-btn>
        <v-btn color="primary" variant="text" @click="close">Fechar</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
/**
 * Roda o diagnóstico rápido por ping ao abrir e mostra o resultado. O teste
 * de velocidade completo é pedido pelo ponto único `linkTestRequests` — quem
 * abre o assistente é a camada de teste de enlace do mapa.
 */
import { computed, ref, watch } from 'vue'
import type { QuickPingReport } from '@/bindings/QuickPingReport'
import { useAuthStore } from '@/stores/auth'
import { useDiagnosticsStore } from '@/stores/diagnostics'
import { useLinkTestRequestsStore } from '@/stores/linkTestRequests'
import { QUICK_PING_SCOPE, QUICK_PING_TITLE } from '@/utils/diagnostics/quickPing'
import QuickPingResult from './QuickPingResult.vue'

const props = defineProps<{
  modelValue: boolean
  deviceId: number | null
  deviceName?: string | null
  ipAddress?: string | null
  /** O equipamento pode receber o teste de velocidade (a tela sabe se há quem o abra). */
  offerFullTest?: boolean
}>()

const emit = defineEmits<{ (e: 'update:modelValue', value: boolean): void }>()

const diagnostics = useDiagnosticsStore()
const auth = useAuthStore()
const linkTestRequests = useLinkTestRequestsStore()
const report = ref<QuickPingReport | null>(null)

const targetLabel = computed(() =>
  [props.deviceName, props.ipAddress ?? report.value?.target].filter(Boolean).join(' · ')
)
const canFullTest = computed(
  () => Boolean(props.offerFullTest) && props.deviceId !== null && auth.canWrite
)

async function run() {
  if (props.deviceId === null) return
  report.value = null
  report.value = await diagnostics.quickPing(props.deviceId)
}

function close() {
  emit('update:modelValue', false)
}

function fullTest() {
  if (props.deviceId === null) return
  linkTestRequests.requestLinkTest({ kind: 'server', targetDeviceId: props.deviceId })
  close()
}

watch(
  () => [props.modelValue, props.deviceId] as const,
  ([open, deviceId], previous) => {
    const reopened = !previous || !previous[0] || previous[1] !== deviceId
    if (open && reopened) void run()
  },
  { immediate: true }
)
</script>

<style scoped>
.qp-target,
.qp-scope {
  font-size: 0.875rem;
  color: rgb(var(--v-theme-on-surface));
}
</style>
