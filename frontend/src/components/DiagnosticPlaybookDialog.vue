<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 850"
    :fullscreen="$vuetify.display.xs"
    @update:model-value="onUpdateModelValue"
  >
    <v-card class="rounded-lg">
      <v-card-title class="d-flex align-center justify-space-between pa-4 bg-primary text-white">
        <div class="d-flex align-center ga-2" style="gap: 8px">
          <v-icon>mdi-clipboard-pulse-outline</v-icon>
          <span>Playbook de Diagnóstico{{ deviceName ? ` — ${deviceName}` : '' }}</span>
        </div>
        <v-btn icon variant="text" color="white" @click="close">
          <v-icon>mdi-close</v-icon>
        </v-btn>
      </v-card-title>

      <v-card-text class="pa-6">
        <!-- Seletor do Playbook -->
        <v-row class="mb-4">
          <v-col cols="12" sm="7">
            <v-select
              v-model="selectedPlaybook"
              :items="[
                { title: '🌐 Conexão com a Internet (WAN)', value: 'internet_health' },
                { title: '🖥️ Diagnóstico de Dispositivo Local', value: 'device_reachability' },
              ]"
              label="Tipo de Playbook"
              variant="outlined"
              density="comfortable"
              :disabled="diagnosticsStore.playbookRunning"
              hide-details
            ></v-select>
          </v-col>

          <v-col v-if="selectedPlaybook === 'device_reachability'" cols="12" sm="5">
            <v-text-field
              v-model="targetIpModel"
              label="Endereço IP do Dispositivo *"
              placeholder="Ex: 192.168.1.50"
              variant="outlined"
              density="comfortable"
              :disabled="diagnosticsStore.playbookRunning || !!deviceId"
              hide-details
            ></v-text-field>
          </v-col>
        </v-row>

        <div class="d-flex align-center justify-space-between flex-wrap ga-3 mb-6">
          <v-btn
            v-if="!diagnosticsStore.playbookRunning"
            color="primary"
            prepend-icon="mdi-play"
            size="large"
            rounded="pill"
            class="px-6 font-weight-bold"
            @click="startPlaybook"
          >
            Executar Diagnóstico
          </v-btn>
          <v-btn
            v-else
            color="error"
            variant="tonal"
            prepend-icon="mdi-stop-circle-outline"
            size="large"
            rounded="pill"
            class="px-6 font-weight-bold"
            @click="cancelPlaybook"
          >
            Interromper
          </v-btn>

          <v-btn
            v-if="summary"
            variant="outlined"
            size="small"
            prepend-icon="mdi-content-copy"
            @click="copyReport"
          >
            Copiar Relatório
          </v-btn>
        </div>

        <v-alert
          v-if="diagnosticsStore.playbookError"
          type="error"
          variant="tonal"
          density="compact"
          class="mb-4"
        >
          {{ diagnosticsStore.playbookError }}
        </v-alert>

        <!-- Checklist de Passos -->
        <v-card variant="outlined" class="rounded-lg mb-6 overflow-hidden">
          <v-list density="comfortable" lines="two">
            <template v-for="(step, idx) in displaySteps" :key="step.stepIndex">
              <v-list-item :title="step.stepName" :subtitle="step.message || step.description">
                <template #prepend>
                  <v-avatar :color="getStepColor(step.status)" size="36" class="text-white mr-3">
                    <v-icon size="20">{{ getStepIcon(step.status) }}</v-icon>
                  </v-avatar>
                </template>

                <template #append>
                  <v-chip
                    size="small"
                    :color="getStepColor(step.status)"
                    variant="tonal"
                    class="font-weight-bold text-uppercase"
                  >
                    {{ getStepLabel(step.status) }}
                  </v-chip>
                </template>
              </v-list-item>
              <v-divider v-if="idx < displaySteps.length - 1"></v-divider>
            </template>
          </v-list>
        </v-card>

        <!-- Cartão Final de Diagnóstico Conclusivo -->
        <v-card
          v-if="summary"
          variant="tonal"
          :color="getSummaryColor(summary.status)"
          class="rounded-lg pa-4 mb-2"
        >
          <div class="d-flex align-center ga-2 mb-2">
            <v-icon size="24" :color="getSummaryColor(summary.status)">
              {{ getSummaryIcon(summary.status) }}
            </v-icon>
            <span class="text-h6 font-weight-bold">
              Síntese do Diagnóstico:
              {{
                summary.status === 'success'
                  ? 'Saudável'
                  : summary.status === 'warning'
                    ? 'Atenção'
                    : 'Falha Detectada'
              }}
            </span>
          </div>

          <p class="text-body-1 mb-3 font-weight-medium">
            {{ summary.diagnosis }}
          </p>

          <div v-if="summary.recommendations.length > 0">
            <div class="text-subtitle-2 font-weight-bold mb-1">
              Ações e Recomendações Sugeridas:
            </div>
            <ul class="pl-4 text-body-2 d-flex flex-column ga-1">
              <li v-for="(rec, rIdx) in summary.recommendations" :key="rIdx">
                {{ rec }}
              </li>
            </ul>
          </div>
        </v-card>
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
import {
  useDiagnosticsStore,
  type PlaybookStepResult,
  type PlaybookSummary,
} from '@/stores/diagnostics'

const props = defineProps<{
  modelValue: boolean
  initialPlaybookType?: string
  deviceId?: number
  deviceName?: string
  deviceIp?: string
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
}>()

const diagnosticsStore = useDiagnosticsStore()

