<template>
  <v-card elevation="2" class="mb-4" data-test="deploy-guide">
    <v-card-item density="compact" class="py-2">
      <template #prepend>
        <v-icon :color="guide.done ? 'success' : 'primary'">
          {{ guide.done ? 'mdi-check-decagram' : 'mdi-map-marker-path' }}
        </v-icon>
      </template>
      <div class="text-body-1 font-weight-bold">
        {{ guide.done ? 'Tudo pronto: cada push publica sozinho' : 'Guia do deploy contínuo' }}
      </div>
      <div class="text-body-2 font-weight-medium" data-test="guide-summary">
        {{ progress.done }} de {{ progress.total }} etapas
        <template v-if="currentStep"> · próxima: {{ currentStep.title }}</template>
      </div>
      <template #append>
        <div class="d-flex align-center ga-1">
          <v-btn
            v-if="expanded && ai.available.value"
            color="primary"
            variant="tonal"
            size="small"
            prepend-icon="mdi-robot-happy-outline"
            class="hidden-xs"
            data-test="guide-ask-ai"
            @click="askAi(null)"
          >
            Perguntar à IA
          </v-btn>
          <v-btn
            color="primary"
            :variant="expanded ? 'text' : 'tonal'"
            size="small"
            :append-icon="expanded ? 'mdi-chevron-up' : 'mdi-chevron-down'"
            :aria-expanded="expanded"
            data-test="guide-toggle"
            @click="expanded = !expanded"
          >
            {{ expanded ? 'Esconder' : 'Ver passo a passo' }}
          </v-btn>
        </div>
      </template>
    </v-card-item>
    <v-progress-linear
      :model-value="percent"
      :color="guide.done ? 'success' : 'primary'"
      height="4"
      :aria-label="`${progress.done} de ${progress.total} etapas prontas`"
    ></v-progress-linear>

    <v-expand-transition>
      <v-card-text v-if="expanded" class="pt-4">
        <v-row dense role="list" aria-label="Etapas do deploy contínuo">
          <v-col
            v-for="(step, index) in guide.steps"
            :key="step.key"
            cols="6"
            md="4"
            xl="2"
            role="listitem"
          >
            <ChoiceCard
              :selected="selected === step.key"
              :color="GUIDE_STATUS[step.status].color"
              class="pa-3 h-100 d-flex align-start ga-2"
              :aria-current="step.status === 'current' ? 'step' : undefined"
              :aria-label="`Etapa ${index + 1}: ${step.title} — ${GUIDE_STATUS[step.status].label}`"
              :data-test="`guide-step-${step.key}`"
              @click="selected = step.key"
            >
              <v-icon :color="GUIDE_STATUS[step.status].color" class="mt-1">
                {{ GUIDE_STATUS[step.status].icon }}
              </v-icon>
              <div>
                <div class="text-body-2 font-weight-bold">{{ index + 1 }}. {{ step.title }}</div>
                <div class="text-caption" :class="`text-${GUIDE_STATUS[step.status].color}`">
                  {{ GUIDE_STATUS[step.status].label }}
                </div>
              </div>
            </ChoiceCard>
          </v-col>
        </v-row>

        <v-card
          v-if="selectedStep"
          border
          flat
          class="pa-4 mt-3"
          aria-live="polite"
          data-test="guide-panel"
        >
          <div class="d-flex align-center flex-wrap ga-2 mb-2">
            <v-chip
              :color="GUIDE_STATUS[selectedStep.status].color"
              :prepend-icon="GUIDE_STATUS[selectedStep.status].icon"
              size="small"
              variant="flat"
              label
            >
              {{ GUIDE_STATUS[selectedStep.status].label }}
            </v-chip>
            <span class="text-subtitle-1 font-weight-bold">{{ selectedStep.title }}</span>
          </div>
          <div class="text-body-1 font-weight-medium mb-2" data-test="guide-detail">
            {{ selectedStep.detail }}
          </div>
          <div class="text-body-2 mb-4">{{ selectedStep.how }}</div>
          <div class="d-flex flex-wrap align-center ga-2">
            <v-btn
              v-if="selectedStep.action && actionAllowed(selectedStep.action)"
              color="primary"
              variant="flat"
              :prepend-icon="GUIDE_ACTION[selectedStep.action.kind].icon"
              data-test="guide-action"
              @click="emit('action', selectedStep.action)"
            >
              {{ GUIDE_ACTION[selectedStep.action.kind].label }}
            </v-btn>
            <span
              v-else-if="selectedStep.action"
              class="text-body-2 font-weight-medium"
              data-test="guide-admin-only"
            >
              Esta etapa é com um administrador de Servidores.
            </span>
            <v-btn
              v-if="ai.available.value"
              color="primary"
              variant="tonal"
              prepend-icon="mdi-robot-happy-outline"
              data-test="guide-step-ask-ai"
              @click="askAi(selectedStep)"
            >
              Perguntar à IA sobre esta etapa
            </v-btn>
          </div>
        </v-card>
      </v-card-text>
    </v-expand-transition>
  </v-card>
</template>

<script setup lang="ts">
/**
 * O guia ao vivo do deploy contínuo: as etapas até a primeira versão no ar,
 * com o estado real de cada uma (vem do backend, `deploys::guide`, e anda
 * sozinho pelo SSE), o botão que resolve a etapa e o atalho para a IA — que
 * lê o mesmo guia por `get_deploy_guide`.
 */
import { computed, ref, watch } from 'vue'
import ChoiceCard from '@/components/ChoiceCard.vue'
import { useAiAssistant } from '@/composables/useAiAssistant'
import type { DeployGuideAction } from '@/bindings/DeployGuideAction'
import type { DeployGuideStep } from '@/bindings/DeployGuideStep'
import type { DeployGuideView } from '@/bindings/DeployGuideView'
import {
  GUIDE_ACTION,
  GUIDE_STATUS,
  guideActionNeedsAdmin,
  guideProgress,
  guideQuestion,
} from '@/utils/deployPresentation'

const props = defineProps<{ guide: DeployGuideView; canAdmin: boolean }>()
const emit = defineEmits<{ action: [action: DeployGuideAction] }>()

const ai = useAiAssistant()

/** Começa fechado: uma linha com a próxima etapa; o passo a passo abre no botão. */
const expanded = ref(false)

const progress = computed(() => guideProgress(props.guide.steps))
const percent = computed(() =>
  progress.value.total ? (progress.value.done / progress.value.total) * 100 : 0
)

const currentKey = computed(
  () => props.guide.steps.find((step) => step.status === 'current')?.key ?? null
)
const selected = ref<DeployGuideStep['key'] | null>(
  currentKey.value ?? props.guide.steps.at(-1)?.key ?? null
)
// A etapa atual andou (algo ficou pronto): o painel acompanha.
watch(currentKey, (key) => {
  if (key) selected.value = key
})
const currentStep = computed(
  () => props.guide.steps.find((step) => step.key === currentKey.value) ?? null
)
const selectedStep = computed(
  () => props.guide.steps.find((step) => step.key === selected.value) ?? null
)

function actionAllowed(action: DeployGuideAction): boolean {
  return props.canAdmin || !guideActionNeedsAdmin(action)
}

function askAi(step: DeployGuideStep | null) {
  ai.ask(guideQuestion(step))
}
</script>
