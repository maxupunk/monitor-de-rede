<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 1180"
    :fullscreen="$vuetify.display.xs"
    scrollable
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card class="rounded-lg">
      <!-- Cabeçalho do Modal -->
      <v-card-title class="pa-4 d-flex align-center justify-space-between flex-wrap ga-3">
        <div class="d-flex align-center ga-3">
          <v-avatar color="primary" variant="tonal" size="44" rounded="lg">
            <v-icon size="26">mdi-cloud-search-outline</v-icon>
          </v-avatar>
          <div>
            <div class="text-h6 font-weight-bold">Catálogo de Serviços SaaS & Bancos</div>
            <div class="text-caption text-medium-emphasis">
              Monitore a latência e qualidade de experiência (QoE) para bancos, nuvem, IA e serviços
              críticos
            </div>
          </div>
        </div>

        <div class="d-flex align-center ga-2">
          <v-btn
            icon="mdi-refresh"
            variant="text"
            size="small"
            :loading="loading"
            @click="loadPresets"
          >
            <v-tooltip activator="parent" location="bottom">Atualizar status do catálogo</v-tooltip>
          </v-btn>
          <v-btn
            icon="mdi-close"
            variant="text"
            size="small"
            @click="emit('update:modelValue', false)"
          ></v-btn>
        </div>
      </v-card-title>

      <v-divider></v-divider>

      <!-- Barra de Filtros e Busca -->
      <div class="pa-4 bg-surface-light border-b">
        <div
          class="d-flex flex-column flex-md-row align-stretch align-md-center justify-space-between ga-3"
        >
          <v-slide-group v-model="selectedCategory" show-arrows mandatory class="flex-grow-1">
            <v-slide-group-item
              v-for="cat in categories"
              :key="cat.value"
              v-slot="{ isSelected: isGroupSelected, toggle }"
              :value="cat.value"
            >
              <v-btn
                :color="isGroupSelected ? 'primary' : undefined"
                :variant="isGroupSelected ? 'flat' : 'outlined'"
                size="small"
                rounded="pill"
                class="mr-2 text-none"
                @click="toggle"
              >
                <v-icon start size="16">{{ cat.icon }}</v-icon>
                {{ cat.label }}
                <v-badge
                  v-if="cat.value !== 'all'"
                  :content="countCategory(cat.value)"
                  inline
                  color="transparent"
                  text-color="medium-emphasis"
                  class="ml-1"
                ></v-badge>
              </v-btn>
            </v-slide-group-item>
          </v-slide-group>

          <v-text-field
            v-model="searchQuery"
            prepend-inner-icon="mdi-magnify"
            placeholder="Buscar por banco, SaaS, provedor ou alvo..."
            single-line
            hide-details
            variant="outlined"
            density="compact"
            clearable
            style="min-width: 260px; max-width: 340px"
          ></v-text-field>
        </div>
      </div>

      <v-card-text class="pa-4 bg-surface">
        <!-- Indicador de Carregamento -->
        <div v-if="loading && presets.length === 0" class="text-center py-12">
          <v-progress-circular indeterminate color="primary" size="48"></v-progress-circular>
          <div class="text-body-2 text-medium-emphasis mt-3">
            Carregando catálogo de serviços...
          </div>
        </div>

        <!-- Alerta de Erro -->
        <v-alert
          v-else-if="errorMessage"
          type="error"
          variant="tonal"
          class="mb-4"
          :text="errorMessage"
        ></v-alert>

        <!-- Grade de Cards SaaS & Bancos -->
        <v-row v-else dense>
          <v-col
            v-for="preset in filteredPresets"
            :key="preset.id"
            cols="12"
            sm="6"
            lg="4"
            class="pa-2"
          >
            <v-card
              variant="outlined"
              class="h-100 d-flex flex-column rounded-lg saas-preset-card transition-all"
              :class="{
                'saas-card-selected': isSelected(preset.id),
                'saas-card-provisioned': preset.isProvisioned,
              }"
              @click="toggleSelection(preset)"
            >
              <!-- Topo do Card: Avatar, Provedor e Checkbox -->
              <div class="pa-4 pb-2 d-flex align-start justify-space-between ga-2">
                <div class="d-flex align-center ga-3 overflow-hidden">
                  <v-avatar
                    :color="preset.color"
                    size="42"
                    variant="tonal"
                    class="flex-shrink-0"
                    rounded="lg"
                  >
                    <v-icon size="24" :color="preset.color">{{ preset.icon }}</v-icon>
                  </v-avatar>
                  <div class="overflow-hidden">
                    <div
                      class="saas-service-title font-weight-bold text-subtitle-1 text-truncate"
                      :title="preset.name"
                    >
                      {{ preset.name }}
                    </div>
                    <div class="text-caption text-medium-emphasis text-truncate">
                      {{ preset.provider }}
                    </div>
                  </div>
                </div>

                <!-- Checkbox ou Badge de Monitorado -->
                <div class="flex-shrink-0">
                  <v-checkbox-btn
                    v-if="!preset.isProvisioned"
                    :model-value="isSelected(preset.id)"
                    density="compact"
                    color="primary"
                    class="ma-0 pa-0"
                    @click.stop="toggleSelection(preset)"
                  ></v-checkbox-btn>
                  <v-chip
                    v-else
                    size="x-small"
                    color="success"
                    variant="tonal"
                    class="font-weight-bold"
                  >
                    <v-icon start size="10">mdi-check-circle</v-icon>
                    Ativo
                  </v-chip>
                </div>
              </div>

              <!-- Tags de Categoria e Protocolo -->
              <div class="px-4 pb-2 d-flex align-center ga-1 flex-wrap">
                <v-chip
                  size="x-small"
                  :color="categoryColor(preset.category)"
                  variant="tonal"
                  class="font-weight-medium"
                >
                  <v-icon start size="12">{{ categoryIcon(preset.category) }}</v-icon>
                  {{ categoryLabel(preset.category) }}
                </v-chip>
                <v-chip
                  size="x-small"
                  :color="preset.checkType === 'http' ? 'info' : 'primary'"
                  variant="outlined"
                  class="font-weight-medium"
                >
                  <v-icon start size="12">
                    {{ preset.checkType === 'http' ? 'mdi-web' : 'mdi-pulse' }}
                  </v-icon>
                  {{ preset.checkType === 'http' ? preset.httpMethod || 'HTTP HEAD' : 'ICMP Ping' }}
                </v-chip>
              </div>

              <!-- Corpo do Card: Descrição e Alvo -->
              <v-card-text
                class="pt-0 px-4 pb-3 flex-grow-1 d-flex flex-column justify-space-between"
              >
                <div class="text-caption text-medium-emphasis mb-3 saas-desc-clamp">
                  {{ preset.description }}
                </div>

                <div>
                  <!-- Alvo / Endpoint -->
                  <div
                    class="saas-target-box d-flex align-center ga-2 pa-2 rounded mb-2 border bg-surface-light"
                  >
                    <v-icon size="14" color="primary">mdi-link-variant</v-icon>
                    <span
                      class="text-caption font-family-monospace text-truncate flex-grow-1"
                      :title="preset.target"
                    >
                      {{ preset.target }}
                    </span>
                  </div>

                  <!-- Intervalo e Threshold de Alerta -->
                  <div
                    class="d-flex align-center justify-space-between text-caption text-medium-emphasis"
                  >
                    <span class="d-flex align-center ga-1">
                      <v-icon size="13">mdi-timer-outline</v-icon>
                      {{ preset.intervalSeconds }}s
                    </span>
                    <span class="d-flex align-center ga-1 text-warning font-weight-medium">
                      <v-icon size="13" color="warning">mdi-speedometer</v-icon>
                      Baseline +50% · 3 leituras
                    </span>
                  </div>
                </div>
              </v-card-text>

              <v-divider></v-divider>

              <!-- Rodapé do Card: Status Atual e Ação -->
              <v-card-actions class="pa-2 px-3 justify-space-between bg-surface-light">
                <div v-if="preset.isProvisioned" class="d-flex align-center ga-2">
                  <v-chip
                    size="small"
                    :color="statusColor(preset.currentStatus)"
                    variant="tonal"
                    class="font-weight-medium"
                  >
                    <v-icon start size="12">mdi-circle</v-icon>
                    {{ preset.currentStatus ? preset.currentStatus.toUpperCase() : 'UP' }}
                    <span v-if="preset.currentLatencyMs" class="ml-1 font-weight-bold">
                      · {{ preset.currentLatencyMs.toFixed(1) }}ms
                    </span>
                  </v-chip>
                </div>

                <div v-else>
                  <span class="text-caption text-medium-emphasis">Não provisionado</span>
                </div>

                <v-btn
                  v-if="!preset.isProvisioned"
                  size="small"
                  color="primary"
                  variant="tonal"
                  prepend-icon="mdi-plus"
                  :loading="provisioningId === preset.id"
                  @click.stop="provisionSingle(preset.id)"
                >
                  Adicionar
                </v-btn>
                <v-btn
                  v-else
                  size="small"
                  color="secondary"
                  variant="text"
                  append-icon="mdi-chevron-right"
                  :to="preset.monitorId ? `/monitors/${preset.monitorId}` : undefined"
                  @click="emit('update:modelValue', false)"
                >
                  Detalhes
                </v-btn>
              </v-card-actions>
            </v-card>
          </v-col>
        </v-row>
      </v-card-text>

      <v-divider></v-divider>

      <!-- Rodapé do Diálogo com Ações em Massa -->
      <v-card-actions class="pa-4 justify-space-between flex-wrap ga-2">
        <div class="d-flex align-center ga-2">
          <v-btn
            size="small"
            variant="text"
            color="primary"
            :disabled="selectablePresets.length === 0"
            @click="selectAll"
          >
            {{
              selectedIds.length === selectablePresets.length && selectablePresets.length > 0
                ? 'Desmarcar Todos'
                : 'Selecionar Todos Não Monitorados'
            }}
          </v-btn>
          <span v-if="selectedIds.length > 0" class="text-caption text-medium-emphasis">
            ({{ selectedIds.length }} selecionado{{ selectedIds.length > 1 ? 's' : '' }})
          </span>
        </div>

        <div class="d-flex align-center ga-2">
          <v-btn variant="outlined" @click="emit('update:modelValue', false)"> Fechar </v-btn>

          <v-btn
            v-if="selectedIds.length > 0"
            color="primary"
            variant="flat"
            prepend-icon="mdi-plus-box-multiple"
            :loading="batchProvisioning"
            @click="provisionBatch"
          >
            Provisionar Selecionados ({{ selectedIds.length }})
          </v-btn>
          <v-btn
            v-else-if="unprovisionedCount > 0"
            color="primary"
            variant="tonal"
            prepend-icon="mdi-auto-fix"
            :loading="batchProvisioning"
            @click="provisionAllRecommended"
          >
            Provisionar Recomendados ({{ Math.min(unprovisionedCount, 6) }})
          </v-btn>
        </div>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue'
