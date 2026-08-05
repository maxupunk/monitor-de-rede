<template>
  <div>
    <div class="d-flex align-center justify-space-between mb-6">
      <div>
        <h1 class="text-h4 font-weight-bold">Monitores de Rede</h1>
        <p class="text-subtitle-1 text-grey-darken-1">
          Verificações de Ping, HTTP, TCP, DNS e SNMP com histórico em linha do tempo
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
        :items-per-page="-1"
        hide-default-footer
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
          <v-chip size="small" :color="typeChip(item).color" variant="tonal">
            <v-icon start size="14">{{ typeChip(item).icon }}</v-icon>
            {{ typeChip(item).label }}
          </v-chip>
        </template>

        <template #item.target="{ item }">
          <span class="text-body-2">{{ formatTarget(item) }}</span>
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
    <MonitorFormDialog
      v-model="dialog"
      :monitor="editingMonitor"
      @saved="onSaved"
    ></MonitorFormDialog>

    <!-- Confirmação de exclusão -->
    <v-dialog v-model="deleteDialog" max-width="440">
      <v-card class="rounded-lg pa-2">
        <v-card-item>
          <template #prepend>
            <v-avatar color="error" variant="tonal" rounded="lg">
              <v-icon>mdi-delete-alert-outline</v-icon>
            </v-avatar>
          </template>
          <v-card-title class="font-weight-bold">Excluir monitor</v-card-title>
        </v-card-item>
        <v-card-text>
          O monitor <strong>{{ monitorToDelete?.name }}</strong> e todo o seu histórico de
          verificações serão removidos permanentemente. Para apenas parar as checagens, desative-o
          na coluna "Ativo".
        </v-card-text>
        <v-card-actions class="justify-end">
          <v-btn variant="text" @click="deleteDialog = false">Cancelar</v-btn>
          <v-btn color="error" variant="flat" :loading="deleting" @click="executeDelete">
            Excluir
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useMonitorsStore, type Monitor } from '@/stores/monitors'
import { useDevicesStore } from '@/stores/devices'
import MonitorTimelineBar from '@/components/MonitorTimelineBar.vue'
import MonitorFormDialog from '@/components/MonitorFormDialog.vue'
import {
  isGaugeMonitor,
  gaugeMetricName,
  gaugeColor as gaugeColorFor,
  isInterfaceMonitor,
  interfaceStatusInfo as interfaceStatusInfoFor,
  latestResultData,
} from '@/utils/monitorPresentation'
import { monitorKind, resolveKind, resolveSnmpMode, SNMP_MODES } from '@/utils/monitorTypes'

const monitorsStore = useMonitorsStore()
const devicesStore = useDevicesStore()
const search = ref('')
const dialog = ref(false)
const editingMonitor = ref<Monitor | null>(null)
const deleteDialog = ref(false)
const deleting = ref(false)
const monitorToDelete = ref<Monitor | null>(null)

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

/**
 * O chip de tipo usa o mesmo catálogo do formulário, com o detalhe de que
 * monitores SNMP se desdobram em leituras diferentes (CPU, memória, interface).
 */
function typeChip(item: Monitor): { label: string; icon: string; color: string } {
  const definition = monitorKind(resolveKind(item.type))

  if (item.type === 'snmp') {
    const mode = resolveSnmpMode(item.configuration)
    const modeDefinition = SNMP_MODES.find((m) => m.value === mode)
    if (mode !== 'availability' && modeDefinition) {
      return {
        label:
          mode === 'interface' ? 'INTERFACE' : isGaugeMonitor(item) ? gaugeLabel(item) : 'SNMP',
        icon: modeDefinition.icon,
        color: definition.color,
      }
    }
  }

  return { label: definition.short, icon: definition.icon, color: definition.color }
}

function gaugeLabel(item: Monitor): string {
  return gaugeMetricName(item) === 'memory_usage' ? 'MEMÓRIA' : 'CPU'
}

function formatTarget(item: Monitor): string {
  const config = item.configuration || {}
  if (item.type === 'tcp') {
    const port = item.port ?? (config.port as number | undefined)
    return port ? `${item.target}:${port}` : item.target
  }
  if (item.type === 'dns') {
    const recordType = (config.recordType as string) || 'A'
    return `${item.target} (${recordType})`
  }
  return item.target || '—'
}

function openDialog(monitor?: Monitor) {
  editingMonitor.value = monitor ?? null
  dialog.value = true
}

async function onSaved() {
  await monitorsStore.fetchMonitors()
}

function confirmDelete(id: number) {
  monitorToDelete.value = monitorsStore.monitors.find((m) => m.id === id) ?? null
  deleteDialog.value = true
}

async function executeDelete() {
  if (!monitorToDelete.value) return
  deleting.value = true
  try {
    await monitorsStore.deleteMonitor(monitorToDelete.value.id)
    deleteDialog.value = false
    monitorToDelete.value = null
  } finally {
    deleting.value = false
  }
}
</script>

<style scoped>
.hover-underline:hover {
  text-decoration: underline !important;
}
</style>
