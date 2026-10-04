<template>
  <v-card variant="outlined" rounded="lg" class="pa-3 pa-md-4">
    <div class="d-flex flex-wrap align-center justify-space-between ga-3">
      <div class="d-flex align-center ga-3 min-w-0">
        <v-progress-circular
          v-if="active"
          indeterminate
          color="primary"
          size="28"
          width="3"
        ></v-progress-circular>
        <v-icon v-else :color="outcome.color" size="28">{{ outcome.icon }}</v-icon>
        <div class="min-w-0">
          <div class="text-subtitle-1 font-weight-bold">{{ title }}</div>
          <div class="text-body-2">{{ subtitle }}</div>
        </div>
      </div>
      <div class="d-flex align-center ga-2">
        <v-chip color="primary" variant="tonal" prepend-icon="mdi-devices">
          {{ scan.hosts.length }} encontrado(s)
        </v-chip>
        <v-chip v-if="active" color="info" variant="tonal">
          {{ formatPercent(percent, 0) }}
        </v-chip>
      </div>
    </div>

    <!-- Etapas: quem está vivo → o que cada um é -->
    <div v-if="scan.phase !== 'probe'" class="d-flex align-center ga-2 mt-4 steps">
      <template v-for="(step, index) in STEPS" :key="step.phase">
        <v-divider v-if="index > 0" class="steps__line"></v-divider>
        <div class="d-flex align-center ga-2">
          <v-avatar size="26" :color="stepColor(index)" :variant="stepVariant(index)">
            <v-icon v-if="stepDone(index)" size="16">mdi-check</v-icon>
            <span v-else class="text-caption font-weight-bold">{{ index + 1 }}</span>
          </v-avatar>
          <span class="text-body-2 font-weight-medium">{{ step.label }}</span>
        </div>
      </template>
    </div>

    <v-progress-linear
      v-if="active"
      :model-value="percent"
      color="primary"
      height="8"
      rounded
      class="mt-3"
    ></v-progress-linear>

    <v-alert
      v-if="scan.error"
      type="error"
      variant="tonal"
      density="compact"
      class="mt-3"
      :text="scan.error"
    ></v-alert>

    <div v-if="scan.logs.length > 0" class="mt-3">
      <v-btn
        variant="text"
        size="small"
        color="primary"
        :append-icon="showLog ? 'mdi-chevron-up' : 'mdi-chevron-down'"
        @click="showLog = !showLog"
      >
        Registro da varredura ({{ scan.logs.length }})
      </v-btn>
      <v-expand-transition>
        <div v-if="showLog" class="scan-log font-mono text-body-2 mt-1">
          <div v-for="(line, index) in scan.logs" :key="index">{{ line }}</div>
        </div>
      </v-expand-transition>
    </div>
  </v-card>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import type { DiscoveryPhase, ScanSessionState } from '@/stores/discovery'
import { formatPercent } from '@/utils/formatters'

const props = defineProps<{
  scan: ScanSessionState
  percent: number
  networkLabel?: string | null
}>()

const STEPS: { phase: DiscoveryPhase; label: string }[] = [
  { phase: 'sweep', label: 'Hosts ativos' },
  { phase: 'identify', label: 'Identificação' },
]

const PHASE_TEXT: Record<DiscoveryPhase, string> = {
  idle: 'Preparando a varredura…',
  sweep: 'Ping, ARP e portas-chave: quem responde aparece abaixo.',
  identify: 'Portas, SNMP, nomes (DNS/NetBIOS), mDNS, UPnP e página web de cada host.',
  probe: 'A varredura roda no probe remoto desta rede.',
}

const OUTCOMES: Record<string, { title: string; icon: string; color: string }> = {
  completed: { title: 'Varredura concluída', icon: 'mdi-check-circle', color: 'success' },
  cancelled: { title: 'Varredura cancelada', icon: 'mdi-stop-circle', color: 'warning' },
  failed: { title: 'A varredura falhou', icon: 'mdi-alert-circle', color: 'error' },
}

const showLog = ref(false)

const active = computed(() => props.scan.status === 'running' || props.scan.status === 'pending')
const outcome = computed(() => OUTCOMES[props.scan.status] ?? OUTCOMES.completed)
const currentStep = computed(() => STEPS.findIndex((step) => step.phase === props.scan.phase))

const title = computed(() => {
  if (!active.value) return outcome.value.title
  const where = props.networkLabel ? ` — ${props.networkLabel}` : ''
  return props.scan.status === 'pending' ? `Na fila${where}` : `Varrendo${where}`
})

const subtitle = computed(() => (active.value ? PHASE_TEXT[props.scan.phase] : finishedText.value))

const finishedText = computed(() => {
  if (props.scan.status === 'completed') {
    return `${props.scan.hosts.length} dispositivo(s) encontrado(s) na última varredura.`
  }
  return props.scan.status === 'failed'
    ? 'Veja o motivo abaixo e tente de novo.'
    : 'Os hosts encontrados até o cancelamento continuam abaixo.'
})

function stepDone(index: number): boolean {
  return !active.value ? props.scan.status === 'completed' : index < currentStep.value
}

function stepColor(index: number): string {
  if (stepDone(index)) return 'success'
  return index === currentStep.value ? 'primary' : 'secondary'
}

function stepVariant(index: number): 'flat' | 'tonal' {
  return stepDone(index) || index === currentStep.value ? 'flat' : 'tonal'
}
</script>

<style scoped>
.steps__line {
  max-width: 64px;
}

.scan-log {
  max-height: 180px;
  overflow-y: auto;
  padding: 8px 12px;
  border-radius: 8px;
  background: rgba(var(--v-theme-on-surface), 0.05);
  color: rgb(var(--v-theme-on-surface));
}

.min-w-0 {
  min-width: 0;
}
</style>