import { useMonitorsStore } from '@/stores/monitors'
import type { SaasPreset } from '@/bindings/SaasPreset'

const props = defineProps<{
  modelValue: boolean
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  (e: 'provisioned'): void
}>()

const monitorsStore = useMonitorsStore()

const loading = ref(false)
const errorMessage = ref<string | null>(null)
const presets = ref<SaasPreset[]>([])
const selectedCategory = ref('all')
const searchQuery = ref('')
const selectedIds = ref<string[]>([])
const provisioningId = ref<string | null>(null)
const batchProvisioning = ref(false)

const categories = [
  { value: 'all', label: 'Todos', icon: 'mdi-view-grid-outline' },
  { value: 'finance', label: 'Bancos & Finanças', icon: 'mdi-bank' },
  { value: 'productivity', label: 'Produtividade & IA', icon: 'mdi-robot-outline' },
  { value: 'cloud', label: 'Nuvem & Infra', icon: 'mdi-cloud-outline' },
  { value: 'communication', label: 'Comunicação', icon: 'mdi-message-text-outline' },
  { value: 'developer', label: 'Dev & APIs', icon: 'mdi-code-tags' },
  { value: 'streaming', label: 'Streaming & Mídia', icon: 'mdi-play-circle-outline' },
  { value: 'government', label: 'Governo & Utilidades', icon: 'mdi-shield-account' },
]

