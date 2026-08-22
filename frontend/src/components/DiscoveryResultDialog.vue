<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 700"
    :fullscreen="$vuetify.display.xs"
    @update:model-value="onUpdateModelValue"
  >
    <v-card v-if="result" class="rounded-lg">
      <v-card-title class="d-flex align-center justify-space-between pa-4 bg-primary text-white">
        <div class="d-flex align-center ga-2" style="gap: 8px">
          <v-icon>mdi-radar</v-icon>
          <span>Detalhes do Dispositivo Descoberto</span>
        </div>
        <v-btn icon variant="text" color="white" @click="close">
          <v-icon>mdi-close</v-icon>
        </v-btn>
      </v-card-title>

      <v-card-text class="pa-6">
        <!-- Header com IP e status -->
        <div class="d-flex align-center justify-space-between flex-wrap ga-3 mb-4">
          <div class="d-flex align-center ga-3">
            <v-avatar color="primary" size="48">
              <v-icon color="white" size="28">mdi-router-network</v-icon>
            </v-avatar>
            <div>
              <div class="text-h6 font-weight-bold">
                {{ result.mdnsName || result.hostname || result.ipAddress }}
              </div>
              <div class="text-caption text-grey">
                IP: {{ result.ipAddress }}
                <span v-if="'discoveryRun' in result && result.discoveryRun?.network">
                  — {{ result.discoveryRun.network.name }} ({{ result.discoveryRun.network.cidr }})
                </span>
              </div>
            </div>
          </div>
          <v-chip :color="isAdded ? 'success' : 'warning'" size="small" variant="flat">
            {{ isAdded ? 'JÁ ADICIONADO' : 'PENDENTE' }}
          </v-chip>
        </div>

        <!-- Grid de informações -->
        <v-row class="mb-4">
          <v-col cols="12" sm="6">
            <v-list border class="rounded-lg">
              <v-list-item
                title="Endereço MAC"
                :subtitle="result.macAddress || 'Não identificado'"
              ></v-list-item>
              <v-list-item
                title="Hostname (DNS)"
                :subtitle="result.hostname || 'Não identificado'"
              ></v-list-item>
              <v-list-item
                title="Nome mDNS/Bonjour"
                :subtitle="result.mdnsName || 'Não identificado'"
              ></v-list-item>
            </v-list>
          </v-col>
          <v-col cols="12" sm="6">
            <v-list border class="rounded-lg">
              <v-list-item
                title="Fabricante"
                :subtitle="result.vendor || 'Não identificado'"
              ></v-list-item>
              <v-list-item title="Tipo de Dispositivo" :subtitle="deviceTypeLabel"></v-list-item>
              <v-list-item
                title="Confiança"
                :subtitle="`${Math.ceil(result.confidence)}%`"
              ></v-list-item>
            </v-list>
          </v-col>
        </v-row>

        <!-- Portas abertas -->
        <v-card v-if="openPorts.length > 0" variant="outlined" class="rounded-lg pa-4 mb-4">
          <div class="text-subtitle-2 font-weight-bold mb-3 d-flex align-center ga-2">
            <v-icon size="18" color="primary">mdi-lan-connect</v-icon>
            Portas Abertas ({{ openPorts.length }})
          </div>
          <div class="d-flex flex-wrap ga-2">
            <v-chip
              v-for="port in openPorts"
              :key="port"
              size="small"
              color="success"
              variant="tonal"
            >
              {{ port }}
              <span v-if="serviceName(port)" class="text-caption ml-1"
              >({{ serviceName(port) }})</span
              >
            </v-chip>
          </div>
        </v-card>

        <!-- Datas -->
        <v-card
          v-if="result.firstSeenAt || result.lastSeenAt"
          variant="outlined"
          class="rounded-lg pa-4 mb-4"
        >
          <div class="text-subtitle-2 font-weight-bold mb-3 d-flex align-center ga-2">
            <v-icon size="18" color="info">mdi-clock-outline</v-icon>
            Linha do Tempo
          </div>
          <v-row>
            <v-col cols="12" sm="6">
              <div class="text-caption text-grey">Primeira vez visto</div>
              <div class="text-body-2">{{ formatDateTime(result.firstSeenAt, '—') }}</div>
            </v-col>
            <v-col cols="12" sm="6">
              <div class="text-caption text-grey">Última vez visto</div>
              <div class="text-body-2">{{ formatDateTime(result.lastSeenAt, '—') }}</div>
            </v-col>
          </v-row>
        </v-card>

        <!-- JSON bruto -->
        <div>
          <div class="d-flex align-center justify-space-between mb-2">
            <div class="text-subtitle-2 font-weight-bold d-flex align-center ga-2">
              <v-icon size="18" color="info">mdi-code-json</v-icon>
              Dados Brutos da Descoberta
            </div>
            <v-btn
              size="x-small"
              variant="tonal"
              color="primary"
              prepend-icon="mdi-content-copy"
              @click="copyJson"
            >
              {{ copied ? 'Copiado!' : 'Copiar JSON' }}
            </v-btn>
          </div>
          <v-card variant="outlined" class="bg-grey-lighten-4 pa-3 overflow-x-auto rounded">
            <pre
              class="text-caption font-mono text-grey-darken-3 mb-0"
              style="white-space: pre-wrap; word-break: break-word"
            >{{ rawJson }}</pre>
          </v-card>
        </div>
      </v-card-text>

      <v-divider></v-divider>

      <v-card-actions class="pa-4 justify-end">
        <v-btn variant="text" @click="close">Fechar</v-btn>
        <v-btn v-if="!isAdded" color="success" prepend-icon="mdi-plus" @click="addDevice">
          Adicionar
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useDevicesStore } from '@/stores/devices'
import type { DiscoveryResult, StreamedDiscoveryHost } from '@/stores/discovery'
import { formatDateTime } from '@/utils/formatters'

