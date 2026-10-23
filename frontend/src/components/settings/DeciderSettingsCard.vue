<template>
  <v-card class="rounded-lg">
    <v-card-title class="font-weight-bold d-flex align-center justify-space-between flex-wrap ga-2">
      <div class="d-flex align-center flex-wrap ga-2">
        <v-icon color="primary">mdi-tools</v-icon>
        Decisor — escolhas rápidas da IA
        <v-chip size="small" variant="tonal" :color="status.color" :prepend-icon="status.icon">
          {{ status.label }}
        </v-chip>
      </div>
      <v-tooltip
        :disabled="canToggle"
        text="Baixe o modelo (passo 2) para poder ativar"
        location="top"
      >
        <template #activator="{ props: tooltipProps }">
          <div v-bind="tooltipProps">
            <v-switch
              :model-value="form.enabled"
              color="primary"
              density="compact"
              hide-details
              label="Ativo"
              :disabled="!canToggle"
              @update:model-value="toggleEnabled"
            />
          </div>
        </template>
      </v-tooltip>
    </v-card-title>

    <v-card-subtitle class="text-wrap">
      Um modelo de decisão — no Ollama desta máquina, em outra da sua rede ou na nuvem — que decide
      em milissegundos, sem gastar tokens do chat: quais ferramentas a IA recebe, que tipo de
      aparelho a descoberta achou, qual interface é o uplink, o que cada log significa e quais
      alertas merecem resumo. É opcional — fora do ar, tudo segue como antes.
    </v-card-subtitle>

    <v-divider class="my-2" />

    <v-card-text>
      <!-- 1. Provedor -->
      <div class="step-title">
        <v-avatar size="24" :color="stepColor(providerReady, 'error')" class="me-2">1</v-avatar>
        Onde o Decisor decide
      </div>
      <DeciderProviderStep v-model:form="form" @refresh="refreshModels" @test="runTest" />

      <!-- 2. Modelo -->
      <div class="step-title mt-5">
        <v-avatar size="24" :color="stepColor(modelReady, 'warning')" class="me-2">2</v-avatar>
        Modelo
      </div>
      <div
        v-if="managesModels && deciderStore.models && !serverOnline"
        class="text-body-2 text-warning mb-2"
      >
        Conecte o Ollama (passo 1) para ver o que está baixado e baixar o modelo.
      </div>
      <v-row dense>
        <v-col cols="12" md="6">
          <v-combobox
            :model-value="form.model"
            :items="modelOptions"
            item-title="name"
            item-value="name"
            :return-object="false"
            label="Modelo"
            :placeholder="modelPlaceholder"
            variant="outlined"
            density="compact"
            hide-details
            prepend-inner-icon="mdi-source-branch"
            @update:model-value="chooseModel"
          >
            <template #item="{ props: itemProps, item }">
              <v-list-item
                v-bind="itemProps"
                :title="item.name"
                :subtitle="item.description ?? 'Instalado no Ollama'"
              >
                <template #append>
                  <div class="d-flex ga-1">
                    <v-chip
                      v-if="item.spec?.recommended"
                      size="x-small"
                      color="primary"
                      variant="flat"
                    >
                      recomendado
                    </v-chip>
                    <v-chip
                      v-if="item.decisions"
                      size="x-small"
                      variant="tonal"
                      :color="decisionsFact(item.decisions).color"
                      :title="`${decisionsFact(item.decisions).label} no benchmark do NetMonitor`"
                      prepend-icon="mdi-podium"
                    >
                      {{ decisionsFact(item.decisions).short }}
                    </v-chip>
                    <v-chip
                      v-else-if="item.benchmark"
                      size="x-small"
                      variant="tonal"
                      :color="benchmarkFact(item.benchmark).color"
                      :title="`${benchmarkFact(item.benchmark).label} no teste público`"
                    >
                      {{ benchmarkFact(item.benchmark).short }}
                    </v-chip>
                    <v-chip
                      v-else-if="item.spec"
                      size="x-small"
                      variant="tonal"
                      :color="languageFact(item.spec.language).color"
                      :title="languageFact(item.spec.language).label"
                    >
                      {{ languageFact(item.spec.language).short }}
                    </v-chip>
                    <v-chip
                      v-if="item.spec?.gpuRecommended"
                      size="x-small"
                      variant="tonal"
                      color="warning"
                      title="GPU recomendada"
                    >
                      GPU
                    </v-chip>
                    <v-chip
                      size="x-small"
                      variant="tonal"
                      :color="item.installed ? 'success' : 'warning'"
                    >
                      {{ item.installed ? 'baixado' : 'não baixado' }}
                    </v-chip>
                  </div>
                </template>
              </v-list-item>
            </template>
          </v-combobox>
        </v-col>
        <v-col v-if="managesModels" cols="12" md="6" class="d-flex align-center ga-2">
          <template v-if="modelReady">
            <v-chip color="success" variant="tonal" prepend-icon="mdi-check-circle">Baixado</v-chip>
            <v-chip v-if="modelInstalling" color="info" variant="tonal">
              <v-progress-circular indeterminate size="14" width="2" class="me-2" />
              Instalando no Ollama… {{ modelStateHere?.percentage ?? 0 }}%
            </v-chip>
            <v-chip v-else-if="modelLoading" color="info" variant="tonal">
              <v-progress-circular indeterminate size="14" width="2" class="me-2" />
              Carregando na memória…
            </v-chip>
            <v-chip
              v-else-if="modelInMemory"
              color="success"
              variant="tonal"
              prepend-icon="mdi-memory"
            >
              Na memória
            </v-chip>
            <v-btn
              v-else
              color="primary"
              variant="tonal"
              size="small"
              prepend-icon="mdi-memory"
              @click="warmUp"
            >
              Carregar agora
            </v-btn>
          </template>
          <v-chip
            v-else-if="selectedSpec?.bundled"
            color="warning"
            variant="tonal"
            prepend-icon="mdi-package-variant-closed"
          >
            Vem com o NetMonitor
          </v-chip>
          <v-btn
            v-else-if="serverOnline && !deciderStore.pulling"
            color="warning"
            variant="flat"
            prepend-icon="mdi-download"
            :disabled="!form.model"
            @click="runPull"
          >
            Baixar {{ form.model }}
          </v-btn>
        </v-col>
      </v-row>
      <v-btn
        variant="text"
        color="primary"
        size="small"
        prepend-icon="mdi-podium"
        class="mt-1 px-1"
        :to="{ name: 'decider-benchmark' }"
      >
        Comparar os modelos no benchmark do NetMonitor
      </v-btn>

      <!-- Ficha do modelo escolhido: o que pesa na escolha -->
      <template v-if="selectedSpec || selectedBenchmark || selectedDecisions">
        <div class="d-flex flex-wrap ga-1 mt-2">
          <v-chip
            v-for="fact in selectedFacts"
            :key="fact.label"
            size="small"
            :color="fact.color"
            variant="tonal"
            :prepend-icon="fact.icon"
          >
            {{ fact.label }}
          </v-chip>
        </div>
        <v-alert
          v-if="managesModels && serverOnline && selectedSpec?.bundled && !modelReady"
          type="info"
          variant="tonal"
          density="compact"
          class="mt-2 text-body-2"
        >
          Este modelo vem com o NetMonitor: <strong>Baixar modelo</strong> o instala no Ollama a
          partir da release do projeto (<code>DECIDER_MODELS_URL</code> no servidor, para usar um
          espelho). Ligar o Decisor também instala, se ainda faltar.
        </v-alert>
        <v-alert
          v-if="nearChance"
          type="warning"
          variant="tonal"
          density="compact"
          class="mt-2 text-body-2"
        >
          <template v-if="selectedDecisions">
            No benchmark das decisões do NetMonitor, este modelo fez
          </template>
          <template v-else>
            No
            <a :href="BENCHMARK_URL" target="_blank" rel="noopener" class="text-primary"
              >teste público</a
            >
            (23 modelos, em inglês e português), este modelo acertou
          </template>
          {{ nearChance }} — perto de escolher no chute (~31%). Prefira
          <strong>{{ RECOMMENDED_MODEL }}</strong
          >, ajustado nas decisões do NetMonitor.
        </v-alert>
        <v-alert
          v-else-if="
            selectedSpec?.language === 'english' && !selectedBenchmark && !selectedDecisions
          "
          type="info"
          variant="tonal"
          density="compact"
          class="mt-2 text-body-2"
        >
          Este modelo não declara português e não foi avaliado. O texto analisado (perguntas do
          chat, logs, nomes) costuma estar em português: prefira um avaliado —
          <strong>{{ RECOMMENDED_MODEL }}</strong> — ou experimente com perguntas reais em Avançado
          › Experimentar antes de ligar.
        </v-alert>
        <v-alert
          v-if="selectedSpec?.gpuRecommended"
          type="warning"
          variant="tonal"
          density="compact"
          class="mt-2 text-body-2"
        >
          Em CPU, cada pergunta do chat pode esperar alguns segundos por este modelo. Para o dia a
          dia, dê a GPU ao Ollama (<code>gpus: all</code> no serviço <code>ollama</code> do compose)
          ou aponte o passo 1 para um Ollama numa máquina com GPU.
        </v-alert>
        <div v-if="timeoutNote" class="d-flex align-center ga-1 mt-2 text-body-2">
          <v-icon size="16" color="info">mdi-timer-cog-outline</v-icon>
          {{ timeoutNote }}
        </div>
      </template>
      <div v-if="deciderStore.pulling && deciderStore.pullProgress" class="mt-2">
        <div class="d-flex align-center justify-space-between text-body-2 mb-1">
          <span>{{ deciderStore.pullProgress.status }}</span>
          <v-btn size="small" color="error" variant="text" @click="deciderStore.cancelPull()">
            Cancelar
          </v-btn>
        </div>
        <v-progress-linear
          :model-value="deciderStore.pullProgress.percentage ?? 0"
          :indeterminate="deciderStore.pullProgress.percentage == null"
          color="primary"
          height="8"
          rounded
        />
      </div>
      <v-alert
        v-else-if="pullErrorHere"
        type="error"
        variant="tonal"
        density="compact"
        class="mt-2"
      >
        {{ pullErrorHere }}
      </v-alert>

      <v-alert
        v-if="modelLoading"
        type="info"
        variant="tonal"
        density="compact"
        class="mt-3"
        icon="mdi-memory"
      >
        Tirando o modelo do disco para a memória — leva de 5 a 20 segundos na primeira vez depois de
        um tempo sem uso. {{ deciderStore.testing ? 'A resposta sai assim que terminar.' : '' }}
      </v-alert>
      <v-alert
        v-else-if="modelLoadFailed"
        type="error"
        variant="tonal"
        density="compact"
        class="mt-3"
      >
        Não foi possível carregar o modelo: {{ deciderStore.modelState?.message }}
      </v-alert>

      <!-- Onde usar -->
      <div class="step-title mt-5">
        <v-icon color="primary" class="me-2">mdi-map-marker-radius-outline</v-icon>
        Onde usar
      </div>
      <div class="text-body-2 mb-2">
        O Decisor só sugere, sempre com a confiança ao lado; você confirma com um clique.
      </div>
      <v-row dense>
        <v-col v-for="feature in FEATURE_SWITCHES" :key="feature.key" cols="12" md="6">
          <v-switch
            v-model="form.features[feature.key]"
            color="primary"
            density="compact"
            hide-details
            :label="feature.label"
          />
          <div class="text-body-2 ms-12 mt-n1">{{ feature.hint }}</div>
        </v-col>
        <v-col cols="12" md="6">
          <v-switch
            v-model="triageEnabled"
            color="primary"
            density="compact"
            hide-details
            label="Triagem da IA proativa"
          />
          <div class="text-body-2 ms-12 mt-n1">
            Decide se um alerta merece resumo do LLM, poupando tokens.
          </div>
        </v-col>
      </v-row>

      <v-expansion-panels variant="accordion" class="mt-4">
        <v-expansion-panel title="Avançado">
          <v-expansion-panel-text>
            <!-- Experimentar: o que o Decisor escolheria e com que confiança -->
            <div class="font-weight-bold mb-1">Experimentar</div>
            <div v-if="!modelReady" class="text-body-2 text-warning mb-2">
              {{
                managesModels
                  ? 'Disponível depois que o modelo estiver baixado.'
                  : 'Disponível depois de preencher o passo 1.'
              }}
            </div>
            <div class="d-flex flex-wrap align-center ga-2">
              <v-text-field
                v-model="question"
                label="Pergunta de exemplo"
                :placeholder="SAMPLE_QUESTION"
                variant="outlined"
                density="compact"
                hide-details
                class="flex-grow-1 question-field"
                :disabled="!modelReady"
                @keydown.enter.prevent="runTest"
              />
              <v-btn
                color="primary"
                variant="flat"
                prepend-icon="mdi-flask-outline"
                :loading="deciderStore.testing && !modelLoading"
                :disabled="!modelReady || deciderStore.pulling || modelLoading"
                @click="runTest"
              >
                <v-progress-circular
                  v-if="modelLoading"
                  indeterminate
                  size="16"
                  width="2"
                  class="me-2"
                />
                {{ modelLoading ? 'Carregando modelo…' : 'Ver o que o Decider escolhe' }}
              </v-btn>
            </div>
            <div
              v-if="
                managesModels && modelReady && !modelInMemory && !modelLoading && !modelLoadFailed
              "
              class="text-body-2 mt-2"
            >
              O modelo está fora da memória: o primeiro teste o carrega antes de responder. No chat,
              a pergunta que chegar nesse momento segue sem o Decisor enquanto ele carrega.
            </div>

            <template v-if="result">
              <v-alert :type="resultType" variant="tonal" density="compact" class="mt-3">
                {{ result.message }}
                <div v-if="result.success" class="d-flex flex-wrap ga-2 mt-1">
                  <v-chip size="small" color="primary" variant="tonal" prepend-icon="mdi-chip">
                    {{ result.model }}
                  </v-chip>
                  <v-chip
                    size="small"
                    color="info"
                    variant="tonal"
                    prepend-icon="mdi-timer-outline"
                  >
                    {{ formatLatency(result.latencyMs) }}
                  </v-chip>
                </div>
              </v-alert>

              <div v-if="result.groups.length" class="mt-3">
                <div v-for="group in scoredGroups" :key="group.id" class="mb-2">
                  <div class="d-flex align-center justify-space-between text-body-2">
                    <span>
                      <v-icon
                        size="16"
                        :color="group.selected ? 'success' : 'info'"
                        :icon="group.selected ? 'mdi-check-circle' : 'mdi-minus-circle-outline'"
                      />
                      <strong class="ms-1">{{ group.id }}</strong>
                      <span class="ms-1">— {{ group.purpose }}</span>
                    </span>
                    <strong>{{ formatPercent(group.probability * 100, 0) }}</strong>
                  </div>
                  <v-progress-linear
                    :model-value="group.probability * 100"
                    :color="group.selected ? 'success' : 'info'"
                    height="6"
                    rounded
                  />
                </div>
              </div>
            </template>

            <v-divider class="my-4" />

            <div class="text-body-2">
              Confiança mínima para uma ferramenta ir à IA:
              <strong>{{ formatPercent(form.minConfidence, 0) }}</strong>
              <span v-if="result?.groups.length"> — em verde, o que iria com este valor.</span>
            </div>
            <v-slider
              v-model="form.minConfidence"
              :min="30"
              :max="95"
              :step="5"
              color="primary"
              hide-details
              class="mb-3"
            />

            <v-select
              v-model="form.features.incidentTriage"
              :items="TRIAGE_MODE_OPTIONS"
              label="Modo da triagem da IA proativa"
              variant="outlined"
              density="compact"
              prepend-inner-icon="mdi-filter-check-outline"
              :disabled="!triageEnabled"
              :hint="triageHint"
              persistent-hint
              class="mb-3"
            />

            <v-text-field
              v-model.number="form.timeoutMs"
              type="number"
              min="200"
              max="10000"
              step="100"
              label="Tempo máximo por pergunta (ms)"
              hint="Passou disso, o chat segue sem a escolha do Decisor."
              persistent-hint
              variant="outlined"
              density="compact"
              prepend-inner-icon="mdi-timer-outline"
              class="mb-3"
            />
          </v-expansion-panel-text>
        </v-expansion-panel>
      </v-expansion-panels>
    </v-card-text>

    <v-card-actions class="pa-4 pt-0 justify-end ga-2">
      <span v-if="dirty" class="text-body-2 text-warning me-auto">Alterações não salvas</span>
      <v-btn v-if="dirty" color="primary" variant="text" @click="discard">Descartar</v-btn>
      <v-btn
        color="primary"
        variant="flat"
        prepend-icon="mdi-content-save-outline"
        :disabled="!dirty"
        :loading="deciderStore.saving"
        @click="save"
      >
        Salvar
      </v-btn>
    </v-card-actions>
  </v-card>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import {
  defaultDeciderSettings,
  useAiDeciderStore,
  type AiDeciderSettings,
  type DeciderModelOption,
} from '@/stores/aiDecider'
import type { DeciderTriageMode } from '@/bindings/DeciderTriageMode'
import { formatElapsedMs, formatLatency, formatPercent } from '@/utils/formatters'
import {
  BENCHMARK_URL,
  RECOMMENDED_MODEL,
  benchmarkFact,
  decisionsFact,
  isLocalOllama,
  isNearChance,
  percent,
  primaryScore,
  languageFact,
  memoryLabel,
  modelFacts,
  providerMissing,
  timeoutForModel,
} from '@/utils/decider'
import { confirm } from '@/composables/useConfirm'
import DeciderProviderStep from './decider/DeciderProviderStep.vue'

