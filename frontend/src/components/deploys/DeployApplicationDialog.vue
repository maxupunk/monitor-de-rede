<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 820"
    :fullscreen="$vuetify.display.xs"
    scrollable
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card class="rounded-lg">
      <v-card-title class="font-weight-bold pt-4 px-6">
        <v-icon start color="primary">mdi-github</v-icon>
        Editar aplicação
      </v-card-title>
      <v-card-text class="px-6">
        <v-alert v-if="error" type="error" variant="tonal" class="mb-4">{{ error }}</v-alert>
        <v-form ref="formRef" autocomplete="off" @submit.prevent="save">
          <DeployApplicationFields
            v-model="form"
            editing
            @manage-credentials="credentialsOpen = true"
          />
        </v-form>
      </v-card-text>
      <v-card-actions class="px-6 pb-4">
        <v-spacer></v-spacer>
        <v-btn variant="text" @click="emit('update:modelValue', false)">Cancelar</v-btn>
        <v-btn color="primary" variant="flat" :loading="saving" @click="save">Salvar</v-btn>
      </v-card-actions>
    </v-card>
    <DeployCredentialsDialog v-model="credentialsOpen" />
  </v-dialog>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import DeployApplicationFields from '@/components/deploys/DeployApplicationFields.vue'
import DeployCredentialsDialog from '@/components/deploys/DeployCredentialsDialog.vue'
import type { DeployApplicationInput } from '@/bindings/DeployApplicationInput'
import type { DeployApplicationView } from '@/bindings/DeployApplicationView'
import { useDeploysStore } from '@/stores/deploys'
import { applicationFromView, cleanApplication, emptyApplication } from '@/utils/deployForms'

const props = defineProps<{ modelValue: boolean; application: DeployApplicationView | null }>()
const emit = defineEmits<{ 'update:modelValue': [value: boolean]; saved: [] }>()

const deploys = useDeploysStore()
const form = ref<DeployApplicationInput>(emptyApplication())
const formRef = ref<{ validate: () => Promise<{ valid: boolean }> } | null>(null)
const saving = ref(false)
const error = ref<string | null>(null)
const credentialsOpen = ref(false)

watch(
  () => props.modelValue,
  (visible) => {
    if (!visible || !props.application) return
    form.value = applicationFromView(props.application)
    error.value = null
  },
  { immediate: true }
)

async function save() {
  if (!props.application) return
  const result = await formRef.value?.validate()
  if (result && !result.valid) return
  saving.value = true
  error.value = null
  try {
    await deploys.saveApplication(props.application.id, cleanApplication(form.value))
    emit('saved')
    emit('update:modelValue', false)
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Não foi possível salvar'
  } finally {
    saving.value = false
  }
}
</script>
