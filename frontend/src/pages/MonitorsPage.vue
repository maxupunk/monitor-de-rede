<template>
  <div>
    <div class="d-flex align-center justify-space-between mb-6">
      <div>
        <h1 class="text-h4 font-weight-bold">Monitores de Rede</h1>
        <p class="text-subtitle-1 text-grey-darken-1">
          Gerenciamento de verificações ICMP (Ping), HTTP, TCP e DNS com histórico em linha do tempo
        </p>
      </div>
      <v-btn color="primary" prepend-icon="mdi-plus" @click="openDialog()"> Novo Monitor </v-btn>
    </div>

    <!-- Tabela de Monitores -->
    <v-card elevation="2" class="rounded-lg">
      <v-card-title class="pa-4 d-flex align-center ga-4">
        <v-text-field
          v-model="search"
          prepend-inner-icon="mdi-magnify"
          label="Buscar por nome, tipo ou alvo"
          single-line
          hide-details
          variant="outlined"
          density="compact"
          class="max-w-300"
        ></v-text-field>
      </v-card-title>

      <v-data-table
        :headers="headers"
        :items="monitorsStore.monitors"
        :search="search"
        :loading="monitorsStore.loading"
        no-data-text="Nenhum monitor cadastrado"
      >
        <!-- Custom Slot para Nome, Dispositivo e Linha do Tempo/Uso -->
        <template #item.name="{ item }">
          <div class="py-2">
            <router-link
              :to="`/monitors/${item.id}`"
              class="text-subtitle-1 font-weight-bold text-decoration-none text-primary hover-underline d-inline-block"
            >
              {{ item.name }}
            </router-link>
            <div class="text-caption text-grey-darken-1 mb-1 d-flex align-center ga-1">
              <v-icon size="12">mdi-router-network</v-icon>
              {{ item.device?.name || 'Dispositivo não vinculado' }}
            </div>

            <!-- Monitores de uso (CPU/Memória via SNMP) não são checagens up/down: mostramos a leitura atual -->
            <div
              v-if="isGaugeMonitor(item)"
              class="d-flex align-center ga-2"
              style="max-width: 220px"
            >
              <v-progress-linear
                :model-value="item.gaugeMetric?.value ?? 0"
                height="10"
                rounded
                :color="gaugeColor(item)"
                style="flex: 1"
              ></v-progress-linear>
              <span class="text-caption font-weight-medium" style="min-width: 34px">
                {{ item.gaugeMetric ? `${Math.round(item.gaugeMetric.value)}%` : 'N/D' }}
              </span>
            </div>
            <template v-else>
              <!-- Interface de rede: up/down não conta a história toda, então mostramos a velocidade negociada -->
              <div v-if="isInterfaceMonitor(item)" class="text-caption mb-1">
                <v-icon size="13" :color="interfaceStatusInfo(item).color">
                  {{ interfaceStatusInfo(item).icon }}
                </v-icon>
                {{ interfaceStatusInfo(item).label }}
              </div>
              <router-link :to="`/monitors/${item.id}`" class="text-decoration-none">
                <MonitorTimelineBar
                  :results="item.recentResults"
                  :max-blocks="24"
                  :height="20"
                  :width="5"
                />
              </router-link>
            </template>
          </div>
        </template>

        <template #item.type="{ item }">
          <v-chip size="small" color="info" variant="tonal">
            {{
              isGaugeMonitor(item)
                ? gaugeTypeLabel(item)
                : isInterfaceMonitor(item)
                  ? 'INTERFACE'
                  : (item.type || 'N/A').toUpperCase()
            }}
          </v-chip>
        </template>

        <template #item.status="{ item }">
          <div class="d-flex flex-column align-start py-1">
            <v-chip v-if="isGaugeMonitor(item)" :color="gaugeColor(item)" size="small">
              {{ item.gaugeMetric ? `${Math.round(item.gaugeMetric.value)}%` : 'SEM DADOS' }}
            </v-chip>
            <v-chip
              v-else-if="isInterfaceMonitor(item)"
              :color="interfaceStatusInfo(item).color"
              size="small"
            >
              <v-icon start size="14">{{ interfaceStatusInfo(item).icon }}</v-icon>
              {{ interfaceStatusInfo(item).label }}
            </v-chip>
            <v-chip v-else :color="getStatusColor(item.status)" size="small">
              {{ (item.status || 'UNKNOWN').toUpperCase() }}
            </v-chip>
            <span v-if="!item.isEnabled" class="text-caption text-grey-darken-1 mt-1 font-italic">
              Última informação
              <v-tooltip activator="parent" location="top">
                Monitor desativado - exibindo última informação registrada
              </v-tooltip>
            </span>
          </div>
        </template>

        <template #item.isEnabled="{ item }">
          <v-switch
            :model-value="item.isEnabled"
            color="success"
            hide-details
            density="compact"
            @update:model-value="(val) => monitorsStore.toggleMonitorEnabled(item.id, Boolean(val))"
          ></v-switch>
        </template>

        <template #item.actions="{ item }">
          <v-btn
            size="small"
            color="primary"
            variant="outlined"
            prepend-icon="mdi-play"
            class="mr-1"
            :loading="monitorsStore.runningId === item.id"
            @click="monitorsStore.runMonitor(item.id)"
          >
            Testar
          </v-btn>

          <v-btn icon size="small" variant="text" color="info" :to="`/monitors/${item.id}`">
            <v-icon>mdi-chart-timeline-variant</v-icon>
            <v-tooltip activator="parent" location="top">Ver Gráficos e Detalhes</v-tooltip>
          </v-btn>

          <v-btn icon size="small" variant="text" color="primary" @click="openDialog(item)">
            <v-icon>mdi-pencil</v-icon>
          </v-btn>
          <v-btn icon size="small" variant="text" color="error" @click="confirmDelete(item.id)">
            <v-icon>mdi-delete</v-icon>
          </v-btn>
        </template>
      </v-data-table>
    </v-card>

    <!-- Modal Form de Monitor -->
    <v-dialog v-model="dialog" max-width="550">
      <v-card class="rounded-lg pa-4">
        <v-card-title class="font-weight-bold">
          {{ editedId ? 'Editar Monitor' : 'Cadastrar Novo Monitor' }}
        </v-card-title>
        <v-card-text>
          <v-form @submit.prevent="save">
            <v-select
              v-model="formModel.deviceId"
              :items="devicesStore.devices"
              item-title="name"
              item-value="id"
              label="Dispositivo Associado"
              variant="outlined"
              required
            ></v-select>
            <v-text-field
              v-model="formModel.name"
              label="Nome do Monitor"
              placeholder="Ex: Ping Google DNS"
              variant="outlined"
              required
            ></v-text-field>
            <v-select
              v-model="formModel.type"
              :items="['ping', 'http', 'tcp', 'dns']"
              label="Tipo de Checagem"
              variant="outlined"
              required
            ></v-select>
            <v-text-field
              v-model="formModel.target"
              label="Alvo (IP / Hostname / URL)"
              placeholder="8.8.8.8 ou https://meusite.com"
              variant="outlined"
              required
            ></v-text-field>
            <v-text-field
              v-if="formModel.type === 'tcp'"
              v-model.number="formModel.port"
              label="Porta TCP"
              type="number"
              variant="outlined"
            ></v-text-field>
            <v-row>
              <v-col cols="6">
                <v-text-field
                  v-model.number="formModel.intervalSeconds"
                  label="Intervalo (s)"
                  type="number"
                  variant="outlined"
                ></v-text-field>
              </v-col>
              <v-col cols="6">
                <v-text-field
                  v-model.number="formModel.timeoutSeconds"
                  label="Timeout (s)"
                  type="number"
                  variant="outlined"
                ></v-text-field>
              </v-col>
            </v-row>
          </v-form>
        </v-card-text>
        <v-card-actions class="justify-end">
          <v-btn variant="text" @click="dialog = false">Cancelar</v-btn>
          <v-btn color="primary" :loading="saving" @click="save">Salvar</v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, reactive } from 'vue'
