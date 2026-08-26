<template>
  <div>
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
      :entries="deviceEntries"
      :scroll-key="logsStore.scrollKey"
      :load="logsStore.load"
      :error="logsStore.error"
      :show-source="false"
      empty-hint="Este dispositivo ainda não enviou syslog para o servidor."
    />
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import LogTable from '@/components/logs/LogTable.vue'
import { useLogsStore, SEVERITY_OPTIONS, WINDOW_OPTIONS } from '@/stores/logs'

const props = defineProps<{
  deviceId: number
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

// Defesa de apresentação: mesmo que uma resposta antiga chegue durante uma
// troca de rota, uma linha de outro equipamento nunca é renderizada aqui.
const deviceEntries = computed(() =>
  logsStore.entries.filter((entry) => entry.deviceId === props.deviceId)
)

// Define o escopo antes da primeira renderização e também ao navegar de um
// detalhe diretamente para outro reutilizando a mesma instância do componente.
watch(() => props.deviceId, applyLogFilters, { immediate: true })

defineExpose({
  applyLogFilters,
})
</script>