/** A mesma pergunta que o backend usa quando o campo fica vazio. */
const SAMPLE_QUESTION =
  'Por que o link da filial ficou lento ontem à noite? Mostra um gráfico da latência.'

type FeatureSwitch = 'chatTools' | 'actionReview' | 'deviceIdentity' | 'interfaces' | 'logEvents'

const FEATURE_SWITCHES: { key: FeatureSwitch; label: string; hint: string }[] = [
  {
    key: 'chatTools',
    label: 'Ferramentas do chat',
    hint: 'Escolhe o que a IA recebe a cada pergunta.',
  },
  {
    key: 'actionReview',
    label: 'Revisar ações automáticas',
    hint: 'No modo automático, manda para a sua confirmação a ação que parecer perigosa no contexto da conversa. Só deixa a IA mais cautelosa.',
  },
  {
    key: 'deviceIdentity',
    label: 'Identificar dispositivos',
    hint: 'Sugere tipo e sistema do que a descoberta encontrou.',
  },
  {
    key: 'interfaces',
    label: 'Uplink e interfaces',
    hint: 'Sugere o uplink e as interfaces que valem monitorar.',
  },
  {
    key: 'logEvents',
    label: 'Classificar eventos de log',
    hint: 'Dá categoria a cada padrão novo de log, em segundo plano.',
  },
]

