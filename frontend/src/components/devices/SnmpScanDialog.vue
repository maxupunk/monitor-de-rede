<template>
  <div>
    <!-- Modal de Descoberta SNMP -->
    <v-dialog v-model="open" :fullscreen="$vuetify.display.xs" max-width="900" scrollable>
      <v-card class="rounded-lg">
        <v-card-title
          class="font-weight-bold d-flex align-center justify-space-between bg-primary text-white pa-4"
        >
          <div class="d-flex align-center ga-2" style="gap: 8px">
            <v-icon>mdi-radar</v-icon>
            <span>Escaneamento & Descoberta SNMP</span>
          </div>
          <div class="d-flex align-center ga-1">
            <v-btn
              variant="text"
              color="white"
              size="small"
              prepend-icon="mdi-tune-vertical"
              @click="profilesDialogOpen = true"
            >
              Perfis SNMP
            </v-btn>
            <v-btn icon variant="text" color="white" @click="open = false">
              <v-icon>mdi-close</v-icon>
            </v-btn>
          </div>
        </v-card-title>

        <v-card-text class="pa-6">
          <div v-if="detailStore.scanningSnmp" class="text-center py-8">
            <v-progress-circular
              indeterminate
              color="primary"
              size="48"
              class="mb-4"
            ></v-progress-circular>
            <div class="text-subtitle-1">
              Escaneando dispositivo via SNMP em {{ detailStore.device?.ipAddress }}...
            </div>
            <div class="text-caption text-high-emphasis">
              Consultando interfaces e uso de CPU/memória...
            </div>
          </div>

          <div v-else-if="detailStore.scanResult">
            <v-alert
              v-if="Object.keys(detailStore.scanResult.collectorErrors || {}).length"
              type="warning"
              variant="tonal"
              density="compact"
              class="mb-4"
            >
              Coleta parcial:
              {{
                Object.entries(detailStore.scanResult.collectorErrors)
                  .map(([collector, error]) => `${collector}: ${error}`)
                  .join(' · ')
              }}
            </v-alert>
            <v-alert
              v-if="!detailStore.scanResult.snmpResponded"
              type="warning"
              variant="tonal"
              class="mb-4"
              prepend-icon="mdi-alert-circle-outline"
              title="Nenhuma resposta SNMP"
              text="O dispositivo não respondeu a nenhum OID consultado, mesmo os padrão (sysDescr/sysName). Confira: (1) SNMP está habilitado no próprio equipamento — não só aqui no cadastro; (2) a community configurada aqui bate com a community de leitura configurada no equipamento; (3) a versão SNMP (v1/v2c/v3) está correta; (4) a porta 161/UDP chega ao equipamento a partir deste servidor (sem firewall/NAT no meio)."
            ></v-alert>

            <v-alert
              v-else
              type="info"
              variant="tonal"
              class="mb-4"
              prepend-icon="mdi-router"
              title="Dispositivo Conectado"
              :subtitle="detailStore.scanResult.systemInfo.sysDescr || 'Dispositivo SNMP'"
            ></v-alert>

            <!-- Perfil Detectado -->
            <v-alert
              v-if="detailStore.scanResult.matchedProfile"
              type="success"
              variant="tonal"
              class="mb-4"
              prepend-icon="mdi-solar-power-variant-outline"
              :title="`Perfil Detectado: ${detailStore.scanResult.matchedProfile.name}`"
            >
              <div class="d-flex align-center justify-space-between flex-wrap ga-2 mt-1">
                <div>
                  <strong>Fabricante:</strong> {{ detailStore.scanResult.matchedProfile.vendor }} ·
                  <strong>Categoria:</strong> {{ detailStore.scanResult.matchedProfile.category }}
                </div>
                <v-btn
                  size="small"
                  variant="outlined"
                  color="success"
                  prepend-icon="mdi-format-list-bulleted"
                  @click="profilesDialogOpen = true"
                >
                  Gerenciar Perfis
                </v-btn>
              </div>
            </v-alert>

            <v-alert
              v-else-if="detailStore.scanResult.snmpResponded"
              type="info"
              variant="tonal"
              density="compact"
              class="mb-4"
              prepend-icon="mdi-information-outline"
            >
              <div class="d-flex align-center justify-space-between flex-wrap ga-2">
                <span>
                  Dispositivo sem perfil específico mapeado.
                  <template v-if="detailStore.scanResult.interfaces.length === 0">
                    Se for um controlador solar (MPPT), nobreak ou sensor IoT, você pode cadastrar
                    um perfil com os OIDs.
                  </template>
                </span>
                <v-btn
                  size="x-small"
                  variant="outlined"
                  color="primary"
                  prepend-icon="mdi-plus"
                  @click="profilesDialogOpen = true"
                >
                  Cadastrar Perfil
                </v-btn>
              </div>
            </v-alert>

            <!-- Telemetria & Sensores Mapeados -->
            <v-card
              v-if="detailStore.scanResult.sensors && detailStore.scanResult.sensors.length > 0"
              border
              flat
              class="mb-6 rounded-lg pa-4"
            >
              <div class="d-flex align-center justify-space-between mb-3">
                <div
                  class="text-subtitle-1 font-weight-bold d-flex align-center ga-2"
                  style="gap: 8px"
                >
                  <v-icon color="primary">mdi-lightning-bolt</v-icon>
                  Sensores & Telemetria do Equipamento ({{ detailStore.scanResult.sensors.length }})
                </div>
                <div class="d-flex align-center ga-2" style="gap: 8px">
                  <v-btn size="small" variant="text" color="primary" @click="selectAllSensors">
                    Selecionar Todos
                  </v-btn>
                  <v-btn size="small" variant="text" color="primary" @click="unselectAllSensors">
                    Desmarcar Todos
                  </v-btn>
                </div>
              </div>

              <v-row>
                <v-col
                  v-for="sensor in detailStore.scanResult.sensors"
                  :key="sensor.key"
                  cols="12"
                  sm="6"
                  md="4"
                >
                  <v-card
                    variant="tonal"
                    color="surface-variant"
                    class="pa-3 rounded-lg fill-height d-flex flex-column justify-space-between border"
                  >
                    <div class="d-flex align-center justify-space-between">
                      <div
                        class="text-caption font-weight-bold text-truncate"
                        :title="sensor.label"
                      >
                        {{ sensor.label }}
                      </div>
                      <v-switch
                        :model-value="selectedSensors.includes(sensor.key)"
                        color="primary"
                        density="compact"
                        hide-details
                        @update:model-value="toggleSensor(sensor.key)"
                      ></v-switch>
                    </div>
                    <div class="mt-2 d-flex align-center justify-space-between">
                      <div class="text-h6 font-weight-bold text-primary text-no-wrap">
                        {{ formatSensorDisplay(sensor) }}
                      </div>
                      <v-tooltip v-if="sensor.oid" location="top">
                        <template #activator="{ props: tooltipProps }">
                          <v-icon v-bind="tooltipProps" size="16" color="info">
                            mdi-information-outline
                          </v-icon>
                        </template>
                        <span>OID: {{ sensor.oid }}</span>
                      </v-tooltip>
                    </div>
                  </v-card>
                </v-col>
              </v-row>
            </v-card>

            <v-card v-if="hasCpuData || hasMemoryData" border flat class="mb-6 rounded-lg pa-4">
              <div
                class="text-subtitle-1 font-weight-bold mb-3 d-flex align-center ga-2"
                style="gap: 8px"
              >
                <v-icon color="primary">mdi-chip</v-icon>
                Monitoramento de Recursos da CPU & Memória
              </div>
              <v-row>
                <v-col v-if="hasCpuData" cols="12" md="6">
                  <v-switch
                    v-model="selectedCpuMonitor"
                    color="primary"
                    label="Monitorar Uso de CPU (%)"
                    hide-details
                  ></v-switch>
                  <div class="text-caption text-high-emphasis ml-8">
                    {{
                      detailStore.scanResult.cpuInfo.coresCount
                        ? `${detailStore.scanResult.cpuInfo.coresCount} núcleos detectados`
                        : 'Medição via MIB'
                    }}
                    <span v-if="detailStore.scanResult.cpuInfo.usagePercent != null">
                      - Uso Atual: {{ detailStore.scanResult.cpuInfo.usagePercent.toFixed(1) }}%
                    </span>
                  </div>
                </v-col>
                <v-col v-if="hasMemoryData" cols="12" md="6">
                  <v-switch
                    v-model="selectedMemoryMonitor"
                    color="primary"
                    label="Monitorar memória RAM"
                    hide-details
                  ></v-switch>
                  <div class="text-caption text-high-emphasis ml-8">
                    <span v-if="detailStore.scanResult.memoryInfo.usedKb != null">
                      Uso: {{ formatBinaryBytes(detailStore.scanResult.memoryInfo.usedKb * 1024) }}
                      <template v-if="detailStore.scanResult.memoryInfo.totalKb">
                        / {{ formatBinaryBytes(detailStore.scanResult.memoryInfo.totalKb * 1024) }}
                      </template>
                    </span>
                    <span v-if="detailStore.scanResult.memoryInfo.usedPercent != null" class="ml-1">
                      ({{ detailStore.scanResult.memoryInfo.usedPercent.toFixed(1) }}%)
                    </span>
                  </div>
                </v-col>
              </v-row>
            </v-card>

            <div class="d-flex align-center justify-space-between mb-3">
              <div
                class="text-subtitle-1 font-weight-bold d-flex align-center ga-2"
                style="gap: 8px"
              >
                <v-icon color="primary">mdi-ethernet-cable</v-icon>
                Interfaces de Rede Descobertas ({{ detailStore.scanResult.interfaces.length }})
              </div>
              <div class="d-flex align-center ga-2" style="gap: 8px">
                <v-btn
                  v-if="suggestedMonitorIndexes.length"
                  size="small"
                  variant="tonal"
                  color="primary"
                  prepend-icon="mdi-lightning-bolt-circle"
                  @click="selectSuggestedInterfaces"
                >
                  Selecionar sugeridas ({{ suggestedMonitorIndexes.length }})
                </v-btn>
                <v-btn size="small" variant="text" color="primary" @click="selectAllInterfaces">
                  Selecionar Todas
                </v-btn>
                <v-btn size="small" variant="text" color="primary" @click="unselectAllInterfaces">
                  Desmarcar Todas
                </v-btn>
              </div>
            </div>

            <div class="table-responsive">
              <v-table border hover class="rounded-lg">
                <thead>
                  <tr>
                    <th style="width: 50px">Monitorar</th>
                    <th>Index</th>
                    <th>Nome Interface</th>
                    <th>MAC Address</th>
                    <th>Velocidade</th>
                    <th>Status Operacional</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="iface in detailStore.scanResult.interfaces" :key="iface.ifIndex">
                    <td>
                      <v-checkbox
                        :model-value="selectedIfIndexes.includes(iface.ifIndex)"
                        color="primary"
                        hide-details
                        @update:model-value="toggleInterface(iface.ifIndex)"
                      ></v-checkbox>
                    </td>
                    <td>{{ iface.ifIndex }}</td>
                    <td>
                      <div class="font-weight-bold">{{ iface.ifName }}</div>
                      <div
                        v-if="iface.ifAlias && iface.ifAlias !== iface.ifName"
                        class="text-caption"
                      >
                        {{ iface.ifAlias }}
                      </div>
                      <div class="d-flex flex-wrap ga-1 mt-1">
                        <DeciderSuggestionChip
                          v-if="uplinkSuggestion?.value === String(iface.ifIndex)"
                          :suggestion="uplinkSuggestion"
                          :format-label="() => 'uplink'"
                          :action-label="
                            isCurrentUplink(iface.ifName) ? undefined : 'Usar como uplink'
                          "
                          @apply="useAsUplink(iface)"
                        />
                        <v-chip
                          v-if="monitorConfidence(iface.ifIndex) != null"
                          size="x-small"
                          color="primary"
                          variant="tonal"
                          prepend-icon="mdi-lightning-bolt-circle"
                        >
                          Vale monitorar · {{ formatPercent(monitorConfidence(iface.ifIndex), 0) }}
                        </v-chip>
                        <v-chip
                          v-if="isCurrentUplink(iface.ifName)"
                          size="x-small"
                          color="success"
                          variant="tonal"
                          prepend-icon="mdi-check-circle"
                        >
                          Uplink atual
                        </v-chip>
                      </div>
                    </td>
                    <td>{{ iface.macAddress || 'N/A' }}</td>
                    <td>
                      <v-chip size="x-small" variant="tonal" color="info">
                        {{ formatLinkSpeed(iface.ifSpeed) }}
                      </v-chip>
                    </td>
                    <td>
                      <v-chip
                        :color="iface.ifOperStatus === 'up' ? 'success' : 'error'"
                        size="x-small"
                      >
                        {{ iface.ifOperStatus ? iface.ifOperStatus.toUpperCase() : 'DOWN' }}
                      </v-chip>
                    </td>
                  </tr>
                  <tr v-if="detailStore.scanResult.interfaces.length === 0">
                    <td colspan="6" class="text-center text-high-emphasis py-6">
                      <div class="d-flex flex-column align-center ga-1">
                        <v-icon size="32" color="grey">mdi-ethernet-off</v-icon>
                        <div class="font-weight-medium">
                          Nenhuma interface de rede convencional detectada via SNMP.
                        </div>
                        <div class="text-caption">
                          Equipamentos como controladores solares MPPT, nobreaks e sensores IoT não
                          utilizam portas de rede para medição. O monitoramento é realizado através
                          da telemetria de sensores acima.
                        </div>
                      </div>
                    </td>
                  </tr>
                </tbody>
              </v-table>
            </div>
          </div>

          <div v-else-if="detailStore.error" class="py-4">
            <v-alert
              type="error"
              variant="tonal"
              class="mb-4 rounded-lg"
              prepend-icon="mdi-alert-circle"
              title="Falha na Comunicação SNMP"
            >
              <div class="mb-3 text-body-2 font-weight-medium">{{ detailStore.error }}</div>
              <v-divider class="my-2"></v-divider>
              <div class="text-caption">
                <strong>Diagnóstico e possíveis causas:</strong>
                <ul class="pl-4 mt-2">
                  <li>
                    O serviço SNMP pode estar desabilitado no equipamento ou restrito por regras de
                    Firewall/ACL.
                  </li>
                  <li>
                    A community SNMP configurada no cadastro deste dispositivo pode não coincidir
                    com a permitida no equipamento (padrão comum: <code>public</code>).
                  </li>
                  <li>
                    A porta 161 UDP pode estar inacessível ou filtrada na rota até o endereço
                    <code>{{ detailStore.device?.ipAddress }}</code
                    >.
                  </li>
                  <li>A versão SNMP (v1, v2c ou v3) difere da configurada no dispositivo.</li>
                </ul>
              </div>
            </v-alert>
            <div class="text-center">
              <v-btn color="primary" variant="outlined" prepend-icon="mdi-refresh" @click="runScan">
                Tentar Novamente
              </v-btn>
            </div>
          </div>
          <div v-else class="text-center py-8 text-high-emphasis">
            Nenhuma informação disponível para exibição.
          </div>
        </v-card-text>

        <v-divider></v-divider>

        <v-card-actions class="pa-4 justify-end">
          <v-btn variant="text" @click="open = false">Cancelar</v-btn>
          <v-btn
            color="primary"
            prepend-icon="mdi-check"
            :loading="savingMonitors"
            :disabled="!detailStore.scanResult || detailStore.scanningSnmp"
            @click="saveMonitors"
          >
            Salvar Configurações de Monitoramento
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <v-dialog
      v-model="snmpRemovalConfirmation"
      :fullscreen="$vuetify.display.xs"
      max-width="560"
      persistent
    >
      <v-card class="rounded-lg">
        <v-card-title class="font-weight-bold d-flex align-center ga-2">
          <v-icon color="warning">mdi-alert-circle-outline</v-icon>
          Remover do Monitoramento & Histórico?
        </v-card-title>
        <v-card-text>
          <p class="mb-3">
            Os seguintes itens foram desmarcados ou não constam mais na varredura SNMP do
            equipamento:
          </p>
          <v-list density="compact" class="bg-surface-light rounded-lg mb-3 pa-2">
            <v-list-item
              v-for="(item, idx) in removedItems"
              :key="idx"
              density="compact"
              prepend-icon="mdi-minus-circle-outline"
              :title="item"
            ></v-list-item>
          </v-list>
          <p class="text-body-2 text-high-emphasis mb-0">
            Deseja apagar o histórico de métricas, coletas e eventos dos itens removidos, ou deseja
            manter o histórico existente?
          </p>
        </v-card-text>
        <v-card-actions class="justify-end flex-wrap ga-2 pa-4">
          <v-btn variant="text" @click="snmpRemovalConfirmation = false">Cancelar</v-btn>
          <v-btn
            color="primary"
            variant="tonal"
            :loading="savingMonitors"
            @click="confirmSaveMonitors(false)"
          >
            Manter Histórico
          </v-btn>
          <v-btn
            color="error"
            variant="flat"
            :loading="savingMonitors"
            @click="confirmSaveMonitors(true)"
          >
            Apagar Histórico
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <!-- Modal de Perfis SNMP -->
    <SnmpProfilesDialog v-model="profilesDialogOpen" />
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import {
  interfaceLabel,
  scanInterfaceCandidates,
  useDeviceDetailStore,
  type DiscoveredSensorItem,
} from '@/stores/deviceDetail'
import { useDevicesStore } from '@/stores/devices'
import SnmpProfilesDialog from '@/components/devices/SnmpProfilesDialog.vue'
import DeciderSuggestionChip from '@/components/ai/DeciderSuggestionChip.vue'
import { formatBinaryBytes, formatLinkSpeed, formatPercent } from '@/utils/formatters'

