<template>
  <div>
    <!-- Botão de Voltar -->
    <v-btn variant="text" prepend-icon="mdi-arrow-left" class="mb-4" to="/devices">
      Voltar para Dispositivos
    </v-btn>

    <!-- Header do Dispositivo -->
    <v-card elevation="2" class="rounded-lg pa-4 mb-6">
      <div class="d-flex align-center justify-space-between flex-wrap gap-4">
        <div class="d-flex align-center ga-4" style="gap: 16px">
          <v-avatar color="primary" size="48" class="mr-2">
            <v-icon color="white">mdi-router-network</v-icon>
          </v-avatar>
          <div>
            <div class="d-flex align-center ga-3 flex-wrap" style="gap: 12px">
              <h1 class="text-h5 font-weight-bold mr-2">
                {{ detailStore.device?.name || `Dispositivo #${deviceId}` }}
              </h1>
              <v-chip
                :color="getStatusColor(detailStore.device?.status)"
                size="small"
                variant="tonal"
                class="px-3"
              >
                <v-icon start size="14">mdi-circle</v-icon>
                {{ (detailStore.device?.status || 'UNKNOWN').toUpperCase() }}
              </v-chip>
            </div>
            <div class="text-subtitle-2 text-grey mt-1">
              IP: {{ detailStore.device?.ipAddress || 'Não informado' }} | Tipo:
              {{ detailStore.device?.type }} | Fabricante:
              {{ detailStore.device?.vendor || 'Desconhecido' }}
            </div>
          </div>
        </div>

        <div class="d-flex align-center ga-3" style="gap: 12px">
          <v-btn
            color="info"
            variant="tonal"
            prepend-icon="mdi-radar"
            :loading="detailStore.scanningSnmp"
            @click="openScanModal"
          >
            Escanear SNMP
          </v-btn>

          <v-btn
            color="secondary"
            prepend-icon="mdi-refresh"
            :loading="detailStore.pollingSnmp"
            @click="detailStore.triggerSnmpPoll(deviceId)"
          >
            Poll SNMP Agora
          </v-btn>
        </div>
      </div>
    </v-card>

    <!-- Abas Interativas -->
    <v-card elevation="2" class="rounded-lg">
      <v-tabs v-model="activeTab" color="primary" align-tabs="title">
        <v-tab value="overview" prepend-icon="mdi-information-outline">Visão Geral</v-tab>
        <v-tab value="monitors" prepend-icon="mdi-heart-pulse">
          Monitores ({{ detailStore.monitors.length }})
        </v-tab>
        <v-tab value="interfaces" prepend-icon="mdi-expansion-card">
          Interfaces SNMP ({{ detailStore.interfaces.length }})
        </v-tab>
        <v-tab value="metrics" prepend-icon="mdi-chart-line">Métricas & Tráfego</v-tab>
        <v-tab value="events" prepend-icon="mdi-history">Histórico de Eventos</v-tab>
      </v-tabs>

      <v-divider></v-divider>

      <v-card-text class="pa-6">
        <v-window v-model="activeTab">
          <!-- Aba Visão Geral -->
          <v-window-item value="overview">
            <v-row>
              <v-col cols="12" md="6">
                <v-list border class="rounded-lg">
                  <v-list-item title="Nome" :subtitle="detailStore.device?.name"></v-list-item>
                  <v-list-item
                    title="Endereço IP"
                    :subtitle="detailStore.device?.ipAddress"
                  ></v-list-item>
                  <v-list-item
                    title="Endereço MAC"
                    :subtitle="detailStore.device?.macAddress || 'Não cadastrado'"
                  ></v-list-item>
                  <v-list-item
                    title="Fabricante / Modelo"
                    :subtitle="`${detailStore.device?.vendor || 'N/A'} - ${detailStore.device?.model || 'N/A'}`"
                  ></v-list-item>
                </v-list>
              </v-col>
              <v-col cols="12" md="6">
                <v-list border class="rounded-lg">
                  <v-list-item
                    title="SNMP Habilitado"
                    :subtitle="detailStore.device?.snmpEnabled ? 'Sim' : 'Não'"
                  ></v-list-item>
                  <v-list-item
                    title="Versão / Comunidade SNMP"
                    :subtitle="`${detailStore.device?.snmpVersion || 'v2c'} / ${detailStore.device?.snmpCommunity || 'public'}`"
                  ></v-list-item>
                  <v-list-item
                    title="Data de Cadastro"
                    :subtitle="detailStore.device?.createdAt || 'Desconhecida'"
                  ></v-list-item>
                </v-list>
              </v-col>
            </v-row>
          </v-window-item>

          <!-- Aba Monitores -->
          <v-window-item value="monitors">
            <v-table hover>
              <thead>
                <tr>
                  <th>Nome</th>
                  <th>Tipo</th>
                  <th>Alvo / Porta</th>
                  <th>Intervalo</th>
                  <th>Status</th>
                  <th>Última Latência</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="mon in detailStore.monitors" :key="mon.id">
                  <td class="font-weight-bold">{{ mon.name }}</td>
                  <td>
                    <v-chip size="x-small" color="info">
                      {{ (mon.type || 'N/A').toUpperCase() }}
                    </v-chip>
                  </td>
                  <td>{{ mon.target }} {{ mon.port ? `:${mon.port}` : '' }}</td>
                  <td>{{ mon.intervalSeconds }}s</td>
                  <td>
                    <v-chip :color="getStatusColor(mon.status)" size="x-small">
                      {{ mon.status || 'UNKNOWN' }}
                    </v-chip>
                  </td>
                  <td>{{ mon.latencyMs !== undefined ? `${mon.latencyMs} ms` : 'N/A' }}</td>
                </tr>
                <tr v-if="detailStore.monitors.length === 0">
                  <td colspan="6" class="text-center text-grey py-4">
                    Nenhum monitor configurado para este equipamento. Clique em "Escanear SNMP".
                  </td>
                </tr>
              </tbody>
            </v-table>
          </v-window-item>

          <!-- Aba Interfaces SNMP -->
          <v-window-item value="interfaces">
            <v-table hover>
              <thead>
                <tr>
                  <th>Index</th>
                  <th>Nome Interface</th>
                  <th>Status Admin / Oper</th>
                  <th>MAC Address</th>
                  <th>IP Address</th>
                  <th>Velocidade (bps)</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="intf in detailStore.interfaces" :key="intf.id">
                  <td>{{ intf.ifIndex ?? intf.snmpIndex ?? '-' }}</td>
                  <td class="font-weight-bold">{{ intf.ifName || intf.name || '-' }}</td>
                  <td>
                    <v-chip
                      :color="(intf.ifOperStatus || intf.operStatus) === 'up' ? 'success' : 'error'"
                      size="x-small"
                      class="mr-1"
                    >
                      Oper: {{ intf.ifOperStatus || intf.operStatus || 'unknown' }}
                    </v-chip>
                  </td>
                  <td>{{ intf.macAddress || 'N/A' }}</td>
                  <td>{{ intf.ipAddress || 'N/A' }}</td>
                  <td>
                    {{
                      intf.ifSpeed || intf.speed
                        ? `${((intf.ifSpeed || intf.speed)! / 1000000).toFixed(0)} Mbps`
                        : 'N/A'
                    }}
                  </td>
                </tr>
                <tr v-if="detailStore.interfaces.length === 0">
                  <td colspan="6" class="text-center text-grey py-4">
                    Nenhuma interface SNMP encontrada. Clique em "Escanear SNMP".
                  </td>
                </tr>
              </tbody>
            </v-table>
          </v-window-item>

          <!-- Aba Métricas -->
          <v-window-item value="metrics">
            <v-row>
              <v-col v-for="met in detailStore.metrics" :key="met.id" cols="12" sm="6" md="4">
                <v-card border flat class="pa-4 rounded-lg">
                  <div class="text-caption text-grey">{{ met.metricName }}</div>
                  <div class="text-h4 font-weight-bold my-1">
                    {{ met.metricValue }} <span class="text-caption">{{ met.unit || '' }}</span>
                  </div>
                  <div class="text-caption text-grey">{{ met.createdAt }}</div>
                </v-card>
              </v-col>
              <v-col v-if="detailStore.metrics.length === 0" cols="12">
                <div class="text-center text-grey py-6">Nenhuma métrica coletada recentemente.</div>
              </v-col>
            </v-row>
          </v-window-item>

          <!-- Aba Eventos -->
          <v-window-item value="events">
            <v-table hover>
              <thead>
                <tr>
                  <th>Severidade</th>
                  <th>Mensagem</th>
                  <th>Data/Hora</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="evt in detailStore.events" :key="evt.id">
                  <td>
                    <v-chip
                      :color="
                        evt.severity === 'critical' || evt.severity === 'error'
                          ? 'error'
                          : 'warning'
                      "
                      size="x-small"
                    >
                      {{ (evt.severity || 'INFO').toUpperCase() }}
                    </v-chip>
                  </td>
                  <td>{{ evt.message }}</td>
                  <td>{{ evt.createdAt }}</td>
                </tr>
                <tr v-if="detailStore.events.length === 0">
                  <td colspan="3" class="text-center text-grey py-4">
                    Nenhum evento registrado para este dispositivo.
                  </td>
                </tr>
              </tbody>
            </v-table>
          </v-window-item>
        </v-window>
      </v-card-text>
    </v-card>

    <!-- Modal de Escaneamento SNMP -->
    <v-dialog v-model="scanModalOpen" max-width="850px">
      <v-card class="rounded-lg">
        <v-card-title class="d-flex align-center justify-space-between pa-4 bg-primary text-white">
          <div class="d-flex align-center ga-2" style="gap: 8px">
            <v-icon>mdi-radar</v-icon>
            <span>Escaneamento & Descoberta SNMP (OpenWrt)</span>
          </div>
          <v-btn icon variant="text" color="white" @click="scanModalOpen = false">
            <v-icon>mdi-close</v-icon>
          </v-btn>
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
            <div class="text-caption text-grey">
              Consultando interfaces, uso de CPU e memória RAM...
            </div>
          </div>

          <div v-else-if="detailStore.scanResult">
            <!-- Dados do Sistema -->
            <v-alert
              type="info"
              variant="tonal"
              class="mb-4"
              prepend-icon="mdi-router"
              title="Dispositivo Conectado"
              :subtitle="detailStore.scanResult.systemInfo.sysDescr || 'OpenWrt / Router Device'"
            ></v-alert>

            <!-- Recursos de CPU & Memória -->
            <v-card variant="outlined" class="mb-6 rounded-lg pa-4">
              <div
                class="text-subtitle-1 font-weight-bold mb-3 d-flex align-center ga-2"
                style="gap: 8px"
              >
                <v-icon color="primary">mdi-chip</v-icon>
                Monitoramento de Recursos da CPU & Memória
              </div>
              <v-row>
                <v-col cols="12" md="6">
                  <v-switch
                    v-model="selectedCpuMonitor"
                    color="primary"
                    label="Monitorar Uso de CPU (%)"
                    hide-details
                  ></v-switch>
                  <div class="text-caption text-grey ml-8">
                    {{
                      detailStore.scanResult.cpuInfo.coresCount
                        ? `${detailStore.scanResult.cpuInfo.coresCount} núcleos detectados`
                        : 'Medição via MIB'
                    }}
                    <span v-if="detailStore.scanResult.cpuInfo.usagePercent !== undefined">
                      - Uso Atual: {{ detailStore.scanResult.cpuInfo.usagePercent }}%
                    </span>
                  </div>
                </v-col>
                <v-col cols="12" md="6">
                  <v-switch
                    v-model="selectedMemoryMonitor"
                    color="primary"
                    label="Monitorar Memória RAM (%)"
                    hide-details
                  ></v-switch>
                  <div class="text-caption text-grey ml-8">
                    <span v-if="detailStore.scanResult.memoryInfo.totalKb">
                      Total: {{ Math.round(detailStore.scanResult.memoryInfo.totalKb / 1024) }} MB
                    </span>
                    <span v-if="detailStore.scanResult.memoryInfo.usedPercent !== undefined">
                      - Uso: {{ detailStore.scanResult.memoryInfo.usedPercent }}%
                    </span>
                  </div>
                </v-col>
              </v-row>
            </v-card>

            <!-- Lista de Interfaces Descobertas -->
            <div class="d-flex align-center justify-space-between mb-3">
              <div
                class="text-subtitle-1 font-weight-bold d-flex align-center ga-2"
                style="gap: 8px"
              >
                <v-icon color="primary">mdi-ethernet-cable</v-icon>
                Interfaces de Rede Descobertas ({{ detailStore.scanResult.interfaces.length }})
              </div>
              <div class="d-flex align-center ga-2" style="gap: 8px">
                <v-btn size="small" variant="text" color="primary" @click="selectAllInterfaces">
                  Selecionar Todas
                </v-btn>
                <v-btn size="small" variant="text" color="grey" @click="unselectAllInterfaces">
                  Desmarcar Todas
                </v-btn>
              </div>
            </div>

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
                  <td class="font-weight-bold">{{ iface.ifName }}</td>
                  <td>{{ iface.macAddress || 'N/A' }}</td>
                  <td>
                    {{ iface.ifSpeed ? `${(iface.ifSpeed / 1000000).toFixed(0)} Mbps` : 'N/A' }}
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
                  <td colspan="6" class="text-center text-grey py-4">
                    Nenhuma interface respondeu na varredura SNMP.
                  </td>
                </tr>
              </tbody>
            </v-table>
          </div>
        </v-card-text>

        <v-divider></v-divider>

        <v-card-actions class="pa-4 justify-end">
          <v-btn variant="text" @click="scanModalOpen = false">Cancelar</v-btn>
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
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { useRoute } from 'vue-router'
import { useDeviceDetailStore } from '@/stores/deviceDetail'

