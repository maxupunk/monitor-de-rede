<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 900"
    :fullscreen="$vuetify.display.xs"
    @update:model-value="onUpdateModelValue"
  >
    <v-card class="rounded-lg">
      <v-card-title class="d-flex align-center justify-space-between pa-4 bg-primary text-white">
        <div class="d-flex align-center ga-2" style="gap: 8px">
          <v-icon>mdi-lan-connect</v-icon>
          <span>Scanner de Portas{{ deviceName ? ` — ${deviceName}` : '' }}</span>
        </div>
        <v-btn icon variant="text" color="white" @click="close">
          <v-icon>mdi-close</v-icon>
        </v-btn>
      </v-card-title>

      <v-card-text class="pa-6">
        <v-row>
          <v-col cols="12" sm="4">
            <v-text-field
              v-model="hostModel"
              label="Host / Endereço IP *"
              placeholder="Ex: 192.168.1.1"
              variant="outlined"
              density="comfortable"
              :disabled="portScanStore.scanning"
              hide-details
            ></v-text-field>
          </v-col>
          <v-col cols="6" sm="2">
            <v-select
              v-model="protocol"
              :items="[
                { title: 'TCP', value: 'tcp' },
                { title: 'UDP', value: 'udp' },
              ]"
              label="Protocolo"
              variant="outlined"
              density="comfortable"
              :disabled="portScanStore.scanning"
              hide-details
            ></v-select>
          </v-col>
          <v-col cols="6" sm="3">
            <v-select
              v-model="presetKey"
              :items="[
                { title: 'Portas Comuns', value: 'common' },
                { title: 'Bem Conhecidas (1-1024)', value: 'range1024' },
                { title: 'Todas (1-65535)', value: 'all' },
                { title: 'Personalizado', value: 'custom' },
              ]"
              label="Intervalo"
              variant="outlined"
              density="comfortable"
              :disabled="portScanStore.scanning"
              hide-details
            ></v-select>
          </v-col>
          <v-col cols="12" sm="3">
            <v-select
              v-model="profile"
              :items="[
                { title: 'Confiável', value: 'reliable' },
                { title: 'Rápido', value: 'fast' },
                { title: 'Completo', value: 'complete' },
              ]"
              label="Perfil"
              variant="outlined"
              density="comfortable"
              :disabled="portScanStore.scanning"
              hide-details
            ></v-select>
          </v-col>

          <v-col cols="12" class="pt-0">
            <div class="text-caption text-grey-darken-1">
              {{ selectedPortCount.toLocaleString('pt-BR') }} porta(s) · pior caso aproximado:
              {{ estimatedDuration }}. Portas que recusam conexão terminam antes.
            </div>
          </v-col>

          <v-col v-if="presetKey === 'custom'" cols="12">
            <v-text-field
              v-model="customPortsInput"
              label="Portas Personalizadas"
              placeholder="Ex: 22,80,443,8000-8100"
              hint="Separe por vírgula. Use um traço para faixas (ex: 20-25). Faixas grandes são processadas em lotes."
              persistent-hint
              variant="outlined"
              density="comfortable"
              :disabled="portScanStore.scanning"
            ></v-text-field>
            <v-alert
              v-if="invalidCustomParts.length"
              type="warning"
              variant="tonal"
              density="compact"
              class="mt-2"
            >
              Itens fora de 1–65535 serão ignorados: {{ invalidCustomParts.join(', ') }}
            </v-alert>
          </v-col>
        </v-row>

        <v-alert
          v-if="protocol === 'udp'"
          type="info"
          variant="tonal"
          density="compact"
          class="mt-2 mb-4"
        >
          Varreduras UDP não garantem resposta: portas sem retorno aparecem como "Aberta/Filtrada" —
          isso é esperado e não indica necessariamente que a porta esteja fechada.
        </v-alert>

        <v-alert
          v-if="scanError"
          :type="scanErrorIsWarningOnly ? 'info' : 'error'"
          variant="tonal"
          density="compact"
          class="mt-2 mb-4"
        >
          {{ scanError }}
        </v-alert>

        <div class="d-flex align-center justify-space-between flex-wrap ga-3 mt-2 mb-4">
          <v-btn
            v-if="!portScanStore.scanning"
            color="primary"
            prepend-icon="mdi-play"
            :disabled="!hostModel"
            @click="startScan"
          >
            Iniciar Varredura
          </v-btn>
          <v-btn
            v-else
            color="error"
            variant="tonal"
            prepend-icon="mdi-stop-circle-outline"
            @click="cancelScan"
          >
            Cancelar Varredura
          </v-btn>

          <v-select
            v-if="results !== null"
            v-model="filterMode"
            :items="[
              { title: 'Apenas Abertas', value: 'open' },
              { title: 'Abertas e Inconclusivas', value: 'open_filtered' },
              { title: 'Todas as Portas', value: 'all' },
            ]"
            label="Exibir"
            variant="outlined"
            density="compact"
            style="max-width: 220px"
            hide-details
          ></v-select>
        </div>

        <div v-if="portScanStore.scanning" class="mb-4">
          <v-progress-linear
            :model-value="progressPercent"
            color="primary"
            height="8"
            rounded
            class="mb-2"
          ></v-progress-linear>
          <div class="text-caption text-grey-darken-1">
            {{ results?.length ?? 0 }} / {{ totalPortsBeingScanned }} portas verificadas
            <span v-if="openCount > 0"> — {{ openCount }} aberta(s) encontrada(s) até agora</span>
          </div>
        </div>

        <div v-if="results !== null">
          <div class="text-caption text-grey-darken-1 mb-2">
            {{ openCount }} porta(s) aberta(s)<span v-if="openFilteredCount > 0"
              >, {{ openFilteredCount }} aberta(s)/filtrada(s)</span
            >
            de {{ results.length }} escaneada(s)
          </div>

          <div class="table-responsive">
            <v-table density="comfortable" hover class="rounded-lg border">
              <thead>
                <tr>
                  <th>Porta</th>
                  <th>Protocolo</th>
                  <th>Serviço</th>
                  <th>Status</th>
                  <th>Latência</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="item in filteredResults" :key="`${item.protocol}-${item.port}`">
                  <td class="font-weight-bold">{{ item.port }}</td>
                  <td>{{ item.protocol.toUpperCase() }}</td>
                  <td>{{ item.service || '-' }}</td>
                  <td>
                    <v-chip :color="statusColor(item.status)" size="x-small" variant="tonal">
                      {{ statusLabel(item.status) }}
                    </v-chip>
                  </td>
                  <td>{{ formatLatency(item.latencyMs) }}</td>
                </tr>
                <tr v-if="filteredResults.length === 0">
                  <td colspan="5" class="text-center text-grey py-4">
                    {{
                      portScanStore.scanning
                        ? 'Aguardando resultados...'
                        : 'Nenhuma porta encontrada para o filtro selecionado.'
                    }}
                  </td>
                </tr>
              </tbody>
            </v-table>
          </div>
        </div>

        <div v-else-if="!portScanStore.scanning" class="text-center text-grey py-8">
          <v-icon size="40" color="grey-lighten-1">mdi-lan-pending</v-icon>
          <div class="mt-2 text-subtitle-2">
            Configure o host e o intervalo de portas acima e clique em "Iniciar Varredura".
          </div>
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
import { ref, computed, watch } from 'vue'
import {
  usePortScanStore,
  type PortScanItem,
  type PortProtocol,
  type PortScanProfile,
} from '@/stores/portScan'
import { getStatusColor } from '@/utils/monitorPresentation'
import { formatLatency } from '@/utils/formatters'