/**
 * Varredura SNMP do equipamento: descobre interfaces, CPU/memória e sensores e
 * deixa o operador escolher o que monitorar. Abrir o diálogo já dispara a
 * varredura.
 */
const props = defineProps<{ deviceId: number }>()
const open = defineModel<boolean>({ default: false })

const detailStore = useDeviceDetailStore()
const devicesStore = useDevicesStore()
const deviceId = computed(() => props.deviceId)

watch(open, (aberto) => {
  if (aberto) void runScan()
})

const profilesDialogOpen = ref(false)
const selectedSensors = ref<string[]>([])

const savingMonitors = ref(false)

const selectedCpuMonitor = ref(false)
const selectedMemoryMonitor = ref(false)

const hasCpuData = computed(() => {
  const cpu = detailStore.scanResult?.cpuInfo
  return Boolean(cpu && cpu.usagePercent != null)
})
const hasMemoryData = computed(() => {
  const mem = detailStore.scanResult?.memoryInfo
  return Boolean(mem && (mem.usedPercent != null || mem.totalKb != null))
})
const selectedIfIndexes = ref<number[]>([])

async function runScan() {
  const res = await detailStore.scanDeviceSnmp(deviceId.value)
  if (res) {
    const cpuSupported = Boolean(res.cpuInfo && res.cpuInfo.usagePercent != null)
    const memSupported = Boolean(
      res.memoryInfo && (res.memoryInfo.usedPercent != null || res.memoryInfo.totalKb != null)
    )
    selectedCpuMonitor.value = cpuSupported && res.hasCpuMonitor
    selectedMemoryMonitor.value = memSupported && res.hasMemoryMonitor
    selectedIfIndexes.value = res.interfaces.filter((i) => i.isMonitored).map((i) => i.ifIndex)
    void detailStore.suggestInterfaces({
      deviceName: detailStore.device?.name ?? null,
      deviceType: detailStore.device?.type ?? null,
      sysDescr: res.systemInfo.sysDescr ?? null,
      interfaces: scanInterfaceCandidates(res),
    })
    const monitoredSensors = (res.sensors || []).filter((s) => s.isMonitored).map((s) => s.key)
    selectedSensors.value =
      monitoredSensors.length > 0 ? monitoredSensors : (res.sensors || []).map((s) => s.key)
  }
}

