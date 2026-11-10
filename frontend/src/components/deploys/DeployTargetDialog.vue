<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 860"
    :fullscreen="$vuetify.display.xs"
    scrollable
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card class="rounded-lg">
      <v-card-title class="font-weight-bold pt-4 px-6">
        <v-icon start color="primary">mdi-server-network</v-icon>
        {{ target ? `Editar alvo — ${target.name}` : `Novo alvo — ${application?.name ?? ''}` }}
      </v-card-title>
      <v-card-text class="px-6">
        <v-alert v-if="error" type="error" variant="tonal" class="mb-4">{{ error }}</v-alert>
        <v-form ref="formRef" autocomplete="off" @submit.prevent="save">
          <DeployTargetFields
            v-model="form"
            :references="references"
            :references-loading="referencesLoading"
            :compiles="application?.builder === 'netmonitor'"
          />
        </v-form>
        <v-card border flat class="pa-4 mt-4">
          <DeployReadinessList
            :readiness="readiness"
            :loading="readinessLoading"
            :error="readinessError"
            @refresh="checkReadiness"
          />
        </v-card>
      </v-card-text>
      <v-card-actions class="px-6 pb-4">
        <v-spacer></v-spacer>
        <v-btn variant="text" @click="emit('update:modelValue', false)">Cancelar</v-btn>
        <v-btn color="primary" variant="flat" :loading="saving" @click="save">Salvar</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import DeployReadinessList from '@/components/deploys/DeployReadinessList.vue'
import DeployTargetFields from '@/components/deploys/DeployTargetFields.vue'
import type { DeployApplicationView } from '@/bindings/DeployApplicationView'
import type { DeployReadinessView } from '@/bindings/DeployReadinessView'
import type { DeployTargetView } from '@/bindings/DeployTargetView'
import { useRepositoryReferences } from '@/composables/useRepositoryReferences'
import { useDeploysStore } from '@/stores/deploys'
import { emptyTarget, targetFromView, toTargetInput, type TargetForm } from '@/utils/deployForms'

const props = defineProps<{
  modelValue: boolean
  application: DeployApplicationView | null
  /** Editar este; sem ele, criar um novo na aplicação. */
  target: DeployTargetView | null
}>()
const emit = defineEmits<{ 'update:modelValue': [value: boolean]; saved: [] }>()

const deploys = useDeploysStore()
const form = ref<TargetForm>(emptyTarget())
const formRef = ref<{ validate: () => Promise<{ valid: boolean }> } | null>(null)
const saving = ref(false)
const error = ref<string | null>(null)
const readiness = ref<DeployReadinessView | null>(null)
const readinessLoading = ref(false)
const readinessError = ref<string | null>(null)
const { references, loading: referencesLoading, load: loadReferences } = useRepositoryReferences()

async function checkReadiness() {
  readinessLoading.value = true
  readinessError.value = null
  try {
    readiness.value = props.target
      ? await deploys.targetReadiness(props.target.id)
      : form.value.probeId !== null && form.value.project
        ? await deploys.readinessFor(form.value.probeId, form.value.project.trim())
        : null
    if (!readiness.value) readinessError.value = 'Escolha o servidor e o projeto para checar.'
  } catch (err) {
    readiness.value = null
    readinessError.value = err instanceof Error ? err.message : 'Não foi possível checar'
  } finally {
    readinessLoading.value = false
  }
}

watch(
  () => props.modelValue,
  (visible) => {
    if (!visible) return
    error.value = null
    readiness.value = null
    readinessError.value = null
    const application = props.application
    form.value = props.target
      ? targetFromView(props.target)
      : emptyTarget({
          name: application?.targets.length ? '' : 'Produção',
          project: application?.slug ?? '',
          position: application?.targets.length ?? 0,
        })
    if (props.target) void checkReadiness()
    if (application) {
      void loadReferences(application.repository, {
        connectionId: application.githubConnectionId,
        credentialId: application.credentialId,
      })
    }
  },
  { immediate: true }
)

async function save() {
  const application = props.application
  if (!application) return
  const result = await formRef.value?.validate()
  const input = toTargetInput(form.value)
  if ((result && !result.valid) || !input) return
  saving.value = true
  error.value = null
  try {
    await deploys.saveTarget(application.id, props.target?.id ?? null, input)
    emit('saved')
    emit('update:modelValue', false)
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Não foi possível salvar o alvo'
  } finally {
    saving.value = false
  }
}
</script>
