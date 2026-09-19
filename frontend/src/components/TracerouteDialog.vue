<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 950"
    :fullscreen="$vuetify.display.xs"
    @update:model-value="onUpdateModelValue"
  >
    <v-card class="rounded-lg">
      <v-card-title class="d-flex align-center justify-space-between pa-4 bg-primary text-white">
        <div class="d-flex align-center ga-2" style="gap: 8px">
          <v-icon>mdi-routes</v-icon>
          <span>Traceroute ICMP{{ deviceName ? ` — ${deviceName}` : '' }}</span>
        </div>
        <v-btn icon variant="text" color="white" @click="close">
          <v-icon>mdi-close</v-icon>
        </v-btn>
      </v-card-title>

      <v-card-text class="pa-6">
        <v-row>
          <v-col cols="12" sm="6">
            <v-text-field
              v-model="hostModel"
              label="Host ou Endereço IP de Destino *"
              placeholder="Ex: 1.1.1.1 ou google.com"
              variant="outlined"
              density="comfortable"
              :disabled="diagnosticsStore.tracerouteRunning"
              prepend-inner-icon="mdi-target"
              hide-details
            ></v-text-field>
          </v-col>
          <v-col cols="6" sm="3">
            <v-select
              v-model="maxHops"
              :items="[15, 20, 30, 45, 60]"
              label="Máx. Saltos"
              variant="outlined"
              density="comfortable"
              :disabled="diagnosticsStore.tracerouteRunning"
              hide-details
            ></v-select>
          </v-col>
          <v-col cols="6" sm="3">
            <v-select
              v-model="timeoutMs"
              :items="[
                { title: 'Rápido (800ms)', value: 800 },
                { title: 'Normal (1500ms)', value: 1500 },
                { title: 'Conservador (3000ms)', value: 3000 },
              ]"
              label="Timeout por Salto"
              variant="outlined"
              density="comfortable"
              :disabled="diagnosticsStore.tracerouteRunning"
              hide-details
            ></v-select>
          </v-col>
        </v-row>

        <v-alert
          v-if="diagnosticsStore.tracerouteError"
          type="error"
          variant="tonal"
          density="compact"
          class="mt-4 mb-2"
        >
          {{ diagnosticsStore.tracerouteError }}
        </v-alert>

        <div class="d-flex align-center justify-space-between flex-wrap ga-3 mt-4 mb-4">
          <div class="d-flex align-center ga-2">
            <v-btn
              v-if="!diagnosticsStore.tracerouteRunning"
              color="primary"
              prepend-icon="mdi-play"
              :disabled="!hostModel"
              @click="startTraceroute"
            >
              Iniciar Traceroute
            </v-btn>
            <v-btn
              v-else
              color="error"
              variant="tonal"
              prepend-icon="mdi-stop-circle-outline"
              @click="cancelTraceroute"
            >
              Interromper
            </v-btn>

            <v-btn
              v-if="hops.length > 0"
              variant="outlined"
              size="small"
              prepend-icon="mdi-content-copy"
              @click="copyRouteText"
            >
              Copiar Rota
            </v-btn>
          </div>

          <div v-if="hops.length > 0" class="text-caption text-grey-darken-1 font-weight-medium">
            {{ hops.length }} salto(s) resolvido(s)
            <span v-if="reachedTarget" class="text-success ml-2 font-weight-bold">
              <v-icon size="14" color="success">mdi-check-circle</v-icon> Destino alcançado
            </span>
          </div>
        </div>

        <v-progress-linear
          v-if="diagnosticsStore.tracerouteRunning"
          indeterminate
          color="primary"
          rounded
          class="mb-4"
        ></v-progress-linear>

        <!-- Tabela de Saltos -->
        <v-card v-if="hops.length > 0" variant="outlined" class="rounded-lg overflow-hidden">
          <v-table density="compact" hover>
            <thead>
              <tr class="bg-grey-lighten-4">
                <th style="width: 70px" class="font-weight-bold"># Salto</th>
                <th class="font-weight-bold">Endereço IP / Hostname</th>
                <th class="font-weight-bold text-center">Sondas (RTT)</th>
                <th class="font-weight-bold text-center">Latência Média</th>
                <th class="font-weight-bold text-right">Status</th>
              </tr>
            </thead>
            <tbody>
              <tr
                v-for="hop in hops"
                :key="hop.hop"
                :class="{ 'bg-green-lighten-5': hop.status === 'reached' }"
              >
                <td class="font-weight-bold text-primary">
                  <v-avatar size="24" color="primary" variant="tonal" class="text-caption">
                    {{ hop.hop }}
                  </v-avatar>
                </td>
                <td>
                  <div class="d-flex flex-column py-1">
                    <span v-if="hop.ip" class="font-weight-bold text-body-2 font-monospace">
                      {{ hop.ip }}
                    </span>
                    <span v-else class="text-grey font-italic text-caption">
                      * * * (Tempo esgotado / Sem resposta)
                    </span>
                    <span
                      v-if="hop.hostname"
                      class="text-caption text-grey-darken-1 d-flex align-center"
                    >
                      <v-icon size="12" class="mr-1">mdi-dns-outline</v-icon>
                      {{ hop.hostname }}
                    </span>
                  </div>
                </td>
                <td class="text-center">
                  <div class="d-flex justify-center ga-1">
                    <v-chip
                      v-for="(rtt, idx) in hop.rttMs"
                      :key="idx"
                      size="x-small"
                      :color="getRttColor(rtt)"
                      variant="flat"
                    >
                      {{ rtt !== null ? `${rtt.toFixed(1)} ms` : '*' }}
                    </v-chip>
                  </div>
                </td>
                <td class="text-center font-weight-bold">
                  <span
                    v-if="hop.avgRttMs !== null && hop.avgRttMs !== undefined"
                    :class="getRttTextColor(hop.avgRttMs)"
                  >
                    {{ hop.avgRttMs.toFixed(1) }} ms
                  </span>
                  <span v-else class="text-grey">*</span>
                </td>
                <td class="text-right">
                  <v-chip
                    size="small"
                    :color="getStatusColor(hop.status)"
                    variant="tonal"
                    class="font-weight-bold"
                  >
                    {{ getStatusLabel(hop.status) }}
                  </v-chip>
                </td>
              </tr>
            </tbody>
          </v-table>
        </v-card>

        <div
          v-else-if="!diagnosticsStore.tracerouteRunning"
          class="text-center text-grey py-12 border rounded-lg bg-grey-lighten-5"
        >
          <v-icon size="48" color="grey-lighten-1" class="mb-2">mdi-routes-clock</v-icon>
          <div class="text-body-1 font-weight-medium">Nenhum traceroute executado ainda</div>
          <div class="text-caption">
            Clique em "Iniciar Traceroute" para traçar o caminho da rede até o destino.
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
import { computed, ref, watch } from 'vue'
import { useDiagnosticsStore, type TracerouteHop } from '@/stores/diagnostics'

