<template>
  <v-dialog v-model="isOpen" max-width="850" scrollable>
    <v-card class="rounded-lg">
      <v-card-title class="d-flex align-center justify-space-between py-3 px-4">
        <div class="d-flex align-center ga-2">
          <v-avatar color="primary" variant="tonal" size="36">
            <v-icon color="primary">
              {{ step === 1 ? 'mdi-plus-box-multiple-outline' : 'mdi-tune-vertical' }}
            </v-icon>
          </v-avatar>
          <div>
            <div class="text-h6 font-weight-bold">
              {{
                step === 1
                  ? 'Catálogo de Widgets e Cards'
                  : `Configurar: ${selectedTemplate?.title}`
              }}
            </div>
            <div class="text-caption text-grey">
              {{
                step === 1
                  ? 'Escolha um template de card ou reative um painel do dashboard'
                  : 'Selecione o recurso compatível para vincular a este novo card'
              }}
            </div>
          </div>
        </div>
        <v-btn icon variant="text" size="small" @click="closeDialog">
          <v-icon>mdi-close</v-icon>
        </v-btn>
      </v-card-title>

      <v-divider></v-divider>

      <!-- PASSO 1: Seleção de Templates de Cards e Widgets Standard -->
      <v-card-text v-if="step === 1" class="pa-4">
        <div class="d-flex flex-column flex-sm-row align-center justify-space-between ga-3 mb-4">
          <v-tabs v-model="selectedTab" color="primary" density="compact" class="w-100 w-sm-auto">
            <v-tab value="all">Todos os Cards</v-tab>
            <v-tab value="custom">Cards Personalizáveis</v-tab>
            <v-tab value="standard">Padrões do Sistema</v-tab>
          </v-tabs>

          <v-text-field
            v-model="searchQuery"
            density="compact"
            variant="outlined"
            placeholder="Buscar por nome ou recurso..."
            prepend-inner-icon="mdi-magnify"
            hide-details
            clearable
            style="max-width: 260px"
            class="w-100 w-sm-auto"
          ></v-text-field>
        </div>

        <!-- Seção 1: Templates Personalizáveis (Podem ser adicionados N vezes) -->
        <div v-if="selectedTab === 'all' || selectedTab === 'custom'" class="mb-6">
          <div class="text-subtitle-2 font-weight-bold mb-2 text-primary d-flex align-center ga-2">
            <v-icon size="18">mdi-toy-brick-plus-outline</v-icon>
            Templates de Cards Personalizáveis (Permitem Múltiplas Instâncias)
          </div>

          <v-row>
            <v-col v-for="tmpl in filteredTemplates" :key="tmpl.type" cols="12" sm="6">
              <v-card
                variant="outlined"
                class="h-100 d-flex flex-column rounded-lg transition-all border-dashed"
              >
                <v-card-item class="pb-2">
                  <template #prepend>
                    <v-avatar color="primary" variant="tonal" size="40" class="mr-3">
                      <v-icon>{{ tmpl.icon }}</v-icon>
                    </v-avatar>
                  </template>
                  <v-card-title class="text-subtitle-1 font-weight-bold">
                    {{ tmpl.title }}
                  </v-card-title>
                  <v-card-subtitle class="mt-1 d-flex align-center ga-1 flex-wrap">
                    <v-chip
                      v-for="resType in tmpl.compatibleResourceTypes"
                      :key="resType"
                      size="x-small"
                      color="deep-purple"
                      variant="tonal"
                    >
                      {{ resourceTypeLabel(resType) }}
                    </v-chip>
                  </v-card-subtitle>
                </v-card-item>

                <v-card-text class="pt-0 text-caption text-grey-darken-1 flex-grow-1">
                  {{ tmpl.description }}
                </v-card-text>

                <v-divider></v-divider>

                <v-card-actions class="pa-3 justify-space-between bg-surface-light">
                  <span class="text-caption text-grey">Múltiplas instâncias</span>
                  <v-btn
                    color="primary"
                    variant="flat"
                    size="small"
                    prepend-icon="mdi-plus"
                    @click="startCardCreation(tmpl)"
                  >
                    Adicionar Card
                  </v-btn>
                </v-card-actions>
              </v-card>
            </v-col>
          </v-row>
        </div>

        <!-- Seção 2: Painéis Padrão do Dashboard -->
        <div v-if="selectedTab === 'all' || selectedTab === 'standard'">
          <div
            class="text-subtitle-2 font-weight-bold mb-2 text-grey-darken-2 d-flex align-center ga-2"
          >
            <v-icon size="18">mdi-view-dashboard-outline</v-icon>
            Painéis Integrados do Sistema
          </div>

          <v-row>
            <v-col v-for="widget in filteredStandardWidgets" :key="widget.id" cols="12" sm="6">
              <v-card
                variant="outlined"
                class="h-100 d-flex flex-column rounded-lg transition-all"
                :class="{ 'border-info': widget.visible }"
              >
                <v-card-item class="pb-2">
                  <template #prepend>
                    <v-avatar
                      :color="widget.visible ? 'info' : 'grey-lighten-1'"
                      variant="tonal"
                      size="40"
                      class="mr-3"
                    >
                      <v-icon>{{ widget.icon }}</v-icon>
                    </v-avatar>
                  </template>
                  <v-card-title class="text-subtitle-1 font-weight-bold">
                    {{ widget.title }}
                  </v-card-title>
                  <v-card-subtitle class="mt-1">
                    <v-chip size="x-small" :color="categoryColor(widget.category)" variant="tonal">
                      {{ categoryLabel(widget.category) }}
                    </v-chip>
                  </v-card-subtitle>
                </v-card-item>

                <v-card-text class="pt-0 text-caption text-grey-darken-1 flex-grow-1">
                  {{ widget.description }}
                </v-card-text>

                <v-divider></v-divider>

                <v-card-actions class="pa-3 justify-end bg-surface-light">
                  <v-btn
                    v-if="!widget.visible"
                    color="info"
                    variant="flat"
                    size="small"
                    prepend-icon="mdi-eye"
                    @click="dashboardStore.toggleWidgetVisibility(widget.id, true)"
                  >
                    Exibir no Dashboard
                  </v-btn>
                  <v-chip v-else color="success" variant="tonal" size="small">
                    <v-icon start size="14">mdi-check-circle-outline</v-icon>
                    Visível no Dashboard
                  </v-chip>
                </v-card-actions>
              </v-card>
            </v-col>
          </v-row>
        </div>
      </v-card-text>

      <!-- PASSO 2: Wizard de Seleção do Recurso Compatível -->
      <v-card-text v-else-if="step === 2 && selectedTemplate" class="pa-6">
        <v-form @submit.prevent="finishCardCreation">
          <v-row>
            <v-col cols="12">
              <v-text-field
                v-model="customTitle"
                label="Título Personalizado do Card"
                variant="outlined"
                density="compact"
                hide-details="auto"
                prepend-inner-icon="mdi-format-title"
                required
              ></v-text-field>
            </v-col>

            <!-- Seleção de Recursos conforme o tipo compatível -->
            <v-col
              v-if="isResourceRequired('bandwidth') || isResourceRequired('numeric')"
              cols="12"
              sm="6"
            >
              <v-select
                v-model="formDeviceId"
                :items="deviceOptions"
                item-title="name"
                item-value="id"
                label="Equipamento de Origem"
                variant="outlined"
                density="compact"
                hide-details="auto"
                prepend-inner-icon="mdi-devices"
                @update:model-value="onFormDeviceChange"
              ></v-select>
            </v-col>

            <v-col v-if="isResourceRequired('bandwidth')" cols="12" sm="6">
              <v-select
                v-model="formInterfaceId"
                :items="interfaceOptions"
                item-title="name"
                item-value="id"
                label="Interface de Rede (Ether)"
                variant="outlined"
                density="compact"
                hide-details="auto"
                prepend-inner-icon="mdi-swap-horizontal"
              ></v-select>
            </v-col>

            <v-col v-if="isResourceRequired('dual-axis')" cols="12" sm="6">
              <v-select
                v-model="formMonitorId"
                :items="monitorOptions"
                item-title="name"
                item-value="id"
                label="Alvo de Ping / Latência"
                variant="outlined"
                density="compact"
                hide-details="auto"
                prepend-inner-icon="mdi-pulse"
              ></v-select>
            </v-col>

            <v-col v-if="isResourceRequired('binary')" cols="12">
              <v-select
                v-model="formMonitorId"
                :items="monitorOptions"
                item-title="name"
                item-value="id"
                label="Monitor de Rede (Dados Binários Booleanos)"
                variant="outlined"
                density="compact"
                hide-details="auto"
                prepend-inner-icon="mdi-checkbox-marked-circle-outline"
              ></v-select>
            </v-col>

            <v-col cols="12" sm="6">
              <v-select
                v-model="formTimeframe"
                :items="timeframeOptions"
                item-title="label"
                item-value="value"
                label="Intervalo de Tempo Inicial"
                variant="outlined"
                density="compact"
                hide-details="auto"
                prepend-inner-icon="mdi-clock-outline"
              ></v-select>
            </v-col>
          </v-row>
        </v-form>
      </v-card-text>

      <v-divider></v-divider>

      <v-card-actions class="pa-4 justify-space-between">
        <v-btn
          v-if="step === 2"
          color="secondary"
          variant="tonal"
          size="small"
          prepend-icon="mdi-arrow-left"
          @click="step = 1"
        >
          Voltar ao Catálogo
        </v-btn>
        <v-btn
          v-else
          color="warning"
          variant="outlined"
          size="small"
          prepend-icon="mdi-restore"
          @click="dashboardStore.resetToDefaultLayout()"
        >
          Restaurar Padrão
        </v-btn>

        <v-btn
          v-if="step === 2"
          color="primary"
          variant="flat"
          size="small"
          prepend-icon="mdi-check"
          @click="finishCardCreation"
        >
          Adicionar ao Dashboard
        </v-btn>
        <v-btn v-else color="primary" variant="flat" size="small" @click="closeDialog">
          Concluir
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import {
  useDashboardStore,
  CARD_TEMPLATES,
  type CardTemplate,
  type WidgetCategory,
  type ResourceCompatibilityType,
} from '@/stores/dashboard'
import { useDevicesStore } from '@/stores/devices'
import { useDeviceDetailStore } from '@/stores/deviceDetail'
import { useMonitorsStore } from '@/stores/monitors'

