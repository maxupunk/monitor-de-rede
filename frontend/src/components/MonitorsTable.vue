<template>
  <ResponsiveDataTable
    :headers="headers"
    :items="monitors"
    :search="search"
    :loading="loading"
    :items-per-page="-1"
    hide-default-footer
    :no-data-text="noDataText"
    :clickable="false"
  >
    <!-- Nome, dispositivo e histórico -->
    <template #item.name="{ item }">
      <div class="py-2">
        <router-link
          :to="`/monitors/${item.id}`"
          class="text-subtitle-1 font-weight-bold text-decoration-none text-primary hover-underline d-block mb-1"
        >
          {{ item.name }}
        </router-link>
        <div
          v-if="showDevice"
          class="text-caption text-grey-darken-1 mb-1 d-flex align-center ga-1"
        >
          <v-icon size="12">mdi-router-network</v-icon>
          {{ item.device?.name || 'Dispositivo não vinculado' }}
        </div>

        <!-- Monitores de uso (CPU/Memória via SNMP) não são checagens up/down: mostramos a leitura atual -->
        <div v-if="isGaugeMonitor(item)" class="d-flex align-center ga-2" style="max-width: 260px">
          <!-- Largura igual à da MonitorTimelineBar abaixo (24 blocos de 5px + 23 gaps de 3px = 189px),
               para os dois estilos de linha ficarem visualmente alinhados na mesma coluna. -->
          <MonitorSparkline
            :data="item.gaugeHistory || []"
            :color="gaugeSparklineColor(item)"
            :width="189"
            :height="28"
          />
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
          <router-link
            :to="`/monitors/${item.id}`"
            class="text-decoration-none d-inline-flex align-center"
          >
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

    <template #item.intervalSeconds="{ item }">
      <span class="text-body-2">{{ item.intervalSeconds }}s</span>
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
        @update:model-value="(val) => toggle(item, Boolean(val))"
      ></v-switch>
    </template>

    <template #item.actions="{ item }">
      <!-- Botão com rótulo e botões-ícone têm alturas diferentes: o flex com
           `align-center` mantém todos na mesma linha de base. -->
      <div class="d-flex align-center ga-1">
        <v-btn
          size="small"
          color="primary"
          variant="outlined"
          prepend-icon="mdi-play"
          :loading="runningId === item.id"
          @click="run(item)"
        >
          Testar
        </v-btn>

        <v-btn icon size="small" variant="text" color="info" :to="`/monitors/${item.id}`">
          <v-icon>mdi-chart-timeline-variant</v-icon>
          <v-tooltip activator="parent" location="top">Ver Gráficos e Detalhes</v-tooltip>
        </v-btn>

        <v-btn icon size="small" variant="text" color="primary" @click="emit('edit', item)">
          <v-icon>mdi-pencil</v-icon>
          <v-tooltip activator="parent" location="top">Editar monitor</v-tooltip>
        </v-btn>
        <v-btn icon size="small" variant="text" color="error" @click="confirmDelete(item)">
          <v-icon>mdi-delete</v-icon>
          <v-tooltip activator="parent" location="top">Excluir monitor</v-tooltip>
        </v-btn>
      </div>
    </template>

    <template #mobile-item="{ item }">
      <div class="d-flex flex-column ga-2">
        <div class="d-flex align-start justify-space-between ga-2">
          <div class="flex-grow-1 text-break">
            <router-link
              :to="`/monitors/${item.id}`"
              class="text-subtitle-1 font-weight-bold text-decoration-none text-primary d-block"
            >
              {{ item.name }}
            </router-link>
            <div v-if="showDevice" class="text-caption text-grey-darken-1">
              {{ item.device?.name || 'Dispositivo não vinculado' }}
            </div>
            <div class="d-flex flex-wrap align-center ga-2 mt-1">
              <v-chip size="x-small" :color="typeChip(item).color" variant="tonal">
                <v-icon start size="12">{{ typeChip(item).icon }}</v-icon>
                {{ typeChip(item).label }}
              </v-chip>
              <span class="text-caption text-grey-darken-1">{{ formatTarget(item) }}</span>
            </div>
          </div>
          <div class="d-flex flex-column align-end ga-1">
            <v-chip
              v-if="isGaugeMonitor(item)"
              :color="gaugeColor(item)"
              size="small"
              variant="tonal"
            >
              {{ item.gaugeMetric ? `${Math.round(item.gaugeMetric.value)}%` : 'SEM DADOS' }}
            </v-chip>
            <v-chip
              v-else-if="isInterfaceMonitor(item)"
              :color="interfaceStatusInfo(item).color"
              size="small"
              variant="tonal"
            >
              {{ interfaceStatusInfo(item).label }}
            </v-chip>
            <v-chip v-else :color="getStatusColor(item.status)" size="small" variant="tonal">
              {{ (item.status || 'UNKNOWN').toUpperCase() }}
            </v-chip>
            <v-switch
              :model-value="item.isEnabled"
              color="success"
              hide-details
              density="compact"
              class="mt-1"
              style="transform: scale(0.85); transform-origin: right center"
              @update:model-value="(val) => toggle(item, Boolean(val))"
            ></v-switch>
          </div>
        </div>

        <div class="monitor-timeline-scroll">
          <router-link
            :to="`/monitors/${item.id}`"
            class="text-decoration-none d-inline-flex align-center"
          >
            <template v-if="isGaugeMonitor(item)">
              <div class="d-flex align-center ga-2">
                <MonitorSparkline
                  :data="item.gaugeHistory || []"
                  :color="gaugeSparklineColor(item)"
                  :width="220"
                  :height="28"
                />
                <span class="text-caption font-weight-medium">
                  {{ item.gaugeMetric ? `${Math.round(item.gaugeMetric.value)}%` : 'N/D' }}
                </span>
              </div>
            </template>
            <MonitorTimelineBar
              v-else
              :results="item.recentResults"
              :max-blocks="24"
              :height="20"
              :width="5"
            />
          </router-link>
        </div>

        <div class="d-flex justify-end ga-1 mt-1">
          <v-btn
            size="small"
            color="primary"
            variant="outlined"
            prepend-icon="mdi-play"
            :loading="runningId === item.id"
            @click="run(item)"
          >
            Testar
          </v-btn>
          <v-btn icon size="small" variant="text" color="info" :to="`/monitors/${item.id}`">
            <v-icon>mdi-chart-timeline-variant</v-icon>
          </v-btn>
          <v-btn icon size="small" variant="text" color="primary" @click="emit('edit', item)">
            <v-icon>mdi-pencil</v-icon>
          </v-btn>
          <v-btn icon size="small" variant="text" color="error" @click="confirmDelete(item)">
            <v-icon>mdi-delete</v-icon>
          </v-btn>
        </div>
      </div>
    </template>
  </ResponsiveDataTable>

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
        verificações serão removidos permanentemente. Para apenas parar as checagens, desative-o na
        coluna "Ativo".
      </v-card-text>
      <v-card-actions class="justify-end">
        <v-btn variant="text" @click="deleteDialog = false">Cancelar</v-btn>
        <v-btn color="error" variant="flat" :loading="deleting" @click="executeDelete">
          Excluir
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useMonitorsStore, type Monitor } from '@/stores/monitors'
import MonitorTimelineBar from '@/components/MonitorTimelineBar.vue'
import MonitorSparkline from '@/components/MonitorSparkline.vue'
import ResponsiveDataTable from '@/components/ResponsiveDataTable.vue'
import {
  isGaugeMonitor,
  gaugeMetricName,
  gaugeColor as gaugeColorFor,
  gaugeHexColor,
  isInterfaceMonitor,
  interfaceStatusInfo as interfaceStatusInfoFor,
  latestResultData,
  getStatusColor,
} from '@/utils/monitorPresentation'
import { monitorKind, resolveKind, resolveSnmpMode, SNMP_MODES } from '@/utils/monitorTypes'