const props = defineProps<{
  modelValue: boolean
  host?: string
  deviceName?: string
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
}>()

const diagnosticsStore = useDiagnosticsStore()

const hostModel = ref('')
const maxHops = ref(30)
const timeoutMs = ref(1500)
const hops = ref<TracerouteHop[]>([])

watch(
  () => props.modelValue,
  (open) => {
    if (open) {
      if (props.host) {
        hostModel.value = props.host
      }
      hops.value = []
    } else if (diagnosticsStore.tracerouteRunning) {
      diagnosticsStore.cancelTraceroute()
    }
  },
  { immediate: true }
)

const reachedTarget = computed(() => {
  return hops.value.some((h) => h.status === 'reached')
})

function onUpdateModelValue(val: boolean) {
  emit('update:modelValue', val)
}

function close() {
  emit('update:modelValue', false)
}

async function startTraceroute() {
  if (!hostModel.value) return
  hops.value = []

  await diagnosticsStore.runTraceroute(
    {
      host: hostModel.value.trim(),
      maxHops: maxHops.value,
      timeoutMs: timeoutMs.value,
      probesPerHop: 3,
    },
    (hop) => {
      hops.value.push(hop)
    }
  )
}

function cancelTraceroute() {
  diagnosticsStore.cancelTraceroute()
}

function getRttColor(rtt: number | null | undefined): string {
  if (rtt === null || rtt === undefined) return 'grey'
  if (rtt < 30) return 'green-darken-1'
  if (rtt < 100) return 'amber-darken-2'
  return 'red-darken-1'
}

function getRttTextColor(rtt: number): string {
  if (rtt < 30) return 'text-success font-weight-bold'
  if (rtt < 100) return 'text-warning font-weight-bold'
  return 'text-error font-weight-bold'
}

function getStatusColor(status: string): string {
  switch (status) {
    case 'reached':
      return 'success'
    case 'intermediate':
      return 'primary'
    case 'timeout':
      return 'grey'
    default:
      return 'info'
  }
}

function getStatusLabel(status: string): string {
  switch (status) {
    case 'reached':
      return 'Destino'
    case 'intermediate':
      return 'Salto'
    case 'timeout':
      return 'Tempo Esgotado'
    default:
      return status
  }
}

function copyRouteText() {
  const lines = hops.value.map((h) => {
    const ipStr = h.ip || '* * *'
    const hostStr = h.hostname ? ` (${h.hostname})` : ''
    const rtts = h.rttMs.map((r) => (r !== null ? `${r.toFixed(1)}ms` : '*')).join(' ')
    return `${h.hop.toString().padStart(2, ' ')}  ${ipStr}${hostStr}  [${rtts}]`
  })
  const header = `Traceroute para ${hostModel.value} (${new Date().toLocaleString()}):\n`
  navigator.clipboard.writeText(header + lines.join('\n'))
}
</script>

<style scoped>
.font-monospace {
  font-family: 'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace;
}
</style>