const props = defineProps<{
  modelValue: boolean
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', val: boolean): void
}>()

const dashboardStore = useDashboardStore()
const devicesStore = useDevicesStore()
const deviceDetailStore = useDeviceDetailStore()
const monitorsStore = useMonitorsStore()

const step = ref<1 | 2>(1)
const selectedTab = ref<'all' | 'custom' | 'standard'>('all')
const searchQuery = ref('')
const selectedTemplate = ref<CardTemplate | null>(null)

// Step 2 Form values
const customTitle = ref('')
const formDeviceId = ref<number | 'all'>('all')
const formInterfaceId = ref<number | 'all'>('all')
const formMonitorId = ref<number | 'all'>('all')
const formTimeframe = ref<'5m' | '15m' | '1h' | '24h'>('15m')

const timeframeOptions = [
  { label: 'Últimos 5 minutos', value: '5m' },
  { label: 'Últimos 15 minutos', value: '15m' },
  { label: 'Última 1 hora', value: '1h' },
  { label: 'Últimas 24 horas', value: '24h' },
]

const isOpen = computed({
  get: () => props.modelValue,
  set: (val) => emit('update:modelValue', val),
})

watch(isOpen, async (newVal) => {
  if (newVal) {
    step.value = 1
    if (devicesStore.devices.length === 0) await devicesStore.fetchDevices()
    if (monitorsStore.monitors.length === 0) await monitorsStore.fetchMonitors()
  }
})