/** Modos da triagem ligada; desligar é a chave em "Onde usar". */
const TRIAGE_MODE_OPTIONS: { value: DeciderTriageMode; title: string }[] = [
  { value: 'enforce', title: 'Decidir (pula alerta que não merece)' },
  { value: 'shadow', title: 'Só registrar (resume sempre)' },
]

const emit = defineEmits<{
  (e: 'saved', message: string, color?: string): void
}>()

const deciderStore = useAiDeciderStore()
const form = ref<AiDeciderSettings>(defaultDeciderSettings())
const question = ref('')

/** O provedor do formulário e o que falta para ele responder. */
const currentProvider = computed(() => deciderStore.provider(form.value.provider))
/** Só o Ollama baixa e carrega modelos; na nuvem eles já estão prontos. */
const managesModels = computed(() => currentProvider.value?.managesModels ?? true)
const providerGap = computed(() => providerMissing(form.value, currentProvider.value))
const serverOnline = computed(() => !!deciderStore.models?.online)
/** Passo 1 pronto: Ollama respondendo, ou nuvem com tudo preenchido. */
const providerReady = computed(() =>
  managesModels.value ? serverOnline.value : !providerGap.value
)
/** Exemplo de modelo no campo vazio: o padrão do provedor, ou o do Jev no "Outro". */
const modelPlaceholder = computed(
  () => `Escolha ou digite, ex.: ${currentProvider.value?.defaultModel || 'jev-latest'}`
)
/** Testar e ativar: no Ollama, com o modelo baixado; na nuvem, com o passo 1 completo. */
const modelReady = computed(() =>
  managesModels.value
    ? serverOnline.value && deciderStore.installed(form.value.model ?? '')
    : !providerGap.value
)
/** Triagem ligada decide por padrão; "Só registrar" fica no Avançado. */
const triageEnabled = computed({
  get: () => form.value.features.incidentTriage !== 'off',
  set: (on: boolean) => {
    form.value.features.incidentTriage = on ? 'enforce' : 'off'
  },
})
const triageHint = computed(() =>
  form.value.features.incidentTriage === 'shadow'
    ? 'Só registrar: o Decisor anota o que decidiria, mas todo alerta continua ganhando resumo.'
    : 'Decidir: alerta que não merece fica sem resumo e ganha o botão "Gerar resumo agora".'
)

