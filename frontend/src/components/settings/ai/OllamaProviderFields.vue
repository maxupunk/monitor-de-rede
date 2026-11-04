<template>
  <v-row dense>
    <v-col cols="12" md="6">
      <v-text-field
        v-model="baseUrl"
        label="URL base do Ollama"
        :placeholder="OLLAMA_DEFAULT_BASE_URL"
        variant="outlined"
        density="compact"
        prepend-inner-icon="mdi-server"
        hint="O padrão é o Ollama que sobe com o NetMonitor. Em outra máquina: http://IP:11434/v1"
        hide-details="auto"
        @blur="baseUrl && refresh()"
      />
    </v-col>

    <v-col cols="12" md="6">
      <AiModelPicker
        v-model="model"
        label="Modelo em uso"
        driver="ollama"
        :placeholder="DEFAULT_MODELS.ollama"
        :items="choices"
        :loading="aiStore.loadingOllamaModels"
        clearable
        catalog-label="Ver biblioteca de modelos"
        @refresh="refresh"
        @open-catalog="emit('open-catalog')"
      />
      <v-btn
        variant="text"
        color="primary"
        size="small"
        prepend-icon="mdi-podium"
        class="mt-1 px-1"
        :to="{ name: 'decider-benchmark', query: { tab: 'chat' } }"
      >
        Comparar os modelos de chat no benchmark do NetMonitor
      </v-btn>
    </v-col>

    <v-col cols="12" md="6">
      <v-select
        :model-value="numCtx"
        label="Janela de contexto (num_ctx)"
        :items="numCtxOptions"
        item-title="title"
        item-value="value"
        variant="outlined"
        density="compact"
        prepend-inner-icon="mdi-memory"
        :hint="numCtxHint"
        persistent-hint
        @update:model-value="pickNumCtx"
      />
    </v-col>

    <v-col cols="12">
      <v-switch
        :model-value="trainedInUse"
        color="success"
        density="compact"
        inset
        label="Modelo treinado com as ferramentas do NetMonitor"
        :hint="trainedHint"
        persistent-hint
        @update:model-value="pickTrained"
      >
        <template v-if="trainedByHand" #append>
          <v-btn
            variant="text"
            size="small"
            color="primary"
            prepend-icon="mdi-auto-fix"
            @click="netmonitorTrained = null"
          >
            Detectar pelo modelo
          </v-btn>
        </template>
      </v-switch>
    </v-col>

    <v-col cols="12">
      <v-switch
        v-model="reasoning"
        color="primary"
        density="compact"
        inset
        label="Raciocinar antes de responder"
        hint="Para modelos que pensam (ex.: lfm3). Desligado responde em segundos; ligado pode levar mais de 30 s e, no estilo Direto, ficar sem resposta."
        persistent-hint
      />
    </v-col>

    <v-col v-if="!aiStore.ollamaOnline" cols="12">
      <v-alert type="warning" variant="tonal" density="compact" icon="mdi-server-off">
        <div class="d-flex align-center justify-space-between flex-wrap ga-2">
          <div>
            <div class="font-weight-bold">Ollama fora do ar ou inacessível</div>
            <div class="text-body-2">
              {{
                aiStore.ollamaError ||
                'Não foi possível conectar ao Ollama. Verifique se o serviço está em execução.'
              }}
            </div>
          </div>
          <v-btn
            variant="flat"
            color="warning"
            size="small"
            prepend-icon="mdi-refresh"
            :loading="aiStore.loadingOllamaModels"
            @click="refresh"
          >
            Tentar novamente
          </v-btn>
        </div>
      </v-alert>
    </v-col>

    <v-col cols="12">
      <OllamaModelInstaller :current-model="model" @install="emit('install', $event)" />
    </v-col>
  </v-row>
</template>

<script setup lang="ts">
import { computed, onMounted, watch } from 'vue'
import { useAiStore } from '@/stores/ai'
import AiModelPicker from './AiModelPicker.vue'
import OllamaModelInstaller from './OllamaModelInstaller.vue'
import { formatContextWindow } from '@/utils/formatters'
import {
  findOllamaChoice,
  isOllamaModelInstalled,
  ollamaChoices,
  withTypedModel,
} from './aiModelCatalog'
import {
  DEFAULT_MODELS,
  DEFAULT_OLLAMA_NUM_CTX,
  NETMONITOR_TRAINED_HINT,
  OLLAMA_DEFAULT_BASE_URL,
  ollamaNumCtxFor,
  ollamaNumCtxOptions,
} from './aiProviders'

