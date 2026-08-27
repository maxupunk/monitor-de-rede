<template>
  <v-card elevation="2" class="rounded-lg fill-height d-flex flex-column">
    <!-- Cabeçalho do Card -->
    <v-card-title
      class="d-flex align-center justify-space-between py-3 px-4 flex-wrap ga-2 border-b"
    >
      <div class="d-flex align-center ga-3">
        <v-avatar color="primary" variant="tonal" size="38" rounded="lg">
          <v-icon color="primary" size="22">mdi-bank-check</v-icon>
        </v-avatar>
        <div>
          <div class="text-subtitle-1 font-weight-bold">Serviços SaaS, Bancos & Nuvem</div>
          <div class="text-caption text-medium-emphasis">
            Qualidade de experiência (QoE) e tempo de resposta de alvos externos
          </div>
        </div>
      </div>

      <!-- Ações e Filtros de Categoria -->
      <div class="d-flex align-center ga-2 flex-wrap">
        <v-select
          v-model="selectedCategory"
          :items="categoryFilterOptions"
          item-title="title"
          item-value="value"
          density="compact"
          variant="outlined"
          hide-details
          style="min-width: 170px"
          class="text-caption"
        ></v-select>

        <v-btn
          color="primary"
          variant="tonal"
          size="small"
          prepend-icon="mdi-plus"
          @click="saasCatalogDialog = true"
        >
          <span class="hidden-xs">Catálogo SaaS</span>
          <span class="hidden-sm-and-up">SaaS</span>
        </v-btn>

        <v-btn icon="mdi-refresh" variant="text" size="small" :loading="loading" @click="refresh">
          <v-tooltip activator="parent" location="bottom">Atualizar status</v-tooltip>
        </v-btn>
      </div>
    </v-card-title>

    <!-- Indicador de Carregamento -->
    <div
      v-if="loading && saasMonitors.length === 0"
      class="d-flex align-center justify-center flex-grow-1 pa-8"
    >
      <v-progress-circular indeterminate color="primary" size="40"></v-progress-circular>
      <span class="text-caption text-medium-emphasis ml-3"
        >Verificando latência dos serviços...</span
      >
    </div>

    <!-- Estado Vazio: Nenhum SaaS Provisionado -->
    <div
      v-else-if="saasMonitors.length === 0"
      class="d-flex flex-column align-center justify-center flex-grow-1 pa-8 text-center"
    >
      <v-avatar color="primary" variant="tonal" size="56" class="mb-3">
        <v-icon size="32">mdi-cloud-plus-outline</v-icon>
      </v-avatar>
      <div class="text-subtitle-1 font-weight-bold">Nenhum serviço SaaS ou banco monitorado</div>
      <div class="text-caption text-medium-emphasis mb-4" style="max-width: 440px">
        Acompanhe a disponibilidade e a latência de ponta a ponta para Nubank, Itaú, Bradesco,
        OpenAI, Google, AWS, Microsoft 365 e outros serviços essenciais.
      </div>
      <v-btn
        color="primary"
        prepend-icon="mdi-cloud-search-outline"
        size="small"
        @click="saasCatalogDialog = true"
      >
        Abrir Catálogo de Serviços
      </v-btn>
    </div>

    <!-- Conteúdo Principal com Monitores Ativos -->
    <v-card-text v-else class="pa-4 flex-grow-1 d-flex flex-column ga-3">
      <!-- Mini Indicadores de Resumo Superior -->
      <div
        class="d-flex align-center justify-space-between flex-wrap ga-2 py-1 px-3 bg-surface-light rounded-lg border"
      >
        <div class="d-flex align-center ga-3 text-caption">
          <span>
            <strong>{{ saasMonitors.length }}</strong> serviço{{
              saasMonitors.length > 1 ? 's' : ''
            }}
            ativo{{ saasMonitors.length > 1 ? 's' : '' }}
          </span>
          <span>·</span>
          <span class="text-success font-weight-bold"> {{ upCount }} UP </span>
          <span v-if="downCount > 0" class="text-error font-weight-bold">
            · {{ downCount }} DOWN
          </span>
        </div>

        <div class="text-caption text-medium-emphasis d-flex align-center ga-2">
          <span>Média Global:</span>
          <v-chip
            size="x-small"
            :color="getLatencyColor(avgLatency)"
            variant="tonal"
            class="font-weight-bold"
          >
            {{ avgLatency ? `${avgLatency.toFixed(1)} ms` : '--' }}
          </v-chip>
        </div>
      </div>

      <!-- Grade dos Cards de Serviços -->
      <div class="saas-widgets-grid overflow-y-auto pr-1" style="max-height: 480px">
        <v-row dense>
          <v-col
            v-for="mon in filteredMonitors"
            :key="mon.id"
            cols="12"
            sm="6"
            md="4"
            lg="3"
            class="pa-1"
          >
            <v-card
              variant="outlined"
              class="pa-3 rounded-lg saas-item-card transition-all cursor-pointer h-100 d-flex flex-column justify-space-between"
              :class="`status-border-${mon.status || 'unknown'}`"
              @click="openMonitorDetail(mon.id)"
            >
              <div>
                <!-- Topo: Avatar do Serviço + Status Chip -->
                <div class="d-flex align-center justify-space-between mb-2">
                  <div class="d-flex align-center ga-2 overflow-hidden">
                    <v-avatar :color="getServiceColor(mon)" size="30" variant="tonal" rounded>
                      <v-icon size="18" :color="getServiceColor(mon)">
                        {{ getServiceIcon(mon) }}
                      </v-icon>
                    </v-avatar>
                    <div class="text-subtitle-2 font-weight-bold text-truncate" :title="mon.name">
                      {{ mon.name }}
                    </div>
                  </div>

                  <v-badge
                    dot
                    :color="
                      mon.status === 'up' ? 'success' : mon.status === 'down' ? 'error' : 'warning'
                    "
                    inline
                  ></v-badge>
                </div>

                <!-- Alvo do Monitor -->
                <div
                  class="text-caption text-medium-emphasis text-truncate mb-2 font-family-monospace"
                  :title="mon.target"
                >
                  {{ mon.target }}
                </div>
              </div>

              <!-- Rodapé do Card do Serviço: Latência e Categoria -->
              <div class="d-flex align-center justify-space-between pt-2 border-t mt-1">
                <span class="text-caption text-medium-emphasis">
                  {{ getCategoryLabel(mon) }}
                </span>

                <div class="d-flex align-center ga-1">
                  <v-chip
                    size="x-small"
                    :color="getLatencyColor(mon.lastLatencyMs)"
                    variant="flat"
                    class="font-weight-bold"
                  >
                    {{
                      mon.lastLatencyMs !== null && mon.lastLatencyMs !== undefined
                        ? `${mon.lastLatencyMs.toFixed(1)} ms`
                        : '--'
                    }}
                  </v-chip>
                </div>
              </div>
            </v-card>
          </v-col>
        </v-row>
      </div>
    </v-card-text>

    <!-- Dialog de Catálogo SaaS -->
    <SaasPresetsDialog v-model="saasCatalogDialog" @provisioned="refresh"></SaasPresetsDialog>

    <!-- Modal de Detalhes do Monitor -->
    <MonitorDetailDialog
      v-model="detailDialog"
      :monitor-id="selectedMonitorId"
    ></MonitorDetailDialog>
  </v-card>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useMonitorsStore, type Monitor } from '@/stores/monitors'