const props = defineProps<{
  modelValue: boolean
  host?: string
  deviceName?: string
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
}>()

const portScanStore = usePortScanStore()

type DisplayFilter = 'open' | 'open_filtered' | 'all'

const hostModel = ref('')
const protocol = ref<PortProtocol>('tcp')
const profile = ref<PortScanProfile>('reliable')
const presetKey = ref<'common' | 'range1024' | 'all' | 'custom'>('common')
const customPortsInput = ref('')
const filterMode = ref<DisplayFilter>('open')
const results = ref<PortScanItem[] | null>(null)
const scanError = ref<string | null>(null)
const scanErrorIsWarningOnly = ref(false)
const totalPortsBeingScanned = ref(0)

const TCP_COMMON_PORTS = [
  21, 22, 23, 25, 53, 80, 110, 111, 135, 139, 143, 389, 443, 445, 465, 554, 587, 993, 995, 1433,
  1521, 2049, 3306, 3389, 5060, 5432, 5900, 6379, 8000, 8080, 8291, 8443, 9000, 9100, 27017,
]
const UDP_COMMON_PORTS = [53, 67, 68, 69, 123, 137, 138, 161, 162, 500, 514, 520, 1900, 4500, 5353]

watch(
  () => props.modelValue,
  (isOpen) => {
    if (isOpen) {
      hostModel.value = props.host || ''
      protocol.value = 'tcp'
      profile.value = 'reliable'
      presetKey.value = 'common'
      customPortsInput.value = ''
      filterMode.value = 'open'
      results.value = null
      scanError.value = null
      scanErrorIsWarningOnly.value = false
      totalPortsBeingScanned.value = 0
    } else if (portScanStore.scanning) {
      portScanStore.cancelScan()
    }
  }
)

const openCount = computed(() => results.value?.filter((r) => r.status === 'open').length ?? 0)

const openFilteredCount = computed(
  () =>
    results.value?.filter((r) => r.status === 'open|filtered' || r.status === 'filtered').length ??
    0
)

const progressPercent = computed(() => {
  if (!totalPortsBeingScanned.value) return 0
  return ((results.value?.length ?? 0) / totalPortsBeingScanned.value) * 100
})