import { useMonitorsStore, type Monitor } from '@/stores/monitors'
import { useDevicesStore } from '@/stores/devices'
import MonitorTimelineBar from '@/components/MonitorTimelineBar.vue'
import {
  isGaugeMonitor,
  gaugeTypeLabel,
  gaugeMetricName,
  gaugeColor as gaugeColorFor,
  isInterfaceMonitor,
  interfaceStatusInfo as interfaceStatusInfoFor,
  latestResultData,
} from '@/utils/monitorPresentation'

const monitorsStore = useMonitorsStore()
const devicesStore = useDevicesStore()
const search = ref('')
const dialog = ref(false)
const saving = ref(false)
const editedId = ref<number | null>(null)

const formModel = reactive<{
  deviceId: number
  name: string
  type: 'ping' | 'http' | 'tcp' | 'dns'
  target: string
  port?: number
  intervalSeconds: number
  timeoutSeconds: number
}>({
  deviceId: 1,
  name: '',
  type: 'ping',
  target: '',
  port: 80,
  intervalSeconds: 60,
  timeoutSeconds: 5,
})

const headers = [
  { title: 'ID', key: 'id', width: '60px' },
  { title: 'Nome, Dispositivo e Histórico', key: 'name' },
  { title: 'Tipo', key: 'type', width: '90px' },
  { title: 'Alvo', key: 'target' },
  { title: 'Status', key: 'status', width: '100px' },
  { title: 'Ativo', key: 'isEnabled', width: '80px' },
  { title: 'Ações', key: 'actions', sortable: false, width: '220px' },
]

