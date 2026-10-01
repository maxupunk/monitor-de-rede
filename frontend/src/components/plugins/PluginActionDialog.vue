<template>
  <v-dialog :model-value="modelValue" max-width="560" @update:model-value="close">
    <v-card v-if="action" class="rounded-lg">
      <v-card-title class="d-flex align-center ga-2">
        <v-icon :color="effect.color">{{ effect.icon }}</v-icon>
        {{ action.title }}
      </v-card-title>
      <v-card-subtitle>{{ pluginName }} · {{ deviceName }}</v-card-subtitle>

      <v-card-text>
        <p v-if="action.description" class="text-body-2 mb-4">{{ action.description }}</p>

        <SettingsForm
          v-if="fields.length > 0"
          ref="form"
          v-model="values"
          :schema="action.params ?? {}"
          compact
        />

        <v-alert v-if="!active" type="warning" variant="tonal" density="compact" class="mb-3">
          Este plugin ainda não está ativo: cada acesso ao equipamento vai pedir sua aprovação.
        </v-alert>

        <WriteConfirm v-if="action.effect === 'write'" v-model="confirmWrite" />
      </v-card-text>

      <v-card-actions>
        <v-spacer></v-spacer>
        <v-btn variant="text" color="secondary" @click="close(false)">Cancelar</v-btn>
        <v-btn
          :color="action.effect === 'write' ? 'error' : 'primary'"
          variant="flat"
          prepend-icon="mdi-play"
          :disabled="action.effect === 'write' && !confirmWrite"
          @click="submit"
        >
          Executar
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { PluginAction } from '@/bindings/PluginAction'
import { effectPresentation } from '@/utils/pluginPresentation'
import { defaultsOf, fieldsOf, paramsOf } from '@/utils/pluginSettings'
import SettingsForm from './settings/SettingsForm.vue'
import WriteConfirm from './WriteConfirm.vue'

const props = defineProps<{
  modelValue: boolean
  action: PluginAction | null
  pluginName: string
  deviceName: string
  active: boolean
  /** Valores já preenchidos (ex.: o pacote da linha escolhida no painel). */
  initialParams?: Record<string, unknown>
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  run: [params: Record<string, unknown>, confirmWrite: boolean]
}>()

const form = ref<{ validate: () => Promise<boolean> } | null>(null)
const values = ref<Record<string, unknown>>({})
const confirmWrite = ref(false)

const effect = computed(() => effectPresentation(props.action?.effect ?? 'read'))
const fields = computed(() => fieldsOf(props.action?.params))

watch(
  () => [props.modelValue, props.action] as const,
  ([open]) => {
    if (!open) return
    confirmWrite.value = false
    values.value = { ...defaultsOf(props.action?.params), ...(props.initialParams ?? {}) }
  },
  { immediate: true }
)

function close(value = false) {
  emit('update:modelValue', value)
}

async function submit() {
  if (form.value && !(await form.value.validate())) return
  const params = paramsOf(props.action?.params, values.value)
  emit('run', params, confirmWrite.value)
  close(false)
}
</script>