/** Passo concluído fica verde; pendente, na cor do que falta. */
function stepColor(done: boolean, pending: string): string {
  return done ? 'success' : pending
}

/** O estado do carregamento vale para o servidor e o modelo do formulário. */
const modelStateHere = computed(() => {
  const state = deciderStore.modelState
  const model = form.value.model ?? ''
  return state && state.baseUrl === form.value.baseUrl && state.model === model ? state : null
})
/** O erro do último download, só enquanto o formulário estiver no mesmo modelo e servidor. */
const pullErrorHere = computed(() => {
  const target = deciderStore.pullTarget
  if (!target || target.model !== form.value.model || target.baseUrl !== form.value.baseUrl)
    return null
  return deciderStore.pullProgress?.error ?? null
})
const modelLoading = computed(() => modelStateHere.value?.state === 'loading')
const modelInstalling = computed(() => modelStateHere.value?.state === 'installing')
const modelLoadFailed = computed(() => modelStateHere.value?.state === 'failed')
const modelInMemory = computed(
  () => modelStateHere.value?.state === 'ready' || deciderStore.inMemory(form.value.model ?? '')
)

function warmUp() {
  void deciderStore.warmUp(normalizedForm())
}

/** Desligar é sempre possível; ligar exige o modelo pronto. */
const canToggle = computed(() => form.value.enabled || modelReady.value)

