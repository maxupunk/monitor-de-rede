<template>
  <div>
    <div class="d-flex align-center mb-2">
      <div class="text-title-small font-weight-bold flex-grow-1">Parâmetros</div>
      <v-btn
        v-if="fields.length > 0"
        size="small"
        variant="text"
        color="primary"
        :prepend-icon="asJson ? 'mdi-form-select' : 'mdi-code-json'"
        @click="toggleJson"
      >
        {{ asJson ? 'Formulário' : 'Editar como JSON' }}
      </v-btn>
    </div>

    <p v-if="fields.length === 0" class="hint text-body-medium mb-0">
      Esta ferramenta não recebe parâmetros.
    </p>

    <template v-else-if="asJson">
      <v-textarea
        v-model="jsonText"
        variant="outlined"
        rows="5"
        auto-grow
        class="mono"
        :error-messages="jsonError ?? undefined"
        aria-label="Parâmetros em JSON"
        @update:model-value="applyJson"
      ></v-textarea>
    </template>

    <v-row v-else dense>
      <v-col
        v-for="field in fields"
        :key="field.key"
        cols="12"
        :sm="field.kind === 'boolean' ? 12 : 6"
      >
        <v-switch
          v-if="field.kind === 'boolean'"
          :model-value="Boolean(values[field.key])"
          :label="field.label"
          :hint="field.hint"
          :persistent-hint="Boolean(field.hint)"
          color="primary"
          inset
          @update:model-value="(value) => set(field.key, Boolean(value))"
        ></v-switch>
        <v-select
          v-else-if="field.kind === 'select'"
          :model-value="textValue(field.key)"
          :items="field.options"
          :label="label(field)"
          :hint="field.hint"
          :persistent-hint="Boolean(field.hint)"
          :clearable="!field.required"
          variant="outlined"
          density="comfortable"
          @update:model-value="(value) => set(field.key, value)"
        ></v-select>
        <v-combobox
          v-else-if="field.kind === 'list'"
          :model-value="listValue(field.key)"
          :label="label(field)"
          :hint="field.hint ?? 'Digite e tecle Enter para cada item'"
          persistent-hint
          multiple
          chips
          closable-chips
          variant="outlined"
          density="comfortable"
          @update:model-value="(value) => set(field.key, value)"
        ></v-combobox>
        <v-combobox
          v-else-if="field.device"
          :model-value="textValue(field.key)"
          :items="deviceOptions"
          :label="label(field)"
          :hint="field.hint ?? 'Nome, IP ou id do equipamento'"
          persistent-hint
          clearable
          variant="outlined"
          density="comfortable"
          @update:model-value="(value) => set(field.key, value)"
        ></v-combobox>
        <v-text-field
          v-else
          :model-value="values[field.key] ?? ''"
          :label="label(field)"
          :hint="field.hint"
          :persistent-hint="Boolean(field.hint)"
          :type="field.kind === 'integer' || field.kind === 'number' ? 'number' : 'text'"
          variant="outlined"
          density="comfortable"
          @update:model-value="(value) => set(field.key, value)"
        ></v-text-field>
      </v-col>
    </v-row>
  </div>
</template>

<script setup lang="ts">
/**
 * Os argumentos da ferramenta, num formulário montado a partir do JSON
 * Schema dela — com "Editar como JSON" para quem prefere. O `v-model` é o
 * objeto já no tipo do contrato, sem os campos em branco.
 */
import { computed, onMounted, ref, watch } from 'vue'
import { useDevicesStore } from '@/stores/devices'
import { buildArguments, schemaFields, type SchemaField } from '@/utils/scheduledTasks/schemaForm'

const props = defineProps<{
  schema: Record<string, unknown>
  modelValue: Record<string, unknown>
}>()
const emit = defineEmits<{ 'update:modelValue': [value: Record<string, unknown>] }>()

const devices = useDevicesStore()

const fields = computed(() => schemaFields(props.schema))
const values = ref<Record<string, unknown>>({ ...props.modelValue })
const asJson = ref(false)
const jsonText = ref('')
const jsonError = ref<string | null>(null)

const deviceOptions = computed(() =>
  devices.devices.map((device) => device.name).filter((name) => name.trim() !== '')
)

function same(a: unknown, b: unknown): boolean {
  return JSON.stringify(a) === JSON.stringify(b)
}

// Outra ferramenta ou outra tarefa: o formulário recomeça do `v-model`.
watch(
  () => props.modelValue,
  (value) => {
    if (!same(value, buildArguments(fields.value, values.value))) {
      values.value = { ...value }
      jsonText.value = JSON.stringify(value, null, 2)
    }
  }
)

function label(field: SchemaField): string {
  return field.required ? `${field.label} *` : field.label
}

function textValue(key: string): string | null {
  const value = values.value[key]
  return value === undefined || value === null ? null : String(value)
}

function listValue(key: string): unknown[] {
  const value = values.value[key]
  return Array.isArray(value) ? value : []
}

function set(key: string, value: unknown) {
  values.value = { ...values.value, [key]: value }
  emit('update:modelValue', buildArguments(fields.value, values.value))
}

function toggleJson() {
  if (!asJson.value) {
    jsonText.value = JSON.stringify(buildArguments(fields.value, values.value), null, 2)
    jsonError.value = null
  }
  asJson.value = !asJson.value
}

function applyJson(text: string) {
  try {
    const parsed: unknown = JSON.parse(text || '{}')
    if (parsed === null || typeof parsed !== 'object' || Array.isArray(parsed)) {
      jsonError.value = 'Os parâmetros precisam ser um objeto JSON: { "chave": valor }'
      return
    }
    jsonError.value = null
    values.value = { ...(parsed as Record<string, unknown>) }
    emit('update:modelValue', parsed as Record<string, unknown>)
  } catch {
    jsonError.value = 'JSON inválido'
  }
}

onMounted(() => {
  if (devices.devices.length === 0) void devices.fetchDevices()
})
</script>

<style scoped>
.hint {
  color: rgb(var(--v-theme-on-surface));
}
.mono :deep(textarea) {
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
}
</style>
