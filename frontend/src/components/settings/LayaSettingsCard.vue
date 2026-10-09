<template>
  <v-card class="rounded-lg">
    <v-card-title class="font-weight-bold d-flex align-center justify-space-between flex-wrap ga-2">
      <div class="d-flex align-center flex-wrap ga-2">
        <v-icon color="primary">mdi-tools</v-icon>
        Laya — ferramentas da IA
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
      Um modelo local que decide em milissegundos, sem gastar tokens: quais ferramentas a IA do chat
      recebe, que tipo de aparelho a descoberta achou, qual interface é o uplink, o que cada log
      significa e quais alertas merecem resumo. É opcional — fora do ar, tudo segue como antes.
    </v-card-subtitle>

    <v-divider class="my-2" />

    <v-card-text>
      <!-- 1. Servidor -->
      <div class="step-title">
        <v-avatar size="24" :color="stepColor(serverOnline, 'error')" class="me-2">1</v-avatar>
        Servidor Ollaya
      </div>
      <v-row dense>
        <v-col cols="12" md="6">
          <v-text-field
            v-model="form.baseUrl"
            label="URL do Ollaya"
            placeholder="http://ollaya:11435"
            variant="outlined"
            density="compact"
            hide-details
            prepend-inner-icon="mdi-lan"
            append-inner-icon="mdi-refresh"
            :loading="layaStore.loadingModels"
            @blur="refreshModels"
            @keydown.enter.prevent="refreshModels"
            @click:append-inner="refreshModels"
          />
        </v-col>
        <v-col cols="12" md="6" class="d-flex align-center">
          <v-chip
            v-if="serverOnline"
            color="success"
            variant="tonal"
            prepend-icon="mdi-check-circle"
          >
            Conectado — {{ installedCount }} modelo(s) instalado(s)
          </v-chip>
        </v-col>
      </v-row>
      <v-alert
        v-if="layaStore.models && !serverOnline"
        type="error"
        variant="tonal"
        density="compact"
        class="mt-2"
      >
        {{ layaStore.models.errorMessage }}
        <div class="mt-1">
          O serviço <code>ollaya</code> sobe junto com o <code>docker compose up</code>; confira se
          ele está no ar (<code>docker compose ps</code>) e use <code>http://ollaya:11435</code>.
        </div>
      </v-alert>

      <!-- 2. Modelo -->
      <div class="step-title mt-5">
        <v-avatar size="24" :color="stepColor(modelReady, 'warning')" class="me-2">2</v-avatar>
        Modelo
      </div>
      <div v-if="layaStore.models && !serverOnline" class="text-body-2 text-warning mb-2">
        Conecte o servidor (passo 1) para ver o que está baixado e baixar o modelo.
      </div>
      <v-row dense>
        <v-col cols="12" md="6">
          <v-combobox
            v-model="form.model"
            :items="modelOptions"
            item-title="name"
            item-value="name"
            :return-object="false"
            label="Modelo"
            placeholder="Escolha ou digite, ex.: laya"
            variant="outlined"
            density="compact"
            hide-details
            prepend-inner-icon="mdi-source-branch"
          >
            <template #item="{ props: itemProps, item }">
              <v-list-item
                v-bind="itemProps"
                :title="item.name"
                :subtitle="item.description ?? 'Instalado no Ollaya'"
              >
                <template #append>
                  <v-chip
                    size="x-small"
                    variant="tonal"
                    :color="item.installed ? 'success' : 'warning'"
                  >
                    {{ item.installed ? 'baixado' : 'não baixado' }}
                  </v-chip>
                </template>
              </v-list-item>
            </template>
          </v-combobox>
        </v-col>
        <v-col cols="12" md="6" class="d-flex align-center ga-2">
          <template v-if="modelReady">
            <v-chip color="success" variant="tonal" prepend-icon="mdi-check-circle">Baixado</v-chip>
            <v-chip v-if="modelLoading" color="info" variant="tonal">
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
          <v-btn
            v-else-if="serverOnline && !layaStore.pulling"
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
      <div v-if="layaStore.pulling && layaStore.pullProgress" class="mt-2">
        <div class="d-flex align-center justify-space-between text-body-2 mb-1">
          <span>{{ layaStore.pullProgress.status }}</span>
          <v-btn size="small" color="error" variant="text" @click="layaStore.cancelPull()">
            Cancelar
          </v-btn>
        </div>
        <v-progress-linear
          :model-value="layaStore.pullProgress.percentage ?? 0"
          :indeterminate="layaStore.pullProgress.percentage == null"
          color="primary"
          height="8"
          rounded
        />
      </div>
      <v-alert
        v-else-if="layaStore.pullProgress?.error"
        type="error"
        variant="tonal"
        density="compact"
        class="mt-2"
      >
        {{ layaStore.pullProgress.error }}
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
        um tempo sem uso. {{ layaStore.testing ? 'A resposta sai assim que terminar.' : '' }}
      </v-alert>
      <v-alert
        v-else-if="modelLoadFailed"
        type="error"
        variant="tonal"
        density="compact"
        class="mt-3"
      >
        Não foi possível carregar o modelo: {{ layaStore.modelState?.message }}
      </v-alert>

      <!-- Onde usar -->
      <div class="step-title mt-5">
        <v-icon color="primary" class="me-2">mdi-map-marker-radius-outline</v-icon>
        Onde usar
      </div>
      <div class="text-body-2 mb-2">
        O Laya só sugere, sempre com a confiança ao lado; você confirma com um clique.
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
            <!-- Experimentar: o que o Laya escolheria e com que confiança -->
            <div class="font-weight-bold mb-1">Experimentar</div>
            <div v-if="!modelReady" class="text-body-2 text-warning mb-2">
              Disponível depois que o modelo estiver baixado.
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
                :loading="layaStore.testing && !modelLoading"
                :disabled="!modelReady || layaStore.pulling || modelLoading"
                @click="runTest"
              >
                <v-progress-circular
                  v-if="modelLoading"
                  indeterminate
                  size="16"
                  width="2"
                  class="me-2"
                />
                {{ modelLoading ? 'Carregando modelo…' : 'Ver o que o Laya escolhe' }}
              </v-btn>
            </div>
            <div
              v-if="modelReady && !modelInMemory && !modelLoading && !modelLoadFailed"
              class="text-body-2 mt-2"
            >
              O modelo está fora da memória: o primeiro teste o carrega antes de responder. No chat,
              a pergunta que chegar nesse momento segue sem o Laya enquanto ele carrega.
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
              hint="Passou disso, o chat segue sem a escolha do Laya."
              persistent-hint
              variant="outlined"
              density="compact"
              prepend-inner-icon="mdi-timer-outline"
              class="mb-3"
            />
            <div class="text-body-2">
              A chave de acesso não fica na tela: a API usa a variável
              <code>OLLAYA_API_KEY</code> do servidor, a mesma que o compose entrega ao serviço
              <code>ollaya</code>.
            </div>
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
        :loading="layaStore.saving"
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
  defaultLayaSettings,
  useAiLayaStore,
  type AiLayaSettings,
  type LayaModelOption,
} from '@/stores/aiLaya'
import type { LayaTriageMode } from '@/bindings/LayaTriageMode'
import { formatLatency, formatPercent } from '@/utils/formatters'
import { isLocalOllaya } from '@/utils/ollaya'
import { confirm } from '@/composables/useConfirm'