/** Campos do Ollama local. Montar é escolher o provedor: consulta o servidor. */
const baseUrl = defineModel<string | null | undefined>('baseUrl', { required: true })
const model = defineModel<string | null | undefined>('model', { required: true })
const numCtx = defineModel<number | null | undefined>('numCtx', { required: true })
const reasoning = defineModel<boolean | undefined>('reasoning', { default: false })
/** `null`: detecta pelo modelo; `true`/`false`: escolhido à mão para este modelo. */
const netmonitorTrained = defineModel<boolean | null | undefined>('netmonitorTrained', {
  default: null,
})

const emit = defineEmits<{
  (e: 'open-catalog'): void
  (e: 'install', modelName: string): void
}>()

const aiStore = useAiStore()

const choices = computed(() =>
  withTypedModel(
    ollamaChoices(aiStore.installedOllamaModels, aiStore.recommendedOllamaModels),
    model.value,
    isOllamaModelInstalled(aiStore.installedOllamaModels, model.value)
  )
)

const selectedChoice = computed(() => findOllamaChoice(choices.value, model.value))
/** Janela nativa do modelo escolhido; `null` quando nem a tabela nem o Ollama a informam. */
const modelWindow = computed(() => selectedChoice.value?.contextLength ?? null)

/** Detectado pelo backend para o modelo escolhido (família `lfm3-netmonitor`). */
const trainedDetected = computed(() => selectedChoice.value?.netmonitorTrained ?? false)
const trainedByHand = computed(() => typeof netmonitorTrained.value === 'boolean')
const trainedInUse = computed(() => netmonitorTrained.value ?? trainedDetected.value)
const trainedHint = computed(() => {
  const origin = trainedByHand.value
    ? 'Escolhido à mão para este modelo.'
    : trainedDetected.value
      ? 'Detectado pelo modelo escolhido.'
      : 'Não detectado: ligue se você ajustou este modelo para o NetMonitor.'
  return `${origin} ${NETMONITOR_TRAINED_HINT}`
})

/** Igual à detecção, volta ao automático: assim a próxima troca de modelo acerta sozinha. */
function pickTrained(value: boolean | null) {
  netmonitorTrained.value = value === trainedDetected.value ? null : Boolean(value)
}
const numCtxOptions = computed(() => ollamaNumCtxOptions(modelWindow.value, numCtx.value))

const numCtxHint = computed(() => {
  const parts = [
    modelWindow.value
      ? `Este modelo aceita até ${formatContextWindow(modelWindow.value)} tokens; menos gasta menos memória.`
      : `Janela do modelo não informada: vale a recomendada (${formatContextWindow(DEFAULT_OLLAMA_NUM_CTX)}).`,
  ]
  if ((numCtx.value ?? 0) > DEFAULT_OLLAMA_NUM_CTX) {
    parts.push(
      `Acima de ${formatContextWindow(DEFAULT_OLLAMA_NUM_CTX)}, suba também a OLLAMA_CONTEXT_LENGTH do servidor.`
    )
  }
  parts.push('Vale depois de Salvar.')
  return parts.join(' ')
})

/**
 * Trocar o modelo leva a janela junto: a do modelo, ou a recomendada. Se a
 * janela só chega depois (lista ainda carregando, modelo recém-instalado),
 * ela é aplicada quando chegar — até a pessoa escolher outra à mão. O valor
 * salvo não é tocado ao abrir a tela: só uma troca de modelo o altera.
 */
let followModelWindow = false

function applyModelWindow() {
  numCtx.value = ollamaNumCtxFor(modelWindow.value)
}

function pickNumCtx(value: number | null) {
  followModelWindow = false
  numCtx.value = value
}

watch(model, () => {
  followModelWindow = true
  applyModelWindow()
  // A escolha à mão era para o modelo anterior: o novo é detectado.
  netmonitorTrained.value = null
})
watch(modelWindow, () => {
  if (followModelWindow) applyModelWindow()
})

function refresh() {
  void aiStore.loadOllamaModels(baseUrl.value)
}

onMounted(refresh)
</script>
