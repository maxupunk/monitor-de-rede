<template>
  <v-dialog
    :model-value="modelValue"
    max-width="900"
    scrollable
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card class="d-flex flex-column">
      <!-- Cabeçalho do Modal -->
      <v-card-title class="d-flex align-center justify-space-between pa-4 bg-surface-variant">
        <div class="d-flex align-center ga-2">
          <v-icon color="primary" size="24">{{ providerIcon }}</v-icon>
          <div>
            <div class="text-h6 font-weight-bold">
              Catálogo e Busca de Modelos — {{ providerName }}
            </div>
            <div class="text-caption text-medium-emphasis">
              {{ providerSubtitle }}
            </div>
          </div>
        </div>

        <div class="d-flex align-center ga-1">
          <v-btn
            icon="mdi-refresh"
            variant="text"
            size="small"
            :loading="isLoading"
            title="Atualizar catálogo via API"
            @click="refreshModels"
          />
          <v-btn
            icon="mdi-close"
            variant="text"
            size="small"
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
            <v-chip filter value="all" size="small" variant="tonal">
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
              Gratuitos (Free) ({{ freeCount }})
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
              Suporte a Ferramentas ({{ toolsCount }})
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
            <span class="text-caption text-medium-emphasis">Fornecedor:</span>
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
          <span class="text-body-2 text-medium-emphasis">Consultando catálogo de modelos...</span>
        </div>

        <!-- Lista Vazia (Sem Resultados) -->
        <div
          v-else-if="filteredModels.length === 0"
          class="d-flex flex-column align-center justify-center pa-8 text-center"
        >
          <v-icon size="48" color="grey" class="mb-2">mdi-cube-off-outline</v-icon>
          <div class="text-subtitle-1 font-weight-bold mb-1">Nenhum modelo encontrado</div>
          <div class="text-caption text-medium-emphasis mb-4" style="max-width: 400px">
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
          <div
            class="d-flex align-center justify-space-between mb-3 text-caption text-medium-emphasis"
          >
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
                    <!-- Linha 1: Nome Amigável e Badges -->
                    <div class="d-flex align-center flex-wrap ga-2 mb-1">
                      <span class="font-weight-bold text-body-1 text-truncate">
                        {{ item.name }}
                      </span>

                      <!-- Badge: Gratuito vs Créditos -->
                      <v-chip
                        v-if="item.isFree"
                        size="x-small"
                        color="success"
                        variant="tonal"
                        class="font-weight-bold"
                      >
                        {{ item.id === 'openrouter/free' ? 'Auto Free Router' : 'Gratuito (Free)' }}
                      </v-chip>
                      <v-chip v-else size="x-small" color="primary" variant="outlined">
                        Standard / Créditos
                      </v-chip>

                      <!-- Badge: Suporte a Ferramentas (Tools) -->
                      <v-chip
                        v-if="item.supportsTools"
                        size="x-small"
                        color="warning"
                        variant="tonal"
                        title="Suporta execução de Ping, Traceroute e Diagnósticos de Rede"
                      >
                        <v-icon start size="12">mdi-tools</v-icon>
                        Tool Use
                      </v-chip>

                      <!-- Badge: Context Length -->
                      <v-chip
                        v-if="item.contextWindow || item.contextLength"
                        size="x-small"
                        color="info"
                        variant="tonal"
                      >
                        {{ item.contextWindow || formatContextLength(item.contextLength) }}
                      </v-chip>

                      <!-- Badge para Ollama: Instalado ou Recomendado -->
                      <v-chip
                        v-if="driver === 'ollama' && item.isInstalled"
                        size="x-small"
                        color="success"
                        variant="flat"
                      >
                        <v-icon start size="12">mdi-check</v-icon>
                        Instalado {{ item.size ? `(${item.size})` : '' }}
                      </v-chip>
                      <v-chip
                        v-else-if="driver === 'ollama' && item.isRecommended"
                        size="x-small"
                        color="primary"
                        variant="tonal"
                      >
                        Recomendado
                      </v-chip>
                    </div>

                    <!-- Linha 2: Slug / Identificador -->
                    <div class="d-flex align-center ga-1 mb-2">
                      <code class="text-caption bg-surface-light px-2 py-0-5 rounded text-truncate">
                        {{ item.id }}
                      </code>
                    </div>

                    <!-- Linha 3: Descrição -->
                    <p
                      v-if="item.description"
                      class="text-caption text-medium-emphasis mb-0 line-clamp-2"
                    >
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
                        size="x-small"
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
        <div class="text-caption text-medium-emphasis">
          Total disponível no catálogo: <strong>{{ totalCount }} modelos</strong>
        </div>
        <v-btn variant="text" @click="emit('update:modelValue', false)">Fechar</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useAiStore } from '@/stores/ai'
import { formatDecimalBytes } from '@/utils/formatters'

interface UniversalModelItem {
  id: string
  name: string
  isFree: boolean
  description?: string | null
  contextLength?: number | bigint | null
  contextWindow?: string | null
  supportsTools?: boolean | null
  isInstalled?: boolean
  isRecommended?: boolean
  size?: string | null
  vendor?: string
}