const selectedPlaybook = ref<'internet_health' | 'device_reachability'>('internet_health')
const targetIpModel = ref('')
const executedSteps = ref<PlaybookStepResult[]>([])
const summary = ref<PlaybookSummary | null>(null)

watch(
  () => props.modelValue,
  (open) => {
    if (open) {
      if (props.initialPlaybookType === 'device_reachability' || props.deviceId || props.deviceIp) {
        selectedPlaybook.value = 'device_reachability'
      } else {
        selectedPlaybook.value = 'internet_health'
      }
      if (props.deviceIp) {
        targetIpModel.value = props.deviceIp
      }
      executedSteps.value = []
      summary.value = null
    } else if (diagnosticsStore.playbookRunning) {
      diagnosticsStore.cancelPlaybook()
    }
  },
  { immediate: true }
)

// Passos esperados para pré-visualização visual antes e durante a execução
const placeholderSteps = computed<PlaybookStepResult[]>(() => {
  if (selectedPlaybook.value === 'internet_health') {
    return [
      {
        stepIndex: 1,
        stepName: 'Resolução DNS',
        description: 'Consulta de resolução de nomes para servidores públicos (1.1.1.1)',
        status: 'pending',
      },
      {
        stepIndex: 2,
        stepName: 'Latência Externa (Ping)',
        description: 'Medição de latência ICMP contra endereço público',
        status: 'pending',
      },
      {
        stepIndex: 3,
        stepName: 'Traçado de Rota (Traceroute)',
        description: 'Mapeamento dos saltos intermediários até o destino público',
        status: 'pending',
      },
      {
        stepIndex: 4,
        stepName: 'Teste de Velocidade (WAN)',
        description: 'Medição de Download, Upload, Ping e Jitter via Cloudflare',
        status: 'pending',
      },
    ]
  } else {
    return [
      {
        stepIndex: 1,
        stepName: 'Sonda ICMP (Ping)',
        description: 'Envio de pacotes Echo Request para o endereço do dispositivo',
        status: 'pending',
      },
      {
        stepIndex: 2,
        stepName: 'Sonda TCP de Portas',
        description: 'Verificação de conectividade TCP em portas comuns de serviço',
        status: 'pending',
      },
      {
        stepIndex: 3,
        stepName: 'Traçado de Rota Local',
        description: 'Mapeamento dos nós de rede locais até o dispositivo',
        status: 'pending',
      },
    ]
  }
})

const displaySteps = computed(() => {
  if (executedSteps.value.length === 0) {
    return placeholderSteps.value
  }
  const result: PlaybookStepResult[] = []
  for (const placeholder of placeholderSteps.value) {
    const executed = executedSteps.value.find((s) => s.stepIndex === placeholder.stepIndex)
    if (executed) {
      result.push(executed)
    } else {
      result.push(placeholder)
    }
  }
  return result
})

function onUpdateModelValue(val: boolean) {
  emit('update:modelValue', val)
}

function close() {
  emit('update:modelValue', false)
}

async function startPlaybook() {
  executedSteps.value = []
  summary.value = null

  await diagnosticsStore.runPlaybook(
    {
      playbookType: selectedPlaybook.value,
      target: targetIpModel.value || undefined,
      deviceId: props.deviceId,
    },
    (step) => {
      const existingIdx = executedSteps.value.findIndex((s) => s.stepIndex === step.stepIndex)
      if (existingIdx >= 0) {
        executedSteps.value[existingIdx] = step
      } else {
        executedSteps.value.push(step)
      }
    },
    (sum) => {
      summary.value = sum
    }
  )
}

function cancelPlaybook() {
  diagnosticsStore.cancelPlaybook()
}

function getStepColor(status: string): string {
  switch (status) {
    case 'success':
      return 'success'
    case 'warning':
      return 'warning'
    case 'failed':
      return 'error'
    case 'running':
      return 'primary'
    default:
      return 'grey'
  }
}

function getStepIcon(status: string): string {
  switch (status) {
    case 'success':
      return 'mdi-check'
    case 'warning':
      return 'mdi-alert'
    case 'failed':
      return 'mdi-close'
    case 'running':
      return 'mdi-loading mdi-spin'
    default:
      return 'mdi-clock-outline'
  }
}

function getStepLabel(status: string): string {
  switch (status) {
    case 'success':
      return 'OK'
    case 'warning':
      return 'Atenção'
    case 'failed':
      return 'Falha'
    case 'running':
      return 'Executando'
    default:
      return 'Pendente'
  }
}

function getSummaryColor(status: string): string {
  switch (status) {
    case 'success':
      return 'success'
    case 'warning':
      return 'warning'
    default:
      return 'error'
  }
}

function getSummaryIcon(status: string): string {
  switch (status) {
    case 'success':
      return 'mdi-check-decagram'
    case 'warning':
      return 'mdi-alert-decagram'
    default:
      return 'mdi-alert-octagon'
  }
}

function copyReport() {
  if (!summary.value) return
  const text = [
    `=== Relatório de Diagnóstico NetMonitor ===`,
    `Playbook: ${summary.value.playbookType}`,
    `Alvo: ${summary.value.target || 'N/A'}`,
    `Status Geral: ${summary.value.status.toUpperCase()}`,
    `Síntese: ${summary.value.diagnosis}`,
    `\nEtapas:`,
    ...summary.value.steps.map(
      (s) => `[${s.status.toUpperCase()}] ${s.stepName}: ${s.message || s.description}`
    ),
    `\nRecomendações:`,
    ...summary.value.recommendations.map((r) => `- ${r}`),
  ].join('\n')

  navigator.clipboard.writeText(text)
}
</script>