/**
 * Listagem de monitores compartilhada entre `/monitors` e a aba "Monitores" de
 * `/devices/:id`.
 *
 * O componente é dono da apresentação **e** das ações que não mudam de tela
 * (testar, ativar/desativar, excluir) — inclusive da confirmação de exclusão.
 * Só a edição sobe como evento, porque o formulário é um diálogo que cada tela
 * já monta com seus próprios padrões (na tela do equipamento, por exemplo, o
 * vínculo vem travado).
 *
 * `variant`:
 * - `full`    — usada em `/monitors`, mostra a coluna de dispositivo;
 * - `device`  — usada dentro de um equipamento, onde o dispositivo é redundante
 *               e o intervalo de checagem é a informação que falta.
 */
const props = withDefaults(
  defineProps<{
    monitors: Monitor[]
    loading?: boolean
    search?: string
    variant?: 'full' | 'device'
    noDataText?: string
  }>(),
  {
    loading: false,
    search: '',
    variant: 'full',
    noDataText: 'Nenhum monitor cadastrado',
  }
)

const emit = defineEmits<{
  (e: 'edit', monitor: Monitor): void
  /** Alguma ação alterou os dados no servidor — a tela deve recarregar sua lista */
  (e: 'changed'): void
}>()

const monitorsStore = useMonitorsStore()
const runningId = ref<number | null>(null)
const deleteDialog = ref(false)
const deleting = ref(false)
const monitorToDelete = ref<Monitor | null>(null)