async function loadPresets() {
  loading.value = true
  errorMessage.value = null
  try {
    const res = await monitorsStore.fetchSaasPresets()
    presets.value = res.presets
  } catch (err: unknown) {
    errorMessage.value = err instanceof Error ? err.message : 'Erro ao carregar catálogo SaaS'
  } finally {
    loading.value = false
  }
}

watch(
  () => props.modelValue,
  (open) => {
    if (open) {
      loadPresets()
      selectedIds.value = []
    }
  }
)

onMounted(() => {
  if (props.modelValue) {
    loadPresets()
  }
})

const filteredPresets = computed(() => {
  return presets.value.filter((p) => {
    if (selectedCategory.value !== 'all' && p.category !== selectedCategory.value) {
      return false
    }
    if (searchQuery.value) {
      const q = searchQuery.value.toLowerCase().trim()
      const matchName = p.name.toLowerCase().includes(q)
      const matchProvider = p.provider.toLowerCase().includes(q)
      const matchTarget = p.target.toLowerCase().includes(q)
      if (!matchName && !matchProvider && !matchTarget) return false
    }
    return true
  })
})

const selectablePresets = computed(() => {
  return filteredPresets.value.filter((p) => !p.isProvisioned)
})

const unprovisionedCount = computed(() => {
  return presets.value.filter((p) => !p.isProvisioned).length
})