const selectedPortCount = computed(() => {
  if (presetKey.value === 'range1024') return 1024
  if (presetKey.value === 'all') return 65535
  if (presetKey.value === 'custom') return parsePortsInput(customPortsInput.value).length
  return protocol.value === 'tcp' ? TCP_COMMON_PORTS.length : UDP_COMMON_PORTS.length
})

const estimatedDuration = computed(() => {
  const settings = {
    fast: { concurrency: 512, perWaveSeconds: 1.5 },
    reliable: { concurrency: 256, perWaveSeconds: 3.1 },
    complete: { concurrency: 64, perWaveSeconds: 7.4 },
  }[profile.value]
  const seconds =
    Math.ceil(selectedPortCount.value / settings.concurrency) * settings.perWaveSeconds
  if (seconds < 60) return `${Math.max(1, Math.ceil(seconds))} s`
  if (seconds < 3600) return `${Math.ceil(seconds / 60)} min`
  return `${(seconds / 3600).toFixed(1)} h`
})

const filteredResults = computed(() => {
  if (!results.value) return []
  const sorted = [...results.value].sort((a, b) => a.port - b.port)
  if (filterMode.value === 'open') {
    return sorted.filter((r) => r.status === 'open')
  }
  if (filterMode.value === 'open_filtered') {
    return sorted.filter(
      (r) => r.status === 'open' || r.status === 'open|filtered' || r.status === 'filtered'
    )
  }
  return sorted
})

function parsePortsInput(input: string): number[] {
  const parts = input
    .split(',')
    .map((s) => s.trim())
    .filter(Boolean)
  const ports = new Set<number>()

  for (const part of parts) {
    const rangeMatch = part.match(/^(\d+)\s*-\s*(\d+)$/)
    if (rangeMatch) {
      let start = Number(rangeMatch[1])
      let end = Number(rangeMatch[2])
      if (start > end) [start, end] = [end, start]
      const boundedStart = Math.max(1, start)
      const boundedEnd = Math.min(65535, end)
      for (let p = boundedStart; p <= boundedEnd; p++) {
        ports.add(p)
      }
    } else {
      const p = Number(part)
      if (Number.isInteger(p) && p >= 1 && p <= 65535) ports.add(p)
    }
  }

  return Array.from(ports).sort((a, b) => a - b)
}

const invalidCustomParts = computed(() => {
  if (presetKey.value !== 'custom') return []
  return customPortsInput.value
    .split(',')
    .map((part) => part.trim())
    .filter(Boolean)
    .filter((part) => {
      const range = part.match(/^(\d+)\s*-\s*(\d+)$/)
      if (range) {
        const start = Number(range[1])
        const end = Number(range[2])
        return start < 1 || start > 65535 || end < 1 || end > 65535
      }
      const port = Number(part)
      return !Number.isInteger(port) || port < 1 || port > 65535
    })
})

function resolvePorts(): number[] {
  if (presetKey.value === 'custom') {
    return parsePortsInput(customPortsInput.value)
  }
  if (presetKey.value === 'range1024') {
    return Array.from({ length: 1024 }, (_, i) => i + 1)
  }
  if (presetKey.value === 'all') {
    return Array.from({ length: 65535 }, (_, i) => i + 1)
  }
  return protocol.value === 'tcp' ? TCP_COMMON_PORTS : UDP_COMMON_PORTS
}

const statusColor = getStatusColor

function statusLabel(status: PortScanItem['status']) {
  if (status === 'open') return 'ABERTA'
  if (status === 'closed') return 'FECHADA'
  if (status === 'open|filtered') return 'ABERTA/FILTRADA'
  if (status === 'filtered') return 'FILTRADA/TIMEOUT'
  if (status === 'unreachable') return 'HOST INALCANÇÁVEL'
  return 'ERRO LOCAL'
}

async function startScan() {
  if (!hostModel.value) return

  const ports = resolvePorts()
  if (ports.length === 0) {
    scanError.value = 'Informe ao menos uma porta válida.'
    scanErrorIsWarningOnly.value = false
    return
  }

  scanError.value = null
  scanErrorIsWarningOnly.value = false
  results.value = []
  totalPortsBeingScanned.value = ports.length

  const completed = await portScanStore.scanPorts(
    { host: hostModel.value, protocol: protocol.value, ports, profile: profile.value },
    (item) => {
      results.value?.push(item)
    }
  )

  if (!completed) {
    if (portScanStore.error) {
      scanError.value = portScanStore.error
      scanErrorIsWarningOnly.value = false
      return
    }
    scanError.value = `Varredura cancelada (${results.value?.length ?? 0}/${totalPortsBeingScanned.value} portas verificadas).`
    scanErrorIsWarningOnly.value = true
  }
}

function cancelScan() {
  portScanStore.cancelScan()
}

function onUpdateModelValue(val: boolean) {
  emit('update:modelValue', val)
}

function close() {
  if (portScanStore.scanning) portScanStore.cancelScan()
  emit('update:modelValue', false)
}
</script>
