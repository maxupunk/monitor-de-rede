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

        <v-form ref="form" @submit.prevent="submit">
          <template v-for="field in fields" :key="field.name">
            <v-select
              v-if="field.options"
              v-model="values[field.name]"
              :items="field.options"
              :label="field.label"
              :hint="field.hint"
              :rules="field.rules"
              persistent-hint
              variant="outlined"
              density="comfortable"
              class="mb-3"
            ></v-select>
            <v-switch
              v-else-if="field.kind === 'boolean'"
              v-model="values[field.name]"
              :label="field.label"
              :hint="field.hint"
              persistent-hint
              color="primary"
              inset
              class="mb-3"
            ></v-switch>
            <v-text-field
              v-else
              v-model="values[field.name]"
              :label="field.label"
              :hint="field.hint"
              :rules="field.rules"
              :type="field.kind === 'string' ? 'text' : 'number'"
              persistent-hint
              variant="outlined"
              density="comfortable"
              class="mb-3"
            ></v-text-field>
          </template>
        </v-form>

        <v-alert v-if="!active" type="warning" variant="tonal" density="compact" class="mb-3">
          Este plugin ainda não está ativo: cada acesso ao equipamento vai pedir sua aprovação.
        </v-alert>

        <template v-if="action.effect === 'write'">
          <v-alert type="error" variant="tonal" density="compact" class="mb-2">
            Esta ação <strong>altera a configuração do equipamento</strong>. Confira os parâmetros e
            tenha um backup da configuração antes de continuar.
          </v-alert>
          <v-checkbox
            v-model="confirmWrite"
            color="error"
            density="compact"
            hide-details
            label="Entendo que esta ação altera o equipamento"
          ></v-checkbox>
        </template>
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
import { computed, reactive, ref, watch } from 'vue'
import type { PluginAction } from '@/bindings/PluginAction'
import { effectPresentation } from '@/utils/pluginPresentation'

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

type Rule = (value: unknown) => boolean | string

interface Field {
  name: string
  kind: 'string' | 'integer' | 'number' | 'boolean'
  label: string
  hint?: string
  options?: unknown[]
  rules: Rule[]
}

const form = ref<{ validate: () => Promise<{ valid: boolean }> } | null>(null)
const values = reactive<Record<string, unknown>>({})
const confirmWrite = ref(false)

const effect = computed(() => effectPresentation(props.action?.effect ?? 'read'))

function schemaOf(action: PluginAction | null): Record<string, Record<string, unknown>> {
  const properties = action?.params?.properties
  return typeof properties === 'object' && properties !== null
    ? (properties as Record<string, Record<string, unknown>>)
    : {}
}

const required = computed<string[]>(() => {
  const list = props.action?.params?.required
  return Array.isArray(list) ? list.filter((item): item is string => typeof item === 'string') : []
})

const fields = computed<Field[]>(() =>
  Object.entries(schemaOf(props.action)).map(([name, schema]) => {
    const kind = (
      ['integer', 'number', 'boolean'].includes(String(schema.type)) ? schema.type : 'string'
    ) as Field['kind']
    const rules: Rule[] = []
    if (required.value.includes(name)) {
      rules.push(
        (value) => (value !== '' && value !== null && value !== undefined) || 'Obrigatório'
      )
    }
    if (typeof schema.pattern === 'string') {
      const pattern = new RegExp(`^(?:${schema.pattern})$`)
      rules.push((value) => value === '' || pattern.test(String(value ?? '')) || 'Formato inválido')
    }
    if (typeof schema.maxLength === 'number') {
      const max = schema.maxLength
      rules.push((value) => String(value ?? '').length <= max || `No máximo ${max} caracteres`)
    }
    return {
      name,
      kind,
      label: typeof schema.title === 'string' ? schema.title : name,
      hint: typeof schema.description === 'string' ? schema.description : undefined,
      options: Array.isArray(schema.enum) ? schema.enum : undefined,
      rules,
    }
  })
)

watch(
  () => [props.modelValue, props.action] as const,
  ([open]) => {
    if (!open) return
    confirmWrite.value = false
    for (const key of Object.keys(values)) delete values[key]
    for (const [name, schema] of Object.entries(schemaOf(props.action))) {
      values[name] =
        props.initialParams?.[name] ?? schema.default ?? (schema.type === 'boolean' ? false : '')
    }
  },
  { immediate: true }
)

function close(value = false) {
  emit('update:modelValue', value)
}

async function submit() {
  const result = await form.value?.validate()
  if (result && !result.valid) return
  const params: Record<string, unknown> = {}
  for (const field of fields.value) {
    const value = values[field.name]
    if (value === '' || value === null || value === undefined) continue
    params[field.name] = field.kind === 'integer' || field.kind === 'number' ? Number(value) : value
  }
  emit('run', params, confirmWrite.value)
  close(false)
}
</script>
