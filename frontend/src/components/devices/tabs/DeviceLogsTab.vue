<template>
  <div>
    <v-card v-if="!device.isSystem" variant="tonal" color="primary" class="mb-4 rounded-lg">
      <v-card-text class="d-flex flex-column flex-sm-row align-start align-sm-center ga-3">
        <div class="flex-grow-1">
          <div class="font-weight-bold">Envio de logs deste dispositivo</div>
          <div class="text-body-2 mt-1">
            Configure pela primeira vez ou reaplique o Syslog quando mudar endereço, rota ou
            equipamento. A credencial é usada somente durante a conexão.
          </div>
        </div>
        <v-btn
          color="primary"
          variant="flat"
          prepend-icon="mdi-console-network-outline"
          @click="openLogSetup"
        >
          Configurar Syslog
        </v-btn>
      </v-card-text>
    </v-card>

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

    <SyslogAutoSetupDialog
      v-if="logSetupTarget"
      :key="logSetupTarget.sessionId"
      v-model="logSetupOpen"
      :target="logSetupTarget"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import LogTable from '@/components/logs/LogTable.vue'
import SyslogAutoSetupDialog from '@/components/logs/SyslogAutoSetupDialog.vue'
import { useLogsStore, SEVERITY_OPTIONS, WINDOW_OPTIONS } from '@/stores/logs'
import type { Device } from '@/stores/devices'
import { createLogSetupTarget, type LogSetupTarget } from '@/utils/syslogProvision'

const props = defineProps<{
  deviceId: number
  device: Device
}>()

const logsStore = useLogsStore()
const logSeverity = ref<number | null>(null)
const logHours = ref<number | null>(24)
const logSeverityOptions = SEVERITY_OPTIONS
const logWindowOptions = WINDOW_OPTIONS
const logSetupOpen = ref(false)
const logSetupTarget = ref<Readonly<LogSetupTarget> | null>(null)
let logSetupSequence = 0

function openLogSetup(): void {
  logSetupTarget.value = createLogSetupTarget(
    ++logSetupSequence,
    props.device,
    props.device.operatingSystem ?? 'auto',
    props.device.effectiveOperatingSystem
  )
  logSetupOpen.value = true
}

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
watch(logSetupOpen, (isOpen) => {
  if (!isOpen) logSetupTarget.value = null
})

defineExpose({
  applyLogFilters,
})
</script>
