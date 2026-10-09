<template>
  <v-card variant="outlined" class="pa-4 rounded-lg">
    <div class="d-flex align-center ga-2 mb-1">
      <v-icon color="primary" size="20">mdi-download-box-outline</v-icon>
      <span class="font-weight-bold">Baixar modelo no Ollama</span>
    </div>
    <div class="text-body-2 mb-3">
      Escolha um recomendado (contexto e ferramentas já testados) ou digite qualquer tag do
      repositório, ex.: <code>qwen3.8:27b</code>. O catálogo completo também baixa por aqui.
    </div>

    <v-alert
      v-if="missingCurrentModel && !aiStore.pullingModelName"
      type="warning"
      variant="tonal"
      density="compact"
      icon="mdi-cloud-download-outline"
      class="mb-3"
    >
      O modelo em uso, <strong>{{ missingCurrentModel }}</strong
      >, ainda não está neste Ollama. Baixe-o abaixo para o chat funcionar.
    </v-alert>

    <!-- Download em andamento: um de cada vez, pelo progresso da store -->
    <div v-if="aiStore.pullingModelName">
      <div class="d-flex align-center justify-space-between ga-2 mb-2">
        <div class="d-flex align-center ga-2">
          <v-progress-circular indeterminate color="primary" size="20" width="2" />
          <span class="font-weight-bold text-body-2">
            Baixando {{ aiStore.pullingModelName }}
          </span>
        </div>
        <v-chip size="small" color="primary" variant="flat" class="font-weight-bold">
          {{ progressPercent ?? 'Baixando' }}
        </v-chip>
      </div>
      <v-progress-linear
        :model-value="aiStore.pullProgress?.percentage ?? 0"
        :indeterminate="aiStore.pullProgress?.percentage == null"
        color="primary"
        height="8"
        rounded
        striped
        class="mb-2"
      />
      <div class="d-flex align-center justify-space-between flex-wrap ga-2 text-body-2">
        <span>{{ aiStore.pullProgress?.status || 'Processando download no Ollama...' }}</span>
        <span v-if="progressBytes">{{ progressBytes }}</span>
      </div>
      <div class="d-flex justify-end mt-2">
        <v-btn
          color="error"
          variant="text"
          size="small"
          prepend-icon="mdi-close-circle-outline"
          @click="aiStore.cancelOllamaPull()"
        >
          Cancelar download
        </v-btn>
      </div>
    </div>

    <template v-else>
      <v-row dense align="center">
        <v-col cols="12" sm="8" md="9">
          <v-combobox
            v-model="target"
            :items="recommended"
            item-title="id"
            item-value="id"
            :return-object="false"
            :custom-filter="filterModelChoices"
            label="Modelo para baixar"
            placeholder="Ex.: ornith-1.5:9b, qwen3.8:27b, mistral"
            variant="outlined"
            density="compact"
            hide-details="auto"
            prepend-inner-icon="mdi-cloud-download-outline"
            clearable
            @update:model-value="(value) => (target = extractModelId(value))"
            @keydown.enter.prevent="requestInstall"
          >
            <template #item="{ item, props: itemProps }">
              <v-list-item
                v-bind="itemProps"
                :title="item.name"
                :subtitle="item.description ?? undefined"
              >
                <template #append>
                  <AiModelChips :item="item" driver="ollama" class="ms-2" />
                </template>
              </v-list-item>
            </template>
          </v-combobox>
        </v-col>
        <v-col cols="12" sm="4" md="3">
          <v-btn
            color="primary"
            variant="flat"
            block
            height="40"
            prepend-icon="mdi-download"
            :disabled="!cleanTarget"
            @click="requestInstall"
          >
            {{ targetInstalled ? 'Baixar de novo' : 'Baixar modelo' }}
          </v-btn>
        </v-col>
      </v-row>

      <div v-if="cleanTarget" class="d-flex align-center flex-wrap ga-1 mt-2">
        <AiModelChips v-if="targetMeta" :item="targetMeta" driver="ollama" />
        <v-chip v-if="!targetInstalled" size="x-small" color="primary" variant="outlined">
          Pronto para baixar
        </v-chip>
      </div>
    </template>

    <v-alert
      v-if="aiStore.pullError"
      type="error"
      variant="tonal"
      density="compact"
      icon="mdi-alert-circle-outline"
      class="mt-3"
      closable
      @click:close="aiStore.pullError = null"
    >
      <div class="font-weight-bold mb-1">Falha ao baixar o modelo no Ollama</div>
      <div class="mb-1">{{ aiStore.pullError }}</div>
      <div>
        Confira se o Ollama está instalado e em execução (ex.: <code>ollama serve</code> no
        terminal).
      </div>
    </v-alert>
  </v-card>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useAiStore } from '@/stores/ai'
import { formatDecimalBytes, formatPercent } from '@/utils/formatters'
import AiModelChips from './AiModelChips.vue'
import {
  filterModelChoices,
  findRecommendedOllamaModel,
  isOllamaModelInstalled,
  recommendedOllamaChoice,
  type AiModelChoice,
} from './aiModelCatalog'
import { extractModelId } from './aiProviders'

/**
 * O único caminho para baixar um modelo no Ollama: o campo, o aviso de modelo
 * em uso ausente (que já preenche o campo) e o progresso. Quem executa o
 * download é o cartão, por `install` — o mesmo pedido do catálogo completo.
 */
const props = defineProps<{
  currentModel?: string | null
}>()

const emit = defineEmits<{
  (e: 'install', modelName: string): void
}>()

const aiStore = useAiStore()
const target = ref('')

const cleanTarget = computed(() => extractModelId(target.value))
const recommended = computed(() => aiStore.recommendedOllamaModels.map(recommendedOllamaChoice))

/** O modelo em uso, quando ele ainda não foi baixado neste Ollama. */
const missingCurrentModel = computed(() => {
  const current = props.currentModel?.trim() ?? ''
  return current && !isOllamaModelInstalled(aiStore.installedOllamaModels, current) ? current : null
})
const targetInstalled = computed(() =>
  isOllamaModelInstalled(aiStore.installedOllamaModels, cleanTarget.value)
)
const targetMeta = computed<AiModelChoice | null>(() => {
  const meta = findRecommendedOllamaModel(aiStore.recommendedOllamaModels, cleanTarget.value)
  return meta ? { ...recommendedOllamaChoice(meta), isInstalled: targetInstalled.value } : null
})

const progressPercent = computed(() => {
  const percentage = aiStore.pullProgress?.percentage
  return percentage == null ? null : formatPercent(percentage, 1)
})
const progressBytes = computed(() => {
  const progress = aiStore.pullProgress
  if (!progress?.completed || !progress.total) return null
  return `${formatDecimalBytes(progress.completed)} de ${formatDecimalBytes(progress.total)}`
})

/** Modelo em uso ausente já vem no campo: baixá-lo é um clique. */
watch(
  missingCurrentModel,
  (missing) => {
    if (missing) target.value = missing
  },
  { immediate: true }
)

function requestInstall() {
  if (cleanTarget.value && !aiStore.pullingModelName) emit('install', cleanTarget.value)
}
</script>