const filteredTemplates = computed(() => {
  const q = searchQuery.value.trim().toLowerCase()
  return CARD_TEMPLATES.filter(
    (t) => !q || t.title.toLowerCase().includes(q) || t.description.toLowerCase().includes(q)
  )
})

const filteredStandardWidgets = computed(() => {
  const q = searchQuery.value.trim().toLowerCase()
  return dashboardStore.sortedWidgets.filter(
    (w) => !q || w.title.toLowerCase().includes(q) || w.description.toLowerCase().includes(q)
  )
})

const deviceOptions = computed(() => {
  const options: Array<{ id: number | 'all'; name: string }> = [
    { id: 'all', name: 'Todos os Equipamentos' },
  ]
  for (const dev of devicesStore.devices) {
    options.push({ id: dev.id, name: dev.name || dev.ipAddress || `Device #${dev.id}` })
  }
  return options
})

const interfaceOptions = computed(() => {
  const options: Array<{ id: number | 'all'; name: string }> = [
    { id: 'all', name: 'Todas as Interfaces (Média)' },
  ]
  for (const iface of deviceDetailStore.interfaces) {
    options.push({
      id: iface.id,
      name: iface.name || iface.ifName || `if-${iface.snmpIndex || iface.id}`,
    })
  }
  return options
})