function toggleInterface(ifIndex: number) {
  const idx = selectedIfIndexes.value.indexOf(ifIndex)
  if (idx > -1) {
    selectedIfIndexes.value.splice(idx, 1)
  } else {
    selectedIfIndexes.value.push(ifIndex)
  }
}

function selectAllInterfaces() {
  if (detailStore.scanResult) {
    selectedIfIndexes.value = detailStore.scanResult.interfaces.map((i) => i.ifIndex)
  }
}

const uplinkSuggestion = computed(() => detailStore.interfaceSuggestions?.uplink ?? null)
const suggestedMonitorIndexes = computed(
  () => detailStore.interfaceSuggestions?.monitor.map((item) => item.ifIndex) ?? []
)

function monitorConfidence(ifIndex: number): number | null {
  return (
    detailStore.interfaceSuggestions?.monitor.find((item) => item.ifIndex === ifIndex)
      ?.confidence ?? null
  )
}

/** Soma as sugeridas ao que já estava marcado — nunca desmarca nada. */
function selectSuggestedInterfaces() {
  const merged = new Set([...selectedIfIndexes.value, ...suggestedMonitorIndexes.value])
  selectedIfIndexes.value = [...merged]
}

function isCurrentUplink(ifName: string): boolean {
  return detailStore.device?.linkInterfaceName === ifName
}