const status = computed(() => {
  if (!deciderStore.models)
    return { label: 'Verificando...', color: 'info', icon: 'mdi-timer-sand' }
  if (!managesModels.value && providerGap.value)
    return { label: providerGap.value, color: 'warning', icon: 'mdi-key-alert-outline' }
  if (!serverOnline.value)
    return { label: 'Ollama fora do ar', color: 'error', icon: 'mdi-lan-disconnect' }
  if (!modelReady.value)
    return { label: 'Modelo não baixado', color: 'warning', icon: 'mdi-download' }
  if (!deciderStore.saved?.enabled)
    return { label: 'Pronto, desligado', color: 'info', icon: 'mdi-power' }
  return { label: 'Ativo', color: 'success', icon: 'mdi-check-circle' }
})

/** A ficha do modelo do formulário; `null` para um fora do catálogo. */
const selectedSpec = computed(() => deciderStore.spec(form.value.model ?? ''))
const selectedBenchmark = computed(() => deciderStore.benchmark(form.value.model ?? ''))
const selectedDecisions = computed(() => deciderStore.decisions(form.value.model ?? ''))
const selectedFacts = computed(() =>
  modelFacts(selectedSpec.value, selectedBenchmark.value, selectedDecisions.value)
)
/** O acerto do modelo escolhido quando ele fica perto do chute; `null` quando não. */
const nearChance = computed(() => {
  const score = primaryScore(selectedBenchmark.value, selectedDecisions.value)
  return isNearChance(score) ? percent(score) : null
})

