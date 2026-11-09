<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 900"
    :fullscreen="$vuetify.display.xs"
    scrollable
    persistent
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card class="rounded-lg">
      <v-card-item class="pt-4 px-6">
        <template #prepend>
          <v-avatar color="primary" variant="tonal" rounded="lg">
            <v-icon>mdi-rocket-launch</v-icon>
          </v-avatar>
        </template>
        <v-card-title class="font-weight-bold">Automatizar deploy</v-card-title>
        <v-card-subtitle class="text-wrap text-high-emphasis">
          Passo {{ step }} de 3 — {{ STEP_TITLES[step - 1] }}
        </v-card-subtitle>
      </v-card-item>
      <v-progress-linear :model-value="(step / 3) * 100" color="primary"></v-progress-linear>

      <v-card-text class="px-6">
        <v-alert
          v-if="error"
          type="error"
          variant="tonal"
          closable
          class="mb-4"
          @click:close="error = null"
        >
          {{ error }}
        </v-alert>

        <!-- 1. A aplicação -->
        <v-form v-show="step === 1" ref="applicationForm" autocomplete="off" @submit.prevent>
          <template v-if="deploys.applications.length">
            <div class="text-body-1 mb-3">De qual repositório vêm as versões?</div>
            <v-row dense class="mb-2">
              <v-col cols="12" sm="6">
                <ChoiceCard
                  class="pa-4 h-100"
                  :selected="applicationId === null"
                  @click="applicationId = null"
                >
                  <div class="d-flex align-center ga-2 font-weight-bold">
                    <v-icon color="primary">mdi-plus-circle</v-icon> Nova aplicação
                  </div>
                  <div class="text-body-2 text-high-emphasis">Outro repositório do GitHub</div>
                </ChoiceCard>
              </v-col>
              <v-col v-for="existing in deploys.applications" :key="existing.id" cols="12" sm="6">
                <ChoiceCard
                  class="pa-4 h-100"
                  :selected="applicationId === existing.id"
                  @click="chooseApplication(existing.id)"
                >
                  <div class="d-flex align-center ga-2 font-weight-bold">
                    <v-icon color="primary">mdi-github</v-icon> {{ existing.name }}
                  </div>
                  <div class="text-body-2 text-high-emphasis">
                    {{ existing.repository }} · {{ existing.targets.length }} alvo(s)
                  </div>
                </ChoiceCard>
              </v-col>
            </v-row>
          </template>
          <DeployApplicationFields
            v-if="applicationId === null"
            v-model="application"
            @manage-credentials="credentialsOpen = true"
          />
        </v-form>

        <!-- 2. Onde roda -->
        <v-form v-show="step === 2" ref="targetForm" autocomplete="off" @submit.prevent>
          <DeployTargetFields
            v-model="target"
            :lock-agent="Boolean(preset?.probeId)"
            :lock-project="Boolean(preset?.project)"
          />
          <v-card border flat class="pa-4 mt-4">
            <DeployReadinessList
              :readiness="readiness"
              :loading="readinessLoading"
              :error="readinessError"
              @refresh="checkReadiness"
            />
          </v-card>
        </v-form>

        <!-- 3. Variáveis -->
        <div v-show="step === 3">
          <div class="text-body-1 mb-2">
            Cole o <code>.env</code> de produção deste alvo. Ele fica cifrado na central e é gravado
            no servidor a cada deploy (com permissão só do dono).
          </div>
          <CodeTextarea
            v-model="env"
            height="min(40vh, 380px)"
            placeholder="APP_KEY=...&#10;DB_PASSWORD=..."
            hint="Opcional agora: dá para editar depois em Variáveis."
            persistent-hint
          />
          <v-alert
            v-if="readiness && !readiness.ready"
            type="warning"
            variant="tonal"
            density="compact"
            class="mt-4"
          >
            A checagem de prontidão ainda tem itens em vermelho. Dá para salvar agora e resolver
            antes do primeiro deploy.
          </v-alert>
        </div>
      </v-card-text>

      <v-card-actions class="px-6 pb-4">
        <v-btn variant="text" @click="cancel">Cancelar</v-btn>
        <v-spacer></v-spacer>
        <v-btn v-if="step > 1" variant="text" color="primary" @click="step -= 1">Voltar</v-btn>
        <v-btn v-if="step < 3" color="primary" variant="flat" @click="next">Continuar</v-btn>
        <v-btn
          v-else
          color="success"
          variant="flat"
          prepend-icon="mdi-check"
          :loading="saving"
          @click="finish"
        >
          Concluir
        </v-btn>
      </v-card-actions>
    </v-card>

    <DeployCredentialsDialog v-model="credentialsOpen" />
  </v-dialog>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import ChoiceCard from '@/components/ChoiceCard.vue'