async function useAsUplink(iface: { ifIndex: number; ifName: string }) {
  const known = detailStore.interfaces.find(
    (item) => (item.ifIndex ?? item.snmpIndex) === iface.ifIndex
  )
  const updated = await devicesStore.updateDevice(deviceId.value, {
    linkInterfaceName: iface.ifName,
    linkInterfaceId: known?.id ?? null,
  })
  if (updated && detailStore.device) {
    detailStore.device.linkInterfaceName = updated.linkInterfaceName ?? iface.ifName
    detailStore.device.linkInterfaceId = updated.linkInterfaceId ?? known?.id ?? null
  }
}

function unselectAllInterfaces() {
  selectedIfIndexes.value = []
}

function toggleSensor(sensorKey: string) {
  const idx = selectedSensors.value.indexOf(sensorKey)
  if (idx > -1) {
    selectedSensors.value.splice(idx, 1)
  } else {
    selectedSensors.value.push(sensorKey)
  }
}

function selectAllSensors() {
  if (detailStore.scanResult?.sensors) {
    selectedSensors.value = detailStore.scanResult.sensors.map((s) => s.key)
  }
}

function unselectAllSensors() {
  selectedSensors.value = []
}

function formatSensorDisplay(sensor: DiscoveredSensorItem): string {
  // 1. Se o sensor possui estados declarados no perfil SNMP:
  const rawNum = sensor.rawValue ?? sensor.value
  if (sensor.states && rawNum != null && Number.isFinite(rawNum)) {
    const key = String(Math.round(rawNum))
    const s = sensor.states[key]
    if (s) {
      return typeof s === 'string' ? s : s.label
    }
  }

  // 2. Se já veio formatado pelo backend com texto descritivo:
  if (sensor.formattedValue && isNaN(Number(sensor.formattedValue))) {
    return sensor.formattedValue
  }

  // 3. Se for numérico, exibe o valor formatado com a unidade física do perfil:
  if (sensor.value != null && Number.isFinite(sensor.value)) {
    if (sensor.unit === 'kWh') {
      return `${sensor.value.toFixed(2)} kWh`
    }
    const valStr = sensor.value % 1 === 0 ? sensor.value.toFixed(0) : sensor.value.toFixed(1)
    return sensor.unit ? `${valStr} ${sensor.unit}` : valStr
  }

  return sensor.formattedValue || 'Sem leitura'
}

