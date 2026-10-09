<template>
  <v-dialog
    :model-value="modelValue"
    max-width="900"
    scrollable
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card class="d-flex flex-column">
      <!-- Cabeçalho do Modal -->
      <v-card-title class="d-flex align-center justify-space-between pa-4">
        <div class="d-flex align-center ga-2">
          <v-icon color="primary" size="24">{{ providerIcon }}</v-icon>
          <div>
            <div class="text-h6 font-weight-bold">Catálogo de modelos — {{ providerName }}</div>
            <div class="text-body-2 text-wrap">
              {{ providerSubtitle }}
            </div>
          </div>
        </div>

        <div class="d-flex align-center ga-1">
          <v-btn
            icon="mdi-refresh"
            variant="text"
            size="small"
            color="primary"
            :loading="isLoading"
            title="Atualizar catálogo"
            @click="refreshModels"
          />
          <v-btn
            icon="mdi-close"
            variant="text"
            size="small"
            color="primary"
            title="Fechar"
            @click="emit('update:modelValue', false)"
          />
        </div>
      </v-card-title>

      <v-divider />

      <!-- Barra de Filtros e Busca -->
      <div class="pa-4 border-b bg-surface">
        <v-text-field
          v-model="searchQuery"
          label="Buscar modelos por nome, ID ou fornecedor..."
          placeholder="Ex: claude, llama, free, deepseek, gemma, tools..."
          variant="outlined"
          density="compact"
          prepend-inner-icon="mdi-magnify"
          clearable
          hide-details
          class="mb-3"
          autofocus
        />

        <div class="d-flex align-center justify-space-between flex-wrap ga-2">
          <!-- Chips de Filtros Rápidos -->
          <v-chip-group
            v-model="activeCategory"
            selected-class="text-primary font-weight-bold"
            mandatory
          >
            <v-chip filter value="all" size="small" variant="tonal" color="primary">
              Todos ({{ totalCount }})
            </v-chip>
            <v-chip
              v-if="hasFreeFilter"
              filter
              value="free"
              size="small"
              variant="tonal"
              color="success"
            >
              Gratuitos ({{ freeCount }})
            </v-chip>
            <v-chip
              v-if="hasToolsFilter"
              filter
              value="tools"
              size="small"
              variant="tonal"
              color="warning"
            >
              <v-icon start size="14">mdi-tools</v-icon>
              Com ferramentas ({{ toolsCount }})
            </v-chip>
            <v-chip
              v-if="driver === 'ollama'"
              filter
              value="installed"
              size="small"
              variant="tonal"
              color="success"
            >
              Instalados ({{ installedCount }})
            </v-chip>
            <v-chip
              v-if="driver === 'ollama'"
              filter
              value="recommended"
              size="small"
              variant="tonal"
              color="primary"
            >
              Recomendados
            </v-chip>
          </v-chip-group>

          <!-- Filtro de Fornecedor para OpenRouter -->
          <div
            v-if="driver === 'openrouter' && vendorList.length > 0"
            class="d-flex align-center ga-1"
          >
            <span class="text-body-2">Fornecedor:</span>
            <v-select
              v-model="selectedVendor"
              :items="['Todos', ...vendorList]"
              density="compact"
              variant="outlined"
              hide-details
              style="min-width: 140px; max-width: 180px"
            />
          </div>
        </div>
      </div>

      <!-- Conteúdo com a Lista de Modelos -->
      <v-card-text class="pa-4" style="max-height: 60vh">
        <!-- Indicador de Carregamento -->
        <div v-if="isLoading" class="d-flex flex-column align-center justify-center pa-8">
          <v-progress-circular indeterminate color="primary" size="36" width="3" class="mb-3" />
          <span class="text-body-2">Consultando catálogo de modelos...</span>
        </div>

        <!-- Lista Vazia (Sem Resultados) -->
        <div
          v-else-if="filteredModels.length === 0"
          class="d-flex flex-column align-center justify-center pa-8 text-center"
        >
          <v-icon size="48" color="info" class="mb-2">mdi-cube-off-outline</v-icon>
          <div class="font-weight-bold mb-1">Nenhum modelo encontrado</div>
          <div v-if="searchQuery" class="text-body-2 mb-4" style="max-width: 400px">
            Não foram encontrados modelos que correspondam ao termo "{{ searchQuery }}".
          </div>
          <v-btn
            v-if="searchQuery && searchQuery.trim()"
            variant="outlined"
            color="primary"
            prepend-icon="mdi-check"
            @click="handleSelect(searchQuery.trim())"
          >
            Usar "{{ searchQuery.trim() }}" como slug personalizado
          </v-btn>
        </div>

        <!-- Lista de Resultados -->
        <div v-else>
          <div class="d-flex align-center justify-space-between mb-3 text-body-2">
            <span>
              Exibindo <strong>{{ paginatedModels.length }}</strong> de
              <strong>{{ filteredModels.length }}</strong> modelos encontrados
            </span>
            <span v-if="totalPages > 1">Página {{ currentPage }} de {{ totalPages }}</span>
          </div>

          <v-row dense>
            <v-col v-for="item in paginatedModels" :key="item.id" cols="12">
              <v-card
                variant="outlined"
                class="pa-3 transition-swing"
                :color="isItemActive(item.id) ? 'primary' : undefined"
                :class="{ 'border-primary': isItemActive(item.id) }"
              >
                <div class="d-flex align-start justify-space-between ga-2">
                  <div class="flex-grow-1" style="min-width: 0">
                    <!-- Linha 1: Nome Amigável e Selos -->
                    <div class="d-flex align-center flex-wrap ga-2 mb-1">
                      <span class="font-weight-bold text-body-1 text-truncate">
                        {{ item.name }}
                      </span>
                      <AiModelChips :item="item" :driver="driver" />
                    </div>

                    <!-- Linha 2: Slug / Identificador -->
                    <div class="d-flex align-center ga-1 mb-2">
                      <code class="text-body-2 bg-surface-light px-2 py-0-5 rounded text-truncate">
                        {{ item.id }}
                      </code>
                    </div>

                    <!-- Linha 3: Descrição -->
                    <p v-if="item.description" class="text-body-2 mb-0 line-clamp-2">
                      {{ item.description }}
                    </p>
                  </div>

                  <!-- Ações do Item -->
                  <div class="d-flex flex-column align-end ga-1 ms-2">
                    <template v-if="isItemActive(item.id)">
                      <v-chip size="small" color="primary" variant="flat" class="font-weight-bold">
                        <v-icon start size="14">mdi-check-circle</v-icon>
                        Em uso
                      </v-chip>
                    </template>
                    <template v-else-if="driver === 'ollama' && !item.isInstalled">
                      <v-btn
                        size="small"
                        variant="flat"
                        color="primary"
                        prepend-icon="mdi-download"
                        :loading="aiStore.pullingModelName === item.id"
                        :disabled="!!aiStore.pullingModelName"
                        @click="handleInstall(item.id)"
                      >
                        Instalar
                      </v-btn>
                      <v-btn
                        size="small"
                        variant="text"
                        color="primary"
                        class="mt-1"
                        @click="handleSelect(item.id)"
                      >
                        Selecionar
                      </v-btn>
                    </template>
                    <template v-else>
                      <v-btn
                        size="small"
                        variant="tonal"
                        color="primary"
                        prepend-icon="mdi-check"
                        @click="handleSelect(item.id)"
                      >
                        Selecionar
                      </v-btn>
                    </template>
                  </div>
                </div>
              </v-card>
            </v-col>
          </v-row>

          <!-- Paginação -->
          <div v-if="totalPages > 1" class="d-flex justify-center mt-4">
            <v-pagination
              v-model="currentPage"
              :length="totalPages"
              :total-visible="7"
              density="compact"
              size="small"
            />
          </div>
        </div>
      </v-card-text>

      <v-divider />

      <!-- Rodapé do Modal -->
      <v-card-actions class="pa-4 bg-surface d-flex justify-space-between">
        <div class="text-body-2">
          Total no catálogo: <strong>{{ totalCount }} modelos</strong>
        </div>
        <v-btn variant="text" color="primary" @click="emit('update:modelValue', false)">
          Fechar
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useAiStore } from '@/stores/ai'
import AiModelChips from './ai/AiModelChips.vue'
import {
  matchesModelQuery,
  ollamaChoices,
  opencodeChoices,
  openrouterChoices,
  type AiModelChoice,
} from './ai/aiModelCatalog'
import { DRIVER_LABELS, type AiDriver } from './ai/aiProviders'