const props = defineProps<{
  modelValue: boolean
  driver: 'openrouter' | 'opencode' | 'ollama'
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

// Identificação visual do provedor ativo
const providerName = computed(() => {
  switch (props.driver) {
    case 'openrouter':
      return 'OpenRouter Gateway'
    case 'opencode':
      return 'OpenCode Go / Zen'
    case 'ollama':
      return 'Ollama Local'
    default:
      return 'Provedor de IA'
  }
})

const providerIcon = computed(() => {
  switch (props.driver) {
    case 'openrouter':
      return 'mdi-router-network'
    case 'opencode':
      return 'mdi-xml'
    case 'ollama':
      return 'mdi-server'
    default:
      return 'mdi-robot'
  }
})

const providerSubtitle = computed(() => {
  switch (props.driver) {
    case 'openrouter':
      return 'Catálogo completo com 440+ modelos de ponta, roteamento gratuito e suporte a chamadas de ferramentas'
    case 'opencode':
      return 'Modelos de alto desempenho e gratuitos da plataforma OpenCode Zen'
    case 'ollama':
      return 'Modelos instalados localmente e catálogo recomendado para diagnósticos de rede'
    default:
      return ''
  }
})

const isLoading = computed(() => {
  switch (props.driver) {
    case 'openrouter':
      return aiStore.loadingOpenrouterModels
    case 'opencode':
      return aiStore.loadingOpencodeModels
    case 'ollama':
      return aiStore.loadingOllamaModels
    default:
      return false
  }
})

// Lista canônica unificada de modelos de acordo com o driver
const rawModels = computed<UniversalModelItem[]>(() => {
  if (props.driver === 'openrouter') {
    return aiStore.openrouterModels.map((m) => {
      const parts = m.id.split('/')
      const vendor = parts.length > 1 ? parts[0] : 'other'
      return {
        id: m.id,
        name: m.name || m.id,
        isFree: m.isFree,
        description: m.description,
        contextLength: m.contextLength,
        supportsTools: m.supportsTools,
        vendor,
      }
    })
  }

  if (props.driver === 'opencode') {
    return aiStore.opencodeModels.map((m) => {
      return {
        id: m.id,
        name: m.name || m.id,
        isFree: m.isFree,
        description: m.description,
        supportsTools: m.supportsTools ?? true,
      }
    })
  }

  if (props.driver === 'ollama') {
    const map = new Map<string, UniversalModelItem>()

    // Modelos instalados
    for (const inst of aiStore.installedOllamaModels) {
      map.set(inst.name, {
        id: inst.name,
        name: inst.name,
        isFree: true,
        isInstalled: true,
        size: inst.size ? formatDecimalBytes(inst.size) : null,
        description: `Modelo instalado no Ollama local (${inst.parameterSize || 'tamanho não informado'}).`,
        supportsTools:
          inst.name.toLowerCase().includes('tool') || inst.name.toLowerCase().includes('groq'),
      })
    }

    // Modelos recomendados
    for (const rec of aiStore.recommendedOllamaModels) {
      const existing = map.get(rec.name)
      if (existing) {
        existing.isRecommended = true
        existing.supportsTools = rec.toolCallingOptimized
        existing.contextWindow = rec.contextWindow
        if (rec.description) existing.description = rec.description
      } else {
        map.set(rec.name, {
          id: rec.name,
          name: rec.name,
          isFree: true,
          isInstalled: rec.isInstalled,
          isRecommended: true,
          supportsTools: rec.toolCallingOptimized,
          contextWindow: rec.contextWindow,
          description: rec.description,
        })
      }
    }

    return Array.from(map.values())
  }

  return []
})

// Extrai fornecedores únicos para OpenRouter (ex: meta-llama, google, anthropic, openai)
const vendorList = computed<string[]>(() => {
  if (props.driver !== 'openrouter') return []
  const set = new Set<string>()
  for (const m of rawModels.value) {
    if (m.vendor && m.vendor !== 'other') {
      set.add(m.vendor)
    }
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
const filteredModels = computed<UniversalModelItem[]>(() => {
  let list = rawModels.value
  const query = searchQuery.value.trim().toLowerCase()

  // Filtro por texto
  if (query) {
    list = list.filter((m) => {
      const idMatch = m.id.toLowerCase().includes(query)
      const nameMatch = m.name.toLowerCase().includes(query)
      const descMatch = (m.description || '').toLowerCase().includes(query)
      const vendorMatch = (m.vendor || '').toLowerCase().includes(query)
      return idMatch || nameMatch || descMatch || vendorMatch
    })
  }

  // Filtro por categoria ativa
  if (activeCategory.value === 'free') {
    list = list.filter((m) => m.isFree)
  } else if (activeCategory.value === 'tools') {
    list = list.filter((m) => m.supportsTools)
  } else if (activeCategory.value === 'installed') {
    list = list.filter((m) => m.isInstalled)
  } else if (activeCategory.value === 'recommended') {
    list = list.filter((m) => m.isRecommended)
  }

  // Filtro por fornecedor no OpenRouter
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

function formatContextLength(ctx: number | bigint | null | undefined): string {
  if (!ctx) return ''
  const num = typeof ctx === 'bigint' ? Number(ctx) : ctx
  if (num >= 1_000_000) {
    return `${(num / 1_000_000).toFixed(1)}M context`
  }
  if (num >= 1000) {
    return `${Math.round(num / 1000)}k context`
  }
  return `${num} tokens`
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
  } else if (props.driver === 'ollama') {
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