const snmpRemovalConfirmation = ref(false)

const removedItems = computed(() => {
  const items: string[] = []
  if (!detailStore.scanResult) return items

  const unselectedInterfaces = detailStore.interfaces.filter((i) => {
    const idx = i.snmpIndex ?? i.ifIndex
    return i.isMonitored && idx != null && !selectedIfIndexes.value.includes(idx)
  })
  for (const intf of unselectedInterfaces) {
    items.push(`Interface "${interfaceLabel(intf)}" (desmarcada do monitoramento)`)
  }

  const scanIndexes = new Set(detailStore.scanResult.interfaces.map((i) => i.ifIndex))
  const missingInterfaces = detailStore.interfaces.filter((i) => {
    const idx = i.snmpIndex ?? i.ifIndex
    return idx != null && !scanIndexes.has(idx)
  })
  for (const intf of missingInterfaces) {
    if (!unselectedInterfaces.includes(intf)) {
      items.push(`Interface "${interfaceLabel(intf)}" (não encontrada no dispositivo via SNMP)`)
    }
  }

  if (detailStore.scanResult.hasCpuMonitor && !selectedCpuMonitor.value) {
    items.push('Monitor de Uso de CPU (desmarcado)')
  }
  if (detailStore.scanResult.hasMemoryMonitor && !selectedMemoryMonitor.value) {
    items.push('Monitor de Uso de Memória (desmarcado)')
  }

  if (detailStore.scanResult.sensors) {
    for (const sensor of detailStore.scanResult.sensors) {
      if (sensor.isMonitored && !selectedSensors.value.includes(sensor.key)) {
        items.push(`Sensor "${sensor.label}" (desmarcado do monitoramento)`)
      }
    }
  }

  return items
})

async function saveMonitors() {
  if (removedItems.value.length > 0) {
    snmpRemovalConfirmation.value = true
    return
  }
  await doApplyMonitors(false)
}

async function confirmSaveMonitors(clearHistory: boolean) {
  snmpRemovalConfirmation.value = false
  await doApplyMonitors(clearHistory)
}

async function doApplyMonitors(clearRemovedHistory: boolean) {
  savingMonitors.value = true
  try {
    const success = await detailStore.applySnmpMonitors(deviceId.value, {
      enableCpuMonitor: hasCpuData.value ? selectedCpuMonitor.value : false,
      enableMemoryMonitor: hasMemoryData.value ? selectedMemoryMonitor.value : false,
      monitoredIfIndexes: selectedIfIndexes.value,
      monitoredSensors: selectedSensors.value,
      clearRemovedHistory,
    })
    if (success) {
      open.value = false
    }
  } finally {
    savingMonitors.value = false
  }
}
</script>