const monitorOptions = computed(() => {
  const options: Array<{ id: number | 'all'; name: string }> = [
    { id: 'all', name: 'Todos os Monitores (Média)' },
  ]
  for (const m of monitorsStore.monitors) {
    options.push({ id: m.id, name: `${m.name} (${m.target})` })
  }
  return options
})

function isResourceRequired(type: ResourceCompatibilityType): boolean {
  if (!selectedTemplate.value) return false
  return selectedTemplate.value.compatibleResourceTypes.includes(type)
}

async function startCardCreation(tmpl: CardTemplate) {
  selectedTemplate.value = tmpl
  customTitle.value = tmpl.title
  formDeviceId.value = 'all'
  formInterfaceId.value = 'all'
  formMonitorId.value = 'all'
  formTimeframe.value = '15m'
  step.value = 2
}

async function onFormDeviceChange(val: number | 'all') {
  if (typeof val === 'number') {
    await deviceDetailStore.loadDeviceDetails(val)
  }
}

function finishCardCreation() {
  if (!selectedTemplate.value) return

  dashboardStore.addCustomWidget(
    selectedTemplate.value.type,
    {
      deviceId: formDeviceId.value,
      interfaceId: formInterfaceId.value,
      monitorId: formMonitorId.value,
      timeframe: formTimeframe.value,
    },
    customTitle.value
  )

  step.value = 1
  isOpen.value = false
}

function closeDialog() {
  step.value = 1
  isOpen.value = false
}

function resourceTypeLabel(rt: ResourceCompatibilityType): string {
  switch (rt) {
    case 'bandwidth':
      return 'Banda (Rx/Tx)'
    case 'dual-axis':
      return 'Eixo Duplo (Banda x Latência)'
    case 'numeric':
      return 'Métrica Numérica'
    case 'binary':
      return 'Estado Binário (Up/Down)'
    case 'dns-resolvers':
      return 'Resolvedores DNS'
    default:
      return rt
  }
}

function categoryLabel(cat: WidgetCategory): string {
  switch (cat) {
    case 'summary':
      return 'Resumo'
    case 'lists':
      return 'Listas & Eventos'
    case 'charts':
      return 'Gráficos Grafana'
    default:
      return 'Geral'
  }
}

function categoryColor(cat: WidgetCategory): string {
  switch (cat) {
    case 'summary':
      return 'primary'
    case 'lists':
      return 'info'
    case 'charts':
      return 'deep-purple'
    default:
      return 'grey'
  }
}
</script>

<style scoped>
.ga-1 {
  gap: 4px;
}
.ga-2 {
  gap: 8px;
}
.ga-3 {
  gap: 12px;
}
.border-dashed {
  border-style: dashed !important;
}
.transition-all {
  transition: all 0.2s ease-in-out;
}
</style>