const showDevice = computed(() => props.variant === 'full')

const headers = computed(() => [
  { title: 'ID', key: 'id', width: '60px' },
  {
    title: showDevice.value ? 'Nome, Dispositivo e Histórico' : 'Nome e Histórico',
    key: 'name',
  },
  { title: 'Tipo', key: 'type', width: '90px' },
  { title: 'Alvo', key: 'target' },
  ...(showDevice.value ? [] : [{ title: 'Intervalo', key: 'intervalSeconds', width: '100px' }]),
  { title: 'Status', key: 'status', width: '100px' },
  { title: 'Ativo', key: 'isEnabled', width: '80px' },
  { title: 'Ações', key: 'actions', sortable: false, width: '220px' },
])

/**
 * As ações vão ao servidor pelo store, mas a lista exibida pode não ser a do
 * store (a tela do equipamento tem a sua). Por isso, além de chamar a ação,
 * avisamos a tela para recarregar o que ela mesma carregou.
 */
async function run(monitor: Monitor) {
  runningId.value = monitor.id
  try {
    await monitorsStore.runMonitor(monitor.id)
    emit('changed')
  } finally {
    runningId.value = null
  }
}

async function toggle(monitor: Monitor, enable: boolean) {
  await monitorsStore.toggleMonitorEnabled(monitor.id, enable)
  emit('changed')
}

function confirmDelete(monitor: Monitor) {
  monitorToDelete.value = monitor
  deleteDialog.value = true
}

async function executeDelete() {
  if (!monitorToDelete.value) return
  deleting.value = true
  try {
    await monitorsStore.deleteMonitor(monitorToDelete.value.id)
    deleteDialog.value = false
    monitorToDelete.value = null
    emit('changed')
  } finally {
    deleting.value = false
  }
}

function gaugeColor(item: Monitor): string {
  return gaugeColorFor(item.gaugeMetric?.value ?? null, gaugeMetricName(item))
}

function gaugeSparklineColor(item: Monitor): string {
  return gaugeHexColor(item.gaugeMetric?.value ?? null, gaugeMetricName(item))
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
</script>

<style scoped>
.hover-underline:hover {
  text-decoration: underline !important;
}
</style>