const props = defineProps<{
  modelValue: boolean
  driver: AiDriver
  currentModel?: string
  apiKey?: string | null
  baseUrl?: string | null
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  (e: 'select', modelId: string): void
  (e: 'install', modelName: string): void
}>()

const aiStore = useAiStore()
const searchQuery = ref('')
const activeCategory = ref<'all' | 'free' | 'tools' | 'installed' | 'recommended'>('all')
const selectedVendor = ref<string>('Todos')
const currentPage = ref(1)
const itemsPerPage = 20

const PROVIDER_ICONS: Record<AiDriver, string> = {
  openrouter: 'mdi-router-network',
  opencode: 'mdi-xml',
  ollama: 'mdi-server',
}

const PROVIDER_SUBTITLES: Record<AiDriver, string> = {
  openrouter: 'Catálogo do OpenRouter, com o roteador gratuito e modelos que executam ferramentas',
  opencode: 'Modelos gratuitos e de alto desempenho da plataforma OpenCode Zen',
  ollama: 'Modelos instalados no seu Ollama e recomendados para diagnósticos de rede',
}

const providerName = computed(() => DRIVER_LABELS[props.driver])
const providerIcon = computed(() => PROVIDER_ICONS[props.driver])
const providerSubtitle = computed(() => PROVIDER_SUBTITLES[props.driver])