function countCategory(catValue: string): number {
  return presets.value.filter((p) => p.category === catValue).length
}

function isSelected(id: string): boolean {
  return selectedIds.value.includes(id)
}

function toggleSelection(preset: SaasPreset) {
  if (preset.isProvisioned) return
  const idx = selectedIds.value.indexOf(preset.id)
  if (idx >= 0) {
    selectedIds.value.splice(idx, 1)
  } else {
    selectedIds.value.push(preset.id)
  }
}

function selectAll() {
  if (selectedIds.value.length === selectablePresets.value.length) {
    selectedIds.value = []
  } else {
    selectedIds.value = selectablePresets.value.map((p) => p.id)
  }
}

async function provisionSingle(id: string) {
  provisioningId.value = id
  try {
    await monitorsStore.provisionSaasPresets([id])
    await loadPresets()
    emit('provisioned')
  } catch (err: unknown) {
    errorMessage.value = err instanceof Error ? err.message : 'Falha ao provisionar monitor'
  } finally {
    provisioningId.value = null
  }
}

async function provisionBatch() {
  if (selectedIds.value.length === 0) return
  batchProvisioning.value = true
  try {
    await monitorsStore.provisionSaasPresets(selectedIds.value)
    selectedIds.value = []
    await loadPresets()
    emit('provisioned')
  } catch (err: unknown) {
    errorMessage.value = err instanceof Error ? err.message : 'Falha ao provisionar monitores'
  } finally {
    batchProvisioning.value = false
  }
}

async function provisionAllRecommended() {
  const recommendedIds = presets.value
    .filter((p) => !p.isProvisioned)
    .slice(0, 6)
    .map((p) => p.id)
  if (recommendedIds.length === 0) return
  batchProvisioning.value = true
  try {
    await monitorsStore.provisionSaasPresets(recommendedIds)
    await loadPresets()
    emit('provisioned')
  } catch (err: unknown) {
    errorMessage.value = err instanceof Error ? err.message : 'Falha ao provisionar catálogo'
  } finally {
    batchProvisioning.value = false
  }
}

function categoryLabel(cat: string): string {
  const found = categories.find((c) => c.value === cat)
  return found ? found.label : cat
}

function categoryIcon(cat: string): string {
  const found = categories.find((c) => c.value === cat)
  return found ? found.icon : 'mdi-cloud'
}

function categoryColor(cat: string): string {
  switch (cat) {
    case 'finance':
      return 'purple-darken-1'
    case 'productivity':
      return 'blue-darken-1'
    case 'developer':
      return 'deep-purple'
    case 'streaming':
      return 'red-darken-1'
    case 'cloud':
      return 'amber-darken-3'
    case 'communication':
      return 'teal-darken-1'
    case 'government':
      return 'indigo'
    default:
      return 'grey'
  }
}

function statusColor(status?: string | null): string {
  switch (status?.toLowerCase()) {
    case 'up':
      return 'success'
    case 'down':
      return 'error'
    case 'warning':
      return 'warning'
    default:
      return 'grey'
  }
}
</script>

<style scoped>
.saas-preset-card {
  border-color: rgba(var(--v-border-color), var(--v-border-opacity));
  cursor: pointer;
  background-color: rgb(var(--v-theme-surface));
}

.saas-preset-card:hover {
  border-color: rgba(var(--v-theme-primary), 0.6);
  transform: translateY(-2px);
  box-shadow: 0 6px 16px rgba(0, 0, 0, 0.08);
}

.saas-card-selected {
  border-color: rgb(var(--v-theme-primary)) !important;
  background-color: rgba(var(--v-theme-primary), 0.04) !important;
}

.saas-card-provisioned {
  border-color: rgba(var(--v-theme-success), 0.45) !important;
}

.saas-service-title {
  font-size: 0.95rem;
  line-height: 1.25;
  color: rgb(var(--v-theme-on-surface));
}

.saas-desc-clamp {
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  min-height: 32px;
  line-height: 16px;
}

.saas-target-box {
  background-color: rgba(var(--v-theme-on-surface), 0.03);
  border-color: rgba(var(--v-border-color), var(--v-border-opacity));
}
</style>