/** A mesma pergunta que o backend usa quando o campo fica vazio. */
const SAMPLE_QUESTION =
  'Por que o link da filial ficou lento ontem à noite? Mostra um gráfico da latência.'

type FeatureSwitch = 'chatTools' | 'deviceIdentity' | 'interfaces' | 'logEvents'

const FEATURE_SWITCHES: { key: FeatureSwitch; label: string; hint: string }[] = [
  {
    key: 'chatTools',
    label: 'Ferramentas do chat',
    hint: 'Escolhe o que a IA recebe a cada pergunta.',
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
const TRIAGE_MODE_OPTIONS: { value: LayaTriageMode; title: string }[] = [
  { value: 'enforce', title: 'Decidir (pula alerta que não merece)' },
  { value: 'shadow', title: 'Só registrar (resume sempre)' },
]

const emit = defineEmits<{
  (e: 'saved', message: string, color?: string): void
}>()

const layaStore = useAiLayaStore()
const form = ref<AiLayaSettings>(defaultLayaSettings())
const question = ref('')

const serverOnline = computed(() => !!layaStore.models?.online)
const installedCount = computed(() => layaStore.models?.installed.length ?? 0)
/** Testar e ativar só depois que o modelo estiver baixado no Ollaya. */
const modelReady = computed(() => serverOnline.value && layaStore.installed(form.value.model ?? ''))
/** Triagem ligada decide por padrão; "Só registrar" fica no Avançado. */
const triageEnabled = computed({
  get: () => form.value.features.incidentTriage !== 'off',
  set: (on: boolean) => {
    form.value.features.incidentTriage = on ? 'enforce' : 'off'
  },
})
const triageHint = computed(() =>
  form.value.features.incidentTriage === 'shadow'
    ? 'Só registrar: o Laya anota o que decidiria, mas todo alerta continua ganhando resumo.'
    : 'Decidir: alerta que não merece fica sem resumo e ganha o botão "Gerar resumo agora".'
)

/** Passo concluído fica verde; pendente, na cor do que falta. */
function stepColor(done: boolean, pending: string): string {
  return done ? 'success' : pending
}

/** O estado do carregamento vale para o servidor e o modelo do formulário. */
const modelStateHere = computed(() => {
  const state = layaStore.modelState
  const model = form.value.model ?? ''
  return state && state.baseUrl === form.value.baseUrl && state.model === model ? state : null
})
const modelLoading = computed(() => modelStateHere.value?.state === 'loading')
const modelLoadFailed = computed(() => modelStateHere.value?.state === 'failed')
const modelInMemory = computed(
  () => modelStateHere.value?.state === 'ready' || layaStore.inMemory(form.value.model ?? '')
)

function warmUp() {
  void layaStore.warmUp(normalizedForm())
}

/** Desligar é sempre possível; ligar exige o modelo pronto. */
const canToggle = computed(() => form.value.enabled || modelReady.value)

const status = computed(() => {
  if (!layaStore.models) return { label: 'Verificando...', color: 'info', icon: 'mdi-timer-sand' }
  if (!serverOnline.value)
    return { label: 'Ollaya fora do ar', color: 'error', icon: 'mdi-lan-disconnect' }
  if (!modelReady.value)
    return { label: 'Modelo não baixado', color: 'warning', icon: 'mdi-download' }
  if (!layaStore.saved?.enabled)
    return { label: 'Pronto, desligado', color: 'info', icon: 'mdi-power' }
  return { label: 'Ativo', color: 'success', icon: 'mdi-check-circle' }
})

/** Catálogo e instalados; o modelo digitado à mão também aparece na lista. */
const modelOptions = computed<LayaModelOption[]>(() => {
  const options = layaStore.models?.options ?? []
  const typed = (form.value.model ?? '').trim()
  if (!typed || options.some((option) => option.name === typed)) return options
  return [
    ...options,
    { name: typed, description: 'Digitado', installed: layaStore.installed(typed) },
  ]
})

const result = computed(() => layaStore.testResult)
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
function normalizedForm(): AiLayaSettings {
  return {
    ...form.value,
    model: (form.value.model ?? '').trim(),
    timeoutMs: Number(form.value.timeoutMs) || defaultLayaSettings().timeoutMs,
  }
}

const dirty = computed(
  () => !!layaStore.saved && JSON.stringify(normalizedForm()) !== JSON.stringify(layaStore.saved)
)

/**
 * Ligar com o Ollaya nesta máquina custa RAM do servidor: avisa antes. Com o
 * Ollaya em outra máquina não há o que avisar — a memória gasta é a dela.
 */
async function toggleEnabled(value: boolean | null) {
  if (value && isLocalOllaya(form.value.baseUrl)) {
    const ok = await confirm({
      title: 'Ligar o Laya neste servidor?',
      message:
        'O Ollaya roda nesta mesma máquina. Enquanto um modelo de decisão estiver carregado, ' +
        'ele usa aproximadamente 3 GB a 4 GB de RAM. Sem uso, o modelo é descarregado e o ' +
        'serviço volta a ocupar cerca de 150 MB.',
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
  void layaStore.loadModels(form.value.baseUrl)
}

function runTest() {
  void layaStore.test(normalizedForm(), question.value)
}

function runPull() {
  void layaStore.pull(normalizedForm(), question.value)
}

function discard() {
  if (layaStore.saved) form.value = { ...layaStore.saved }
  refreshModels()
}

async function save() {
  const error = await layaStore.save(normalizedForm())
  if (error) {
    emit('saved', error, 'error')
    return
  }
  discard()
  emit('saved', 'Laya salvo com sucesso!', 'success')
}

onMounted(async () => {
  try {
    form.value = { ...(await layaStore.load()) }
  } catch {
    emit('saved', 'Não foi possível carregar a configuração do Laya', 'error')
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