import CodeTextarea from '@/components/CodeTextarea.vue'
import DeployApplicationFields from '@/components/deploys/DeployApplicationFields.vue'
import DeployCredentialsDialog from '@/components/deploys/DeployCredentialsDialog.vue'
import DeployReadinessList from '@/components/deploys/DeployReadinessList.vue'
import DeployTargetFields from '@/components/deploys/DeployTargetFields.vue'
import type { DeployApplicationInput } from '@/bindings/DeployApplicationInput'
import type { DeployReadinessView } from '@/bindings/DeployReadinessView'
import { useDeploysStore } from '@/stores/deploys'
import {
  cleanApplication,
  emptyApplication,
  emptyTarget,
  toTargetInput,
  type SetupPreset,
  type TargetForm,
} from '@/utils/deployForms'

const props = defineProps<{ modelValue: boolean; preset?: SetupPreset | null }>()
const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  done: [applicationId: number, targetId: number]
}>()

const deploys = useDeploysStore()

const STEP_TITLES = ['De onde vem', 'Onde roda', 'Variáveis']

type FormRef = { validate: () => Promise<{ valid: boolean }> } | null

const step = ref(1)
const applicationId = ref<number | null>(null)
const application = ref<DeployApplicationInput>(emptyApplication())
const target = ref<TargetForm>(emptyTarget())
const env = ref('')
const error = ref<string | null>(null)
const saving = ref(false)
const credentialsOpen = ref(false)
const applicationForm = ref<FormRef>(null)
const targetForm = ref<FormRef>(null)

const readiness = ref<DeployReadinessView | null>(null)
const readinessLoading = ref(false)
const readinessError = ref<string | null>(null)

function reset() {
  step.value = 1
  applicationId.value = null
  application.value = emptyApplication()
  target.value = emptyTarget({
    probeId: props.preset?.probeId ?? null,
    project: props.preset?.project ?? '',
  })
  if (props.preset?.project) {
    application.value.name = props.preset.project
    application.value.slug = props.preset.project
  }
  env.value = ''
  error.value = null
  readiness.value = null
  readinessError.value = null
}

function chooseApplication(id: number) {
  applicationId.value = id
  const chosen = deploys.applications.find((item) => item.id === id)
  // Um segundo alvo da mesma aplicação costuma ser outro ambiente.
  if (chosen && chosen.targets.length) target.value.name = ''
  if (chosen && !target.value.project) target.value.project = chosen.slug
}

async function checkReadiness() {
  if (target.value.probeId === null || !target.value.project) {
    readinessError.value = 'Escolha o servidor e o projeto para checar.'
    return
  }
  readinessLoading.value = true
  readinessError.value = null
  try {
    readiness.value = await deploys.readinessFor(target.value.probeId, target.value.project.trim())
  } catch (err) {
    readiness.value = null
    readinessError.value = err instanceof Error ? err.message : 'Não foi possível checar o servidor'
  } finally {
    readinessLoading.value = false
  }
}

async function next() {
  error.value = null
  if (step.value === 1) {
    if (applicationId.value === null) {
      const result = await applicationForm.value?.validate()
      if (result && !result.valid) return
      if (!target.value.project) target.value.project = application.value.slug
    }
    step.value = 2
    void checkReadiness()
    return
  }
  const result = await targetForm.value?.validate()
  if (result && !result.valid) return
  step.value = 3
}

async function finish() {
  const input = toTargetInput(target.value)
  if (!input) {
    step.value = 2
    return
  }
  saving.value = true
  error.value = null
  try {
    const created = await deploys.setup({
      applicationId: applicationId.value,
      application: applicationId.value === null ? cleanApplication(application.value) : undefined,
      target: input,
      env: env.value.trim() ? env.value : undefined,
    })
    emit('done', created.applicationId, created.targetId)
    emit('update:modelValue', false)
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Não foi possível salvar'
  } finally {
    saving.value = false
  }
}

function cancel() {
  emit('update:modelValue', false)
}

watch(
  () => props.modelValue,
  (visible) => {
    if (visible) {
      reset()
      void deploys.fetchAgents()
    }
  },
  { immediate: true }
)

/** Trocar o servidor ou o projeto refaz a checagem (só no passo 2). */
watch(
  () => [target.value.probeId, target.value.project] as const,
  () => {
    if (step.value === 2) readiness.value = null
  }
)
</script>