/** Avisa quando trocar de modelo mexeu no tempo máximo por pergunta. */
const timeoutNote = ref<string | null>(null)

/**
 * Escolha do usuário no seletor (não a carga do formulário): o tempo máximo
 * acompanha o sugerido do modelo, e a tela diz que mudou.
 */
function chooseModel(next: string | null) {
  const previous = deciderStore.spec(form.value.model ?? '')
  const nextSpec = deciderStore.spec(next ?? '')
  const current = Number(form.value.timeoutMs) || defaultDeciderSettings().timeoutMs
  const timeout = timeoutForModel(current, previous, nextSpec, defaultDeciderSettings().timeoutMs)
  timeoutNote.value =
    timeout === current
      ? null
      : `Tempo máximo por pergunta ajustado para ${formatElapsedMs(timeout)}, o sugerido para ${next}.`
  form.value.timeoutMs = timeout
  form.value.model = next ?? ''
}

/** Catálogo e instalados; o modelo digitado à mão também aparece na lista. */
const modelOptions = computed<DeciderModelOption[]>(() => {
  const options = deciderStore.models?.options ?? []
  const typed = (form.value.model ?? '').trim()
  if (!typed || options.some((option) => option.name === typed)) return options
  return [
    ...options,
    { name: typed, description: 'Digitado', installed: deciderStore.installed(typed) },
  ]
})

