<template>
  <div class="ai-automation-settings">
    <!-- Ferramentas e ações -->
    <div class="text-subtitle-2 font-weight-bold mb-1">Ferramentas e ações</div>
    <v-checkbox
      v-model="allowActiveTools"
      color="primary"
      density="compact"
      hide-details
      label="Permitir testes ativos de rede (ping, traceroute, scan de portas, DNS e playbooks)"
    />
    <v-checkbox
      v-model="requireToolConfirmation"
      color="primary"
      density="compact"
      hide-details
      :disabled="!allowActiveTools"
      label="Pedir minha confirmação antes de cada teste ativo"
    />
    <v-checkbox
      v-model="allowActions"
      color="primary"
      density="compact"
      hide-details
      label="Permitir que a IA proponha ações: reconhecer ou silenciar alerta, janela de manutenção e criar monitor"
    />
    <div class="text-caption text-medium-emphasis ms-10 mb-4">
      Toda ação aparece no chat com os botões Confirmar e Cancelar; nada é alterado sem o seu
      clique, e a execução fica registrada na auditoria em seu nome.
    </div>

    <!-- IA proativa -->
    <div class="d-flex align-center ga-2 mb-1">
      <span class="text-subtitle-2 font-weight-bold">IA proativa</span>
      <v-chip size="x-small" color="warning" variant="tonal">consome tokens</v-chip>
    </div>
    <div class="text-caption text-medium-emphasis mb-2">
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

    <AiDigestPanel class="mt-3" />
  </div>
</template>

<script setup lang="ts">
import type { AiDigestSchedule } from '@/bindings/AiDigestSchedule'
import type { AiIncidentSeverity } from '@/bindings/AiIncidentSeverity'
import type { AiProactiveSettings } from '@/bindings/AiProactiveSettings'
import AiDigestPanel from './AiDigestPanel.vue'

const allowActiveTools = defineModel<boolean>('allowActiveTools', { required: true })
const requireToolConfirmation = defineModel<boolean>('requireToolConfirmation', {
  required: true,
})
const allowActions = defineModel<boolean>('allowActions', { required: true })
const proactive = defineModel<AiProactiveSettings>('proactive', { required: true })

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
  title: `${String(hour).padStart(2, '0')}:00`,
  value: hour,
}))
</script>