onMounted(async () => {
  await Promise.all([monitorsStore.fetchMonitors(), devicesStore.fetchDevices()])
})

function getStatusColor(status: string) {
  switch (status) {
    case 'up':
    case 'online':
      return 'success'
    case 'down':
    case 'offline':
      return 'error'
    case 'warning':
      return 'warning'
    default:
      return 'grey'
  }
}

function gaugeColor(item: Monitor): string {
  return gaugeColorFor(item.gaugeMetric?.value ?? null, gaugeMetricName(item))
}

function interfaceStatusInfo(item: Monitor) {
  return interfaceStatusInfoFor(item.status, latestResultData(item.recentResults))
}

function openDialog(monitor?: Monitor) {
  if (monitor) {
    editedId.value = monitor.id
    formModel.deviceId = monitor.deviceId
    formModel.name = monitor.name
    formModel.type = (monitor.type as 'ping' | 'http' | 'tcp' | 'dns') || 'ping'
    formModel.target =
      monitor.target ||
      ((monitor.configuration?.host ||
        monitor.configuration?.url ||
        monitor.configuration?.domain ||
        '') as string)
    formModel.port = monitor.port || (monitor.configuration?.port as number | undefined) || 80
    formModel.intervalSeconds = monitor.intervalSeconds
    formModel.timeoutSeconds = monitor.timeoutSeconds
  } else {
    editedId.value = null
    formModel.deviceId = devicesStore.devices[0]?.id || 1
    formModel.name = ''
    formModel.type = 'ping'
    formModel.target = ''
    formModel.port = 80
    formModel.intervalSeconds = 60
    formModel.timeoutSeconds = 5
  }
  dialog.value = true
}

async function save() {
  if (!formModel.name || !formModel.target) return
  saving.value = true

  let configuration: Record<string, unknown> = {}
  if (formModel.type === 'ping') {
    configuration = { host: formModel.target }
  } else if (formModel.type === 'http') {
    configuration = {
      url: formModel.target.startsWith('http') ? formModel.target : `http://${formModel.target}`,
    }
  } else if (formModel.type === 'tcp') {
    configuration = { host: formModel.target, port: formModel.port || 80 }
  } else if (formModel.type === 'dns') {
    configuration = { domain: formModel.target }
  }

  const payload = {
    ...formModel,
    configuration,
  }

  if (editedId.value) {
    await monitorsStore.updateMonitor(editedId.value, payload)
  } else {
    await monitorsStore.createMonitor(payload)
  }
  saving.value = false
  dialog.value = false
}

async function confirmDelete(id: number) {
  if (confirm('Tem certeza de que deseja excluir este monitor?')) {
    await monitorsStore.deleteMonitor(id)
  }
}
</script>

<style scoped>
.hover-underline:hover {
  text-decoration: underline !important;
}
</style>