const props = defineProps<{
  modelValue: boolean
  result: DiscoveryResult | StreamedDiscoveryHost | null
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  (e: 'add'): void
}>()

const devicesStore = useDevicesStore()
const copied = ref(false)

const isAdded = computed(() => {
  if (!props.result) return false
  return devicesStore.devices.some((d) => d.ipAddress === props.result?.ipAddress)
})

const deviceTypeLabel = computed(() => {
  const type = props.result?.deviceType
  if (!type || type === 'unknown') return 'Desconhecido'
  const labels: Record<string, string> = {
    router: 'Roteador',
    switch: 'Switch',
    access_point: 'Access Point',
    printer: 'Impressora',
    camera: 'Câmera',
    server: 'Servidor',
    web_device: 'Dispositivo Web',
    other: 'Outro',
  }
  return labels[type] || type
})

const openPorts = computed(() => {
  const result = props.result
  if (result && Array.isArray(result.openPorts) && result.openPorts.length > 0) {
    return result.openPorts
  }
  const data = result?.data
  if (data && Array.isArray(data.openPorts)) return data.openPorts as number[]
  return []
})

const rawJson = computed(() => {
  if (!props.result) return '{}'
  return JSON.stringify(props.result, null, 2)
})

function serviceName(port: number): string {
  const map: Record<number, string> = {
    22: 'SSH',
    53: 'DNS',
    80: 'HTTP',
    139: 'NetBIOS',
    161: 'SNMP',
    443: 'HTTPS',
    445: 'SMB',
    554: 'RTSP',
    1900: 'SSDP',
    3389: 'RDP',
    5353: 'mDNS',
    8000: 'HTTP Alt',
    8080: 'HTTP Proxy',
    8291: 'Winbox',
    9100: 'Print',
  }
  return map[port] || ''
}

function onUpdateModelValue(val: boolean) {
  emit('update:modelValue', val)
}

function close() {
  emit('update:modelValue', false)
}

function addDevice() {
  emit('add')
}

async function copyJson() {
  try {
    await navigator.clipboard.writeText(rawJson.value)
    copied.value = true
    setTimeout(() => {
      copied.value = false
    }, 2000)
  } catch {
    // clipboard failure fallback
  }
}
</script>