const isLoading = computed(() => {
  switch (props.driver) {
    case 'openrouter':
      return aiStore.loadingOpenrouterModels
    case 'opencode':
      return aiStore.loadingOpencodeModels
    default:
      return aiStore.loadingOllamaModels
  }
})

// Lista canônica unificada de modelos de acordo com o driver
const rawModels = computed<AiModelChoice[]>(() => {
  switch (props.driver) {
    case 'openrouter':
      return openrouterChoices(aiStore.openrouterModels)
    case 'opencode':
      return opencodeChoices(aiStore.opencodeModels)
    default:
      return ollamaChoices(aiStore.installedOllamaModels, aiStore.recommendedOllamaModels)
  }
})

// Fornecedores únicos do OpenRouter (ex: meta-llama, google, anthropic, openai)
const vendorList = computed<string[]>(() => {
  if (props.driver !== 'openrouter') return []
  const set = new Set<string>()
  for (const m of rawModels.value) {
    if (m.vendor) set.add(m.vendor)
  }
  return Array.from(set).sort()
})

// Contagens de estatísticas
const totalCount = computed(() => rawModels.value.length)
const freeCount = computed(() => rawModels.value.filter((m) => m.isFree).length)
const toolsCount = computed(() => rawModels.value.filter((m) => m.supportsTools).length)
const installedCount = computed(() => rawModels.value.filter((m) => m.isInstalled).length)

const hasFreeFilter = computed(() => props.driver === 'openrouter' || props.driver === 'opencode')
const hasToolsFilter = computed(() => props.driver === 'openrouter' || props.driver === 'ollama')

// Filtragem em tempo real por busca de texto, categoria e fornecedor
const filteredModels = computed<AiModelChoice[]>(() => {
  let list = rawModels.value.filter((m) => matchesModelQuery(m, searchQuery.value))

  if (activeCategory.value === 'free') {
    list = list.filter((m) => m.isFree)
  } else if (activeCategory.value === 'tools') {
    list = list.filter((m) => m.supportsTools)
  } else if (activeCategory.value === 'installed') {
    list = list.filter((m) => m.isInstalled)
  } else if (activeCategory.value === 'recommended') {
    list = list.filter((m) => m.isRecommended)
  }

  if (props.driver === 'openrouter' && selectedVendor.value && selectedVendor.value !== 'Todos') {
    list = list.filter((m) => m.vendor === selectedVendor.value)
  }

  return list
})

// Paginação dos resultados filtrados
const totalPages = computed(() => Math.ceil(filteredModels.value.length / itemsPerPage) || 1)

const paginatedModels = computed(() => {
  const start = (currentPage.value - 1) * itemsPerPage
  return filteredModels.value.slice(start, start + itemsPerPage)
})

watch([searchQuery, activeCategory, selectedVendor], () => {
  currentPage.value = 1
})

function isItemActive(modelId: string): boolean {
  if (!props.currentModel) return false
  return props.currentModel.trim().toLowerCase() === modelId.trim().toLowerCase()
}

function handleSelect(modelId: string) {
  emit('select', modelId)
  emit('update:modelValue', false)
}

function handleInstall(modelName: string) {
  emit('install', modelName)
}

async function refreshModels() {
  if (props.driver === 'openrouter') {
    await aiStore.loadOpenrouterModels(props.apiKey)
  } else if (props.driver === 'opencode') {
    await aiStore.loadOpencodeModels(props.apiKey, props.baseUrl)
  } else {
    await aiStore.loadOllamaModels(props.baseUrl)
  }
}
</script>

<style scoped>
.line-clamp-2 {
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
</style>