import SaasPresetsDialog from '@/components/monitors/SaasPresetsDialog.vue'
import MonitorDetailDialog from '@/components/monitors/MonitorDetailDialog.vue'

const monitorsStore = useMonitorsStore()

const loading = ref(false)
const selectedCategory = ref('all')
const saasCatalogDialog = ref(false)
const detailDialog = ref(false)
const selectedMonitorId = ref<number | null>(null)

const categoryFilterOptions = [
  { title: 'Todas as Categorias', value: 'all' },
  { title: 'Bancos & Finanças', value: 'finance' },
  { title: 'Produtividade & IA', value: 'productivity' },
  { title: 'Nuvem & Infra', value: 'cloud' },
  { title: 'Comunicação', value: 'communication' },
  { title: 'Streaming', value: 'streaming' },
  { title: 'Dev & APIs', value: 'developer' },
  { title: 'Governo', value: 'government' },
]

async function refresh() {
  loading.value = true
  try {
    await monitorsStore.fetchMonitors()
  } finally {
    loading.value = false
  }
}

onMounted(() => {
  if (monitorsStore.monitors.length === 0) {
    refresh()
  }
})

const saasMonitors = computed(() => {
  return monitorsStore.monitors.filter((m) => {
    const config = m.configuration as Record<string, unknown> | undefined
    return config?.isSaas === true || config?.saasPresetId !== undefined
  })
})

const filteredMonitors = computed(() => {
  if (selectedCategory.value === 'all') return saasMonitors.value
  return saasMonitors.value.filter((m) => {
    const config = m.configuration as Record<string, unknown> | undefined
    return config?.saasCategory === selectedCategory.value
  })
})

const upCount = computed(() => {
  return saasMonitors.value.filter((m) => m.status === 'up').length
})

const downCount = computed(() => {
  return saasMonitors.value.filter((m) => m.status === 'down').length
})