const result = computed(() => deciderStore.testResult)
const resultType = computed(() => {
  if (!result.value) return 'info'
  if (result.value.success) return 'success'
  return result.value.online && !result.value.modelInstalled ? 'warning' : 'error'
})

/** Recalcula o "iria para a IA" pelo limiar do formulário, não o do teste. */
const scoredGroups = computed(() => {
  const threshold = form.value.minConfidence / 100
  return (result.value?.groups ?? []).map((group) => ({
    ...group,
    selected: group.probability >= threshold,
  }))
})

/** Campo numérico apagado chega como '' — o backend exige um inteiro. */
function normalizedForm(): AiDeciderSettings {
  return {
    ...form.value,
    model: (form.value.model ?? '').trim(),
    timeoutMs: Number(form.value.timeoutMs) || defaultDeciderSettings().timeoutMs,
  }
}

const dirty = computed(
  () =>
    !!deciderStore.saved && JSON.stringify(normalizedForm()) !== JSON.stringify(deciderStore.saved)
)

/** Quanto o modelo escolhido ocupa carregado, para o aviso ao ligar. */
const loadedMemory = computed(() =>
  selectedSpec.value ? memoryLabel(selectedSpec.value) : 'alguns GB'
)

/**
 * Ligar avisa o que custa: na nuvem, os textos saem da rede e o uso é cobrado;
 * com o Ollama nesta máquina, RAM do servidor. Com o Ollama em outra máquina
 * não há o que avisar — a memória gasta é a dela.
 */
async function toggleEnabled(value: boolean | null) {
  const provider = currentProvider.value
  if (value && provider?.cloud) {
    const ok = await confirm({
      title: `Ligar o Decisor com ${provider.label}?`,
      message:
        `Cada decisão manda a ${provider.label} o texto que ela analisa: perguntas do chat, ` +
        'mensagens de log, nomes, IPs e descrições de dispositivos e interfaces. O uso é ' +
        'cobrado na sua conta, por token de entrada. Para nada sair da rede, use o Ollama.',
      confirmText: 'Ligar e enviar',
      confirmColor: 'warning',
      icon: 'mdi-cloud-alert-outline',
      iconColor: 'warning',
    })
    if (!ok) return
  } else if (value && isLocalOllama(form.value.baseUrl)) {
    const ok = await confirm({
      title: 'Ligar o Decisor neste servidor?',
      message:
        'O Ollama roda nesta mesma máquina. Enquanto o modelo ' +
        `${form.value.model} estiver carregado, ele usa ${loadedMemory.value} de memória. ` +
        'Sem uso, o modelo é descarregado e o serviço volta a ocupar cerca de 50 MB.',
      confirmText: 'Ligar mesmo assim',
      confirmColor: 'warning',
      icon: 'mdi-memory',
      iconColor: 'warning',
    })
    if (!ok) return
  }
  form.value.enabled = Boolean(value)
}

function refreshModels() {
  void deciderStore.loadModels(normalizedForm())
}

function runTest() {
  void deciderStore.test(normalizedForm(), question.value)
}

function runPull() {
  void deciderStore.pull(normalizedForm(), question.value)
}

function discard() {
  if (deciderStore.saved) form.value = { ...deciderStore.saved }
  refreshModels()
}

async function save() {
  const error = await deciderStore.save(normalizedForm())
  if (error) {
    emit('saved', error, 'error')
    return
  }
  discard()
  emit('saved', 'Decisor salvo com sucesso!', 'success')
}

onMounted(async () => {
  void deciderStore.loadProviders()
  try {
    form.value = { ...(await deciderStore.load()) }
  } catch {
    emit('saved', 'Não foi possível carregar a configuração do Decisor', 'error')
  }
  refreshModels()
})
</script>

<style scoped>
.step-title {
  display: flex;
  align-items: center;
  font-weight: 700;
  margin-bottom: 8px;
}

.question-field {
  min-width: 240px;
}
</style>
