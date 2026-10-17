<template>
  <v-dialog
    :model-value="modelValue"
    :fullscreen="smAndDown"
    max-width="860"
    scrollable
    persistent
    @update:model-value="(open) => !open && close()"
    @keydown.esc="close"
  >
    <v-card class="wizard" :rounded="smAndDown ? 0 : 'lg'">
      <v-card-title class="d-flex align-center ga-2 pt-4">
        <v-icon color="primary">mdi-speedometer</v-icon>
        <span class="flex-grow-1">Novo teste de velocidade</span>
        <v-btn
          icon="mdi-close"
          variant="text"
          size="small"
          aria-label="Fechar"
          @click="close"
        ></v-btn>
      </v-card-title>

      <nav class="steps px-4" aria-label="Etapas do assistente">
        <ol>
          <li v-for="item in STEPS" :key="item.value">
            <button
              type="button"
              class="step"
              :class="{ current: step === item.value, done: step > item.value }"
              :aria-current="step === item.value ? 'step' : undefined"
              :disabled="item.value > maxReachable"
              @click="goTo(item.value)"
            >
              <span class="step-number">
                <v-icon v-if="step > item.value" size="16" color="white">mdi-check</v-icon>
                <template v-else>{{ item.value }}</template>
              </span>
              <span class="step-title">{{ item.title }}</span>
            </button>
          </li>
        </ol>
      </nav>
      <v-divider class="mt-3"></v-divider>

      <v-card-text class="wizard-body">
        <v-alert
          v-if="!auth.canWrite"
          type="info"
          variant="tonal"
          density="compact"
          icon="mdi-eye-outline"
          class="mb-4"
        >
          {{ READ_ONLY_TEST_REASON }}
        </v-alert>

        <v-window v-model="step" :touch="false">
          <v-window-item :value="1">
            <LinkTestTargetFields v-model="draft" />
            <v-alert
              v-if="showTargetErrors && targetProblems.length > 0"
              type="warning"
              variant="tonal"
              density="compact"
              class="mt-2"
            >
              {{ targetProblems[0] }}
            </v-alert>
          </v-window-item>

          <v-window-item :value="2">
            <LinkTestSpecFields v-model="spec" />
            <div class="estimate text-body-medium mt-2">
              <v-icon size="18" color="primary">mdi-timer-outline</v-icon>
              Cada enlace leva {{ estimatedDurationText(estimateSeconds(spec)) }}.
              {{ draft.kind === 'link' ? '' : 'O caminho mede um enlace de cada vez.' }}
            </div>
          </v-window-item>

          <v-window-item :value="3">
            <div v-if="planning" class="d-flex align-center ga-3 py-8 justify-center">
              <v-progress-circular indeterminate color="primary"></v-progress-circular>
              <span class="text-body-medium">
                Calculando o plano: caminho, motores e o que cada ponta precisa…
              </span>
            </div>
            <v-alert v-else-if="planError" type="error" variant="tonal" class="mb-2">
              <div class="mb-2">{{ planError }}</div>
              <v-btn
                color="error"
                variant="flat"
                size="small"
                prepend-icon="mdi-refresh"
                @click="loadPlan"
              >
                Tentar de novo
              </v-btn>
            </v-alert>
            <LinkTestPlanView v-else-if="plan" :plan="plan" :spec="spec" />
          </v-window-item>
        </v-window>

        <v-alert v-if="startError" type="error" variant="tonal" class="mt-3">
          {{ startError }}
        </v-alert>
      </v-card-text>

      <v-divider></v-divider>
      <v-card-actions class="px-4 py-3 flex-wrap ga-2">
        <v-btn
          v-if="step > 1"
          color="primary"
          variant="outlined"
          prepend-icon="mdi-arrow-left"
          @click="goTo(step - 1)"
        >
          Voltar
        </v-btn>
        <v-spacer></v-spacer>
        <v-btn color="primary" variant="text" @click="close">Cancelar</v-btn>
        <v-btn
          v-if="step < 3"
          color="primary"
          variant="flat"
          append-icon="mdi-arrow-right"
          :disabled="step + 1 > maxReachable && targetProblems.length === 0"
          @click="next"
        >
          {{ step === 1 ? 'Continuar' : 'Ver o plano' }}
        </v-btn>
        <v-tooltip v-else :text="runReasons[0] ?? ''" :disabled="runReasons.length === 0">
          <template #activator="{ props: tip }">
            <span v-bind="tip">
              <v-btn
                color="primary"
                variant="flat"
                prepend-icon="mdi-play"
                :loading="starting"
                :disabled="runReasons.length > 0 || planning"
                @click="startTest"
              >
                Iniciar teste
              </v-btn>
            </span>
          </template>
        </v-tooltip>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
/**
 * Assistente do teste de enlace (ADR 015 §5): o que testar → como → plano
 * com confirmação. Nada roda sem o plano na tela e o "Iniciar teste".
 */