const avgLatency = computed(() => {
  const withLat = saasMonitors.value.filter(
    (m) => m.lastLatencyMs !== null && m.lastLatencyMs !== undefined
  )
  if (withLat.length === 0) return null
  const sum = withLat.reduce((acc, m) => acc + (m.lastLatencyMs || 0), 0)
  return sum / withLat.length
})

function getLatencyColor(latency: number | null | undefined): string {
  if (latency === null || latency === undefined) return 'grey'
  if (latency < 40) return 'success'
  if (latency < 80) return 'light-green-darken-1'
  if (latency < 150) return 'warning'
  return 'error'
}

function getServiceIcon(m: Monitor): string {
  const config = m.configuration as Record<string, unknown> | undefined
  const presetId = String(config?.saasPresetId || '')
  if (
    presetId.includes('nubank') ||
    presetId.includes('inter') ||
    presetId.includes('stripe') ||
    presetId.includes('pagbank')
  ) {
    return 'mdi-credit-card-outline'
  }
  if (
    presetId.includes('itau') ||
    presetId.includes('bradesco') ||
    presetId.includes('bb') ||
    presetId.includes('caixa') ||
    presetId.includes('santander')
  ) {
    return 'mdi-bank'
  }
  if (presetId.includes('google')) return 'mdi-google'
  if (presetId.includes('microsoft') || presetId.includes('teams') || presetId.includes('azure'))
    return 'mdi-microsoft'
  if (presetId.includes('aws')) return 'mdi-aws'
  if (presetId.includes('cloudflare')) return 'mdi-cloud-outline'
  if (presetId.includes('whatsapp')) return 'mdi-whatsapp'
  if (presetId.includes('telegram')) return 'mdi-send'
  if (presetId.includes('openai')) return 'mdi-robot-outline'
  if (presetId.includes('slack')) return 'mdi-slack'
  if (presetId.includes('github')) return 'mdi-github'
  if (presetId.includes('netflix')) return 'mdi-netflix'
  if (presetId.includes('spotify')) return 'mdi-spotify'
  if (presetId.includes('govbr')) return 'mdi-shield-account'
  return m.type === 'http' ? 'mdi-web' : 'mdi-pulse'
}

function getServiceColor(m: Monitor): string {
  const config = m.configuration as Record<string, unknown> | undefined
  const presetId = String(config?.saasPresetId || '')
  if (presetId.includes('nubank')) return '#820AD1'
  if (presetId.includes('itau')) return '#EC7000'
  if (presetId.includes('bradesco')) return '#CC092F'
  if (presetId.includes('bb')) return '#EAA300'
  if (presetId.includes('caixa')) return '#006699'
  if (presetId.includes('santander')) return '#EA1D25'
  if (presetId.includes('inter')) return '#FF7A00'
  if (presetId.includes('mercadopago')) return '#009EE3'
  if (presetId.includes('pagbank') || presetId.includes('stone')) return '#00A868'
  if (presetId.includes('stripe')) return '#635BFF'
  if (presetId.includes('openai')) return '#10A37F'
  if (presetId.includes('google')) return '#4285F4'
  if (presetId.includes('cloudflare')) return '#F38020'
  if (presetId.includes('aws')) return '#FF9900'
  if (presetId.includes('whatsapp')) return '#25D366'
  if (presetId.includes('telegram')) return '#229ED9'
  if (presetId.includes('netflix')) return '#E50914'
  if (presetId.includes('spotify')) return '#1DB954'
  return 'primary'
}

function getCategoryLabel(m: Monitor): string {
  const config = m.configuration as Record<string, unknown> | undefined
  const cat = String(config?.saasCategory || '')
  switch (cat) {
    case 'finance':
      return 'Bancos & Finanças'
    case 'productivity':
      return 'Produtividade & IA'
    case 'cloud':
      return 'Nuvem'
    case 'communication':
      return 'Comunicação'
    case 'streaming':
      return 'Streaming'
    case 'developer':
      return 'Dev & APIs'
    case 'government':
      return 'Governo'
    default:
      return 'SaaS'
  }
}

function openMonitorDetail(id: number) {
  selectedMonitorId.value = id
  detailDialog.value = true
}
</script>

<style scoped>
.saas-item-card {
  background-color: rgb(var(--v-theme-surface));
  border-color: rgba(var(--v-border-color), var(--v-border-opacity));
}

.saas-item-card:hover {
  transform: translateY(-2px);
  border-color: rgba(var(--v-theme-primary), 0.5);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.06);
}

.status-border-down {
  border-left: 3px solid rgb(var(--v-theme-error)) !important;
}

.status-border-up {
  border-left: 3px solid rgb(var(--v-theme-success)) !important;
}

.status-border-warning {
  border-left: 3px solid rgb(var(--v-theme-warning)) !important;
}
</style>
