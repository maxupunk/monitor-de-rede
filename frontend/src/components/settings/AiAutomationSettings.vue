<template>
  <div class="ai-automation-settings">
    <!-- Testes ativos de rede -->
    <AiModeToggle
      v-model="toolsMode"
      title="Testes ativos de rede (ping, traceroute, portas, DNS e playbooks)"
      :options="ACTIVE_TOOLS_OPTIONS"
      class="mb-5"
    />

    <!-- Ações: tudo que muda algo no sistema -->
    <div class="font-weight-bold mb-1">Ações</div>
    <div class="text-body-2 mb-2">
      Ações mudam algo no sistema, ao contrário dos testes. Cada uma aparece no chat e fica
      registrada na auditoria em seu nome.
    </div>
    <v-checkbox
      v-model="allowActions"
      color="primary"
      density="compact"
      hide-details
      label="Alertas, manutenção e monitores: a IA pode propor reconhecer ou silenciar alerta, abrir janela de manutenção e criar monitor"
    />
    <div class="text-body-2 ms-10 mb-4">
      Essas ações sempre pedem o seu clique em Confirmar; nada é alterado sem ele.
    </div>

    <AiModeToggle
      v-model="containerActions"
      title="Containers Docker: iniciar, parar e reiniciar"
      :options="CONTAINER_ACTION_OPTIONS"
      class="mb-5"
    >
      Vale para a central e para os agentes remotos que permitem (política local do agente). Remover
      ou atualizar container nunca fica com a IA.
    </AiModeToggle>

    <!-- IA proativa -->
    <div class="d-flex align-center ga-2 mb-1">
      <span class="font-weight-bold">IA proativa</span>
      <v-chip size="x-small" color="warning" variant="tonal">consome tokens</v-chip>
    </div>
    <div class="text-body-2 mb-2">
      Rotinas que chamam o provedor sem você perguntar. Usam só consultas de leitura e respondem no
      modo direto.
    </div>

    <v-switch
      v-model="proactive.incidentSummaries"
      color="primary"
      density="compact"
      hide-details
      label="Resumir o incidente automaticamente quando um alerta abrir"
    />
    <v-row v-if="proactive.incidentSummaries" dense class="mt-1 mb-2">
      <v-col cols="12" sm="6">
        <v-select
          v-model="proactive.incidentMinSeverity"
          :items="severityOptions"
          label="Severidade mínima"
          variant="outlined"
          density="compact"
          hide-details
        />
      </v-col>
      <v-col cols="12" sm="6">
        <v-text-field
          v-model.number="proactive.maxSummariesPerHour"
          type="number"
          min="1"
          max="60"
          label="Máximo de resumos por hora"
          variant="outlined"
          density="compact"
          hide-details
        />
      </v-col>
    </v-row>

    <v-row dense class="mt-2">
      <v-col cols="12" sm="6">
        <v-select
          v-model="proactive.digest"
          :items="digestOptions"
          label="Resumo da rede pelas notificações"
          variant="outlined"
          density="compact"
          hide-details
        />
      </v-col>
      <v-col cols="12" sm="6">
        <v-select
          v-model="proactive.digestHour"
          :items="hourOptions"
          :disabled="proactive.digest === 'off'"
          label="Horário de envio (horário do servidor)"
          variant="outlined"
          density="compact"
          hide-details
        />
      </v-col>
    </v-row>
    <v-switch
      v-model="proactive.skipQuietDigest"
      color="primary"
      density="compact"
      hide-details
      class="mt-2"
      :disabled="proactive.digest === 'off'"
      label="Não enviar o resumo quando nenhum alerta abriu no período (economiza tokens)"
    />
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { AiContainerActionMode } from '@/bindings/AiContainerActionMode'
import type { AiDigestSchedule } from '@/bindings/AiDigestSchedule'
import type { AiIncidentSeverity } from '@/bindings/AiIncidentSeverity'
import type { AiProactiveSettings } from '@/bindings/AiProactiveSettings'
import { formatHourOfDay } from '@/utils/formatters'
import AiModeToggle from './ai/AiModeToggle.vue'
import {
  ACTIVE_TOOLS_OPTIONS,
  CONTAINER_ACTION_OPTIONS,
  activeToolsFlags,
  activeToolsMode,
  type AiActiveToolsMode,
} from './ai/aiAutomationModes'

const allowActiveTools = defineModel<boolean>('allowActiveTools', { required: true })
const requireToolConfirmation = defineModel<boolean>('requireToolConfirmation', {
  required: true,
})
const allowActions = defineModel<boolean>('allowActions', { required: true })
const containerActions = defineModel<AiContainerActionMode>('containerActions', {
  required: true,
})
const proactive = defineModel<AiProactiveSettings>('proactive', { required: true })

/** Os dois booleanos do contrato vistos como uma escolha só. */
const toolsMode = computed<AiActiveToolsMode>({
  get: () => activeToolsMode(allowActiveTools.value, requireToolConfirmation.value),
  set: (mode) => {
    const flags = activeToolsFlags(mode, requireToolConfirmation.value)
    allowActiveTools.value = flags.allowActiveTools
    requireToolConfirmation.value = flags.requireToolConfirmation
  },
})

const severityOptions: { title: string; value: AiIncidentSeverity }[] = [
  { title: 'Só críticos', value: 'critical' },
  { title: 'Críticos e avisos', value: 'warning' },
]

const digestOptions: { title: string; value: AiDigestSchedule }[] = [
  { title: 'Desligado', value: 'off' },
  { title: 'Diário', value: 'daily' },
  { title: 'Semanal (segunda-feira)', value: 'weekly' },
]

const hourOptions = Array.from({ length: 24 }, (_, hour) => ({
  title: formatHourOfDay(hour),
  value: hour,
}))
</script>
