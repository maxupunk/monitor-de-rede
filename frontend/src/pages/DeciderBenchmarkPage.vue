<template>
  <v-container fluid class="pa-4 pa-md-6">
    <!-- Cabeçalho unificado -->
    <div class="d-flex flex-wrap align-center justify-space-between ga-3 mb-4">
      <div>
        <div class="d-flex align-center ga-2">
          <v-icon size="26" color="primary">mdi-chart-box-outline</v-icon>
          <h1 class="text-h5 font-weight-bold">Benchmark dos modelos de IA</h1>
        </div>
        <p class="text-body-2 text-high-emphasis mt-1 mb-0">
          {{
            activeTab === 'chat'
              ? 'Desempenho real dos modelos locais do Assistente do Chat (Ollama) nas 61 ferramentas e cenários de rede do NetMonitor.'
              : 'Acerto e velocidade dos modelos de decisão rápida (Ollaya) nas seis frentes de classificação do NetMonitor.'
          }}
        </p>
      </div>

      <div class="d-flex flex-wrap align-center ga-2">
        <v-btn
          variant="tonal"
          color="primary"
          prepend-icon="mdi-tune-variant"
          :to="{ name: 'settings', query: { tab: 'ia' } }"
        >
          {{ activeTab === 'chat' ? 'Escolher modelo do Chat' : 'Escolher modelo no Decisor' }}
        </v-btn>
      </div>
    </div>

    <!-- Navegação por abas: Assistente do Chat × Decisor -->
    <v-card elevation="2" class="mb-4">
      <v-tabs
        :model-value="activeTab"
        color="primary"
        align-tabs="start"
        data-testid="ai-benchmark-tabs"
        @update:model-value="onSelectTab"
      >
        <v-tab value="chat" data-testid="ai-benchmark-tab-chat">
          <v-icon start size="18">mdi-robot-outline</v-icon>
          Assistente do Chat (Ollama)
          <v-chip
            v-if="aiStore.chatBenchmarkReport"
            size="x-small"
            color="success"
            variant="tonal"
            class="ml-2 font-mono font-weight-bold"
          >
            {{ aiStore.chatBenchmarkReport.models.length }} modelos · até
            {{ percent(topChatScore) }}
          </v-chip>
        </v-tab>

        <v-tab value="decider" data-testid="ai-benchmark-tab-decider">
          <v-icon start size="18">mdi-CallSplit</v-icon>
          Decisor Rápido (Ollaya)
          <v-chip
            v-if="deciderStore.benchmarkReport"
            size="x-small"
            color="primary"
            variant="tonal"
            class="ml-2 font-mono font-weight-bold"
          >
            {{ deciderStore.benchmarkReport.models.length }} modelos · até
            {{ percent(topDeciderScore) }}
          </v-chip>
        </v-tab>
      </v-tabs>
    </v-card>

    <!-- Conteúdo da aba ativa -->
    <ChatBenchmarkSection
      v-if="activeTab === 'chat'"
      :report="aiStore.chatBenchmarkReport"
      :loading="aiStore.loadingChatBenchmark"
      :error="aiStore.chatBenchmarkError"
      :current-chat-model="currentChatModel"
      :installed-models="aiStore.installedOllamaModels"
    />

    <DeciderBenchmarkSection
      v-else
      :report="deciderStore.benchmarkReport"
      :loading="deciderStore.loadingBenchmark"
      :error="deciderStore.benchmarkError"
      :current-model="deciderStore.saved?.model ?? null"
    />
  </v-container>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import ChatBenchmarkSection from '@/components/aiBenchmark/ChatBenchmarkSection.vue'
import DeciderBenchmarkSection from '@/components/aiBenchmark/DeciderBenchmarkSection.vue'
import { DEFAULT_MODELS, resolveDriver } from '@/components/settings/ai/aiProviders'
import { useAiStore } from '@/stores/ai'
import { useAiDeciderStore } from '@/stores/aiDecider'
import { resolveBenchmarkTab, type BenchmarkTab } from '@/utils/aiBenchmark'
import { percent } from '@/utils/decider'

const route = useRoute()
const router = useRouter()
const aiStore = useAiStore()
const deciderStore = useAiDeciderStore()

const activeTab = ref<BenchmarkTab>(resolveBenchmarkTab(route.query.tab))

watch(
  () => route.query.tab,
  (newTab) => {
    activeTab.value = resolveBenchmarkTab(newTab)
  }
)

function onSelectTab(value: unknown) {
  const next = resolveBenchmarkTab(value)
  activeTab.value = next
  void router.replace({
    query: {
      ...route.query,
      tab: next,
    },
  })
}

const currentChatModel = computed<string | null>(() => {
  if (!aiStore.settings) return null
  if (resolveDriver(aiStore.settings.activeDriver) !== 'ollama') return null
  return aiStore.settings.ollamaModel || DEFAULT_MODELS.ollama
})

const topChatScore = computed<number>(() => {
  const models = aiStore.chatBenchmarkReport?.models ?? []
  if (models.length === 0) return 0
  return Math.max(...models.map((m) => m.blind?.overall ?? m.bench.overall))
})

const topDeciderScore = computed<number>(() => {
  const models = deciderStore.benchmarkReport?.models ?? []
  if (models.length === 0) return 0
  return Math.max(...models.map((m) => m.overall))
})

onMounted(() => {
  if (!aiStore.chatBenchmarkReport) {
    void aiStore.loadChatBenchmark()
  }
  if (!deciderStore.benchmarkReport) {
    void deciderStore.loadBenchmark()
  }
  if (!aiStore.settings) {
    void aiStore.loadSettings()
  }
  if (aiStore.installedOllamaModels.length === 0) {
    void aiStore.loadOllamaModels()
  }
  if (!deciderStore.saved) {
    void deciderStore.load()
  }
})
</script>