const route = useRoute()
const detailStore = useDeviceDetailStore()
const activeTab = ref('overview')
const scanModalOpen = ref(false)
const savingMonitors = ref(false)

const selectedCpuMonitor = ref(true)
const selectedMemoryMonitor = ref(true)
const selectedIfIndexes = ref<number[]>([])

const deviceId = computed(() => Number(route.params.id))

onMounted(() => {
  if (deviceId.value) {
    detailStore.loadDeviceDetails(deviceId.value)
  }
})

async function openScanModal() {
  scanModalOpen.value = true
  const res = await detailStore.scanDeviceSnmp(deviceId.value)
  if (res) {
    selectedCpuMonitor.value = res.hasCpuMonitor || true
    selectedMemoryMonitor.value = res.hasMemoryMonitor || true
    selectedIfIndexes.value = res.interfaces.filter((i) => i.isMonitored).map((i) => i.ifIndex)
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

function unselectAllInterfaces() {
  selectedIfIndexes.value = []
}

async function saveMonitors() {
  savingMonitors.value = true
  try {
    const success = await detailStore.applySnmpMonitors(deviceId.value, {
      enableCpuMonitor: selectedCpuMonitor.value,
      enableMemoryMonitor: selectedMemoryMonitor.value,
      monitoredIfIndexes: selectedIfIndexes.value,
    })
    if (success) {
      scanModalOpen.value = false
    }
  } finally {
    savingMonitors.value = false
  }
}

function getStatusColor(status?: string) {
  switch (status?.toLowerCase()) {
    case 'online':
    case 'up':
      return 'success'
    case 'offline':
    case 'down':
      return 'error'
    case 'warning':
      return 'warning'
    default:
      return 'grey'
  }
}
</script>
