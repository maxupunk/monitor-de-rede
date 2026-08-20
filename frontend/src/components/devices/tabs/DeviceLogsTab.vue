<template>
  <div>
    <!-- Chamada para ação enquanto o equipamento nunca enviou nada -->
    <v-alert v-if="logsNaoConfigurados" type="info" variant="tonal" border="start" class="mb-4">
      <div class="d-flex align-center flex-wrap ga-3">
        <div class="flex-grow-1">
          <div class="font-weight-bold mb-1">
            Este equipamento ainda não envia log para o servidor.
          </div>
          O envio de syslog é configurado no próprio roteador. O servidor pode fazer isso sozinho:
          ele acessa o equipamento, aplica os comandos e confirma a chegada da primeira mensagem.
        </div>
        <div class="d-flex ga-2">
          <v-btn color="primary" variant="flat" @click="emit('openAutoSetup')">
            <v-icon start>mdi-flash</v-icon>
            Ativar log
          </v-btn>
          <v-btn color="primary" variant="tonal" @click="emit('openSetup')"> Ver comandos </v-btn>
        </div>
      </div>
    </v-alert>

    <!-- Mascaramento do Docker -->
    <v-alert
      v-if="logsStore.natMasking"
      type="warning"
      variant="tonal"
      border="start"
      class="mb-4"
      density="comfortable"
    >
      <div class="font-weight-bold mb-1">
        O Docker está reescrevendo o endereço de origem das mensagens.
      </div>
      Todos os equipamentos chegam como
      <strong>{{ (logsStore.nat?.gateways ?? []).join(', ') || 'o gateway da bridge' }}</strong
      >, então o vínculo passa a depender do nome que cada um envia no syslog. Abra
      <RouterLink to="/logs">Logs</RouterLink> para vincular por nome, ou publique o servidor com
      <code>network_mode: host</code> para o endereço real chegar.
    </v-alert>

    <div class="d-flex align-center flex-wrap ga-3 mb-4">
      <v-select
        v-model="logSeverity"
        :items="logSeverityOptions"
        item-title="label"
        item-value="value"
        label="Severidade"
        hide-details
        clearable
        density="compact"
        variant="outlined"
        style="max-width: 240px"
        @update:model-value="applyLogFilters"
      ></v-select>
      <v-select
        v-model="logHours"
        :items="logWindowOptions"
        item-title="label"
        item-value="value"
        label="Período"
        hide-details
        density="compact"
        variant="outlined"
        style="max-width: 200px"
        @update:model-value="applyLogFilters"
      ></v-select>
      <v-spacer></v-spacer>
      <v-btn color="primary" variant="tonal" size="small" @click="emit('openAutoSetup')">
        <v-icon start>mdi-flash</v-icon>
        <span class="hidden-xs">Ativar log</span>
      </v-btn>
      <v-btn
        :color="logsStore.tailing ? 'success' : 'primary'"
        :variant="logsStore.tailing ? 'flat' : 'tonal'"
        size="small"
        @click="logsStore.toggleTail()"
      >
        <v-icon start>
          {{ logsStore.tailing ? 'mdi-radiobox-marked' : 'mdi-play-circle-outline' }}
        </v-icon>
        {{ logsStore.tailing ? 'Ao vivo' : 'Acompanhar' }}
      </v-btn>
    </div>

    <LogTable
      :entries="logsStore.entries"
      :scroll-key="logsStore.scrollKey"
      :load="logsStore.load"
      :error="logsStore.error"
      :show-source="false"
      empty-hint="Este dispositivo ainda não enviou syslog para o servidor."
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import LogTable from '@/components/logs/LogTable.vue'
import { useLogsStore, SEVERITY_OPTIONS, WINDOW_OPTIONS } from '@/stores/logs'

const props = defineProps<{
  deviceId: number
}>()

const emit = defineEmits<{
  (e: 'openAutoSetup'): void
  (e: 'openSetup'): void
}>()

const logsStore = useLogsStore()
const logSeverity = ref<number | null>(null)
const logHours = ref<number | null>(24)
const logSeverityOptions = SEVERITY_OPTIONS
const logWindowOptions = WINDOW_OPTIONS

function applyLogFilters(): void {
  const estavaAoVivo = logsStore.tailing
  if (estavaAoVivo) logsStore.stopTail()
  logsStore.applyFilters({
    deviceId: props.deviceId,
    severity: logSeverity.value,
    hours: logHours.value,
    search: '',
  })
  if (estavaAoVivo) logsStore.startTail()
}

const logsNaoConfigurados = computed(() => {
  if (!logsStore.sourcesLoaded) return false
  if (logsStore.sources.some((fonte) => fonte.deviceId === props.deviceId)) return false
  return logsStore.isEmpty
})

defineExpose({
  applyLogFilters,
})
</script>