import { computed, ref, watch } from 'vue'
import { useDisplay } from 'vuetify'
import { useAuthStore } from '@/stores/auth'
import { useLinkTestsStore } from '@/stores/linkTests'
import type { LinkTestRequest } from '@/stores/linkTestRequests'
import type { LinkTestPlan, LinkTestSpec } from '@/types/linkTests'
import { estimateSeconds, estimatedDurationText, normalizeSpec } from '@/utils/linkTests/spec'
import {
  READ_ONLY_TEST_REASON,
  draftFromRequest,
  requestFromDraft,
  runBlockers,
  targetErrors,
  type LinkTestTargetDraft,
} from '@/utils/linkTests/wizard'
import LinkTestPlanView from './LinkTestPlanView.vue'
import LinkTestSpecFields from './LinkTestSpecFields.vue'
import LinkTestTargetFields from './LinkTestTargetFields.vue'

const props = defineProps<{
  modelValue: boolean
  /** Pedido que abriu o assistente (menu da topologia, "repetir"). */
  request?: LinkTestRequest | null
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  started: [testId: number]
  /** O plano em tela (ou `null`) — a topologia destaca o caminho. */
  planned: [plan: LinkTestPlan | null]
}>()

const STEPS = [
  { value: 1, title: 'O que testar' },
  { value: 2, title: 'Como' },
  { value: 3, title: 'Plano' },
] as const

const { smAndDown } = useDisplay()
const auth = useAuthStore()
const store = useLinkTestsStore()

const step = ref(1)
const draft = ref<LinkTestTargetDraft>(draftFromRequest(null))
const spec = ref<LinkTestSpec>(normalizeSpec())
const plan = ref<LinkTestPlan | null>(null)
const planning = ref(false)
const planError = ref<string | null>(null)
const starting = ref(false)
const startError = ref<string | null>(null)
const showTargetErrors = ref(false)
let planSeq = 0

const targetProblems = computed(() => targetErrors(draft.value))
/** Pedir o plano já é escrita no backend: perfil somente leitura para no "Como". */
const maxReachable = computed(() => {
  if (targetProblems.value.length > 0) return 1
  return auth.canWrite ? 3 : 2
})
const runReasons = computed(() => runBlockers(plan.value, auth.canWrite))

function describe(err: unknown, fallback: string): string {
  return err instanceof Error && err.message ? err.message : fallback
}

function reset() {
  draft.value = draftFromRequest(props.request)
  spec.value = normalizeSpec(props.request?.spec)
  plan.value = null
  planError.value = null
  startError.value = null
  showTargetErrors.value = false
  // Alvo já conhecido (veio do mapa): começa no "Como", com o "Voltar" à mão.
  step.value = targetErrors(draft.value).length === 0 ? 2 : 1
}

watch(
  () => props.modelValue,
  (open) => {
    if (open) reset()
  },
  { immediate: true }
)

// Mudou o alvo ou os parâmetros: o plano em tela deixa de valer.
watch([draft, spec], () => {
  if (plan.value || planError.value) {
    plan.value = null
    planError.value = null
  }
})

watch(plan, (value) => emit('planned', value))

async function loadPlan() {
  const seq = ++planSeq
  planning.value = true
  planError.value = null
  plan.value = null
  try {
    const result = await store.plan(requestFromDraft(draft.value, spec.value))
    if (seq === planSeq) plan.value = result
  } catch (err: unknown) {
    if (seq === planSeq) planError.value = describe(err, 'Não foi possível calcular o plano.')
  } finally {
    if (seq === planSeq) planning.value = false
  }
}

function goTo(target: number) {
  if (target > maxReachable.value) {
    showTargetErrors.value = true
    return
  }
  step.value = target
  if (target === 3 && !plan.value && !planning.value) void loadPlan()
}

function next() {
  goTo(step.value + 1)
}

async function startTest() {
  if (runReasons.value.length > 0) return
  starting.value = true
  startError.value = null
  try {
    const id = await store.start(requestFromDraft(draft.value, spec.value, true))
    emit('started', id)
    emit('update:modelValue', false)
  } catch (err: unknown) {
    startError.value = describe(err, 'Não foi possível iniciar o teste.')
  } finally {
    starting.value = false
  }
}

function close() {
  planSeq += 1
  planning.value = false
  emit('planned', null)
  emit('update:modelValue', false)
}
</script>

<style scoped>
.wizard-body {
  min-height: 320px;
}
.steps ol {
  list-style: none;
  display: flex;
  gap: 8px;
  padding: 0;
  margin: 0;
  flex-wrap: wrap;
}
.step {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px 6px 6px;
  border-radius: 999px;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.3);
  background: transparent;
  color: rgb(var(--v-theme-on-surface));
  font-weight: 600;
  cursor: pointer;
}
.step:disabled {
  cursor: not-allowed;
  opacity: 0.6;
}
.step:focus-visible {
  outline: 2px solid rgb(var(--v-theme-primary));
  outline-offset: 2px;
}
.step-number {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  border-radius: 50%;
  background: rgb(var(--v-theme-info));
  color: #fff;
  font-size: 0.85rem;
}
.step.current {
  border-color: rgb(var(--v-theme-primary));
  box-shadow: 0 0 0 1px rgb(var(--v-theme-primary));
}
.step.current .step-number {
  background: rgb(var(--v-theme-primary));
}
.step.done .step-number {
  background: rgb(var(--v-theme-success));
}
.estimate {
  display: flex;
  align-items: center;
  gap: 6px;
  color: rgb(var(--v-theme-on-surface));
}
</style>
