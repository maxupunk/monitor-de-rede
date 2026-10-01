<template>
  <v-form ref="form" :disabled="disabled" @submit.prevent>
    <v-row dense>
      <template v-for="field in fields" :key="field.name">
        <v-col v-if="field.kind === 'objects'" cols="12">
          <SettingsListEditor
            :title="field.label"
            :hint="field.hint"
            :item-schema="field.items ?? {}"
            :model-value="listOf(field.name)"
            :disabled="disabled"
            @update:model-value="(items) => set(field.name, items)"
          />
        </v-col>
        <v-col v-else-if="field.kind === 'list'" cols="12" :md="compact ? 12 : 6">
          <v-combobox
            :model-value="listOf(field.name)"
            :label="field.label"
            :hint="field.hint ?? 'Digite e tecle Enter para incluir'"
            persistent-hint
            multiple
            chips
            closable-chips
            variant="outlined"
            density="comfortable"
            @update:model-value="(items) => set(field.name, items)"
          ></v-combobox>
        </v-col>
        <v-col v-else-if="field.kind === 'boolean'" cols="12" sm="6">
          <v-switch
            :model-value="Boolean(value[field.name])"
            :label="field.label"
            :hint="field.hint"
            :persistent-hint="Boolean(field.hint)"
            :hide-details="!field.hint"
            color="primary"
            inset
            density="comfortable"
            @update:model-value="(checked) => set(field.name, Boolean(checked))"
          ></v-switch>
        </v-col>
        <v-col v-else cols="12" :md="compact ? 12 : 6">
          <v-select
            v-if="field.options"
            :model-value="value[field.name] ?? ''"
            :items="field.options.map((option) => ({ title: optionLabel(option), value: option }))"
            :label="field.label"
            :hint="field.hint"
            :persistent-hint="Boolean(field.hint)"
            :rules="field.rules"
            variant="outlined"
            density="comfortable"
            @update:model-value="(selected) => set(field.name, selected)"
          ></v-select>
          <v-text-field
            v-else
            :model-value="value[field.name] ?? ''"
            :label="field.label + (field.required ? ' *' : '')"
            :hint="secretHint(field) ?? field.hint"
            :persistent-hint="Boolean(secretHint(field) ?? field.hint)"
            :rules="field.rules"
            :type="inputType(field)"
            :autocomplete="field.secret ? 'new-password' : 'off'"
            variant="outlined"
            density="comfortable"
            @update:model-value="(text) => set(field.name, text)"
          ></v-text-field>
        </v-col>
      </template>
    </v-row>
  </v-form>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import {
  SECRET_MASK,
  fieldsOf,
  optionLabel,
  type Schema,
  type SchemaField,
} from '@/utils/pluginSettings'
import SettingsListEditor from './SettingsListEditor.vue'

const props = defineProps<{
  schema: Schema
  modelValue: Record<string, unknown>
  disabled?: boolean
  /** Uma coluna só (diálogos estreitos). */
  compact?: boolean
}>()

const emit = defineEmits<{ 'update:modelValue': [value: Record<string, unknown>] }>()

const form = ref<{ validate: () => Promise<{ valid: boolean }> } | null>(null)
const fields = computed(() => fieldsOf(props.schema))
const value = computed(() => props.modelValue ?? {})

function set(name: string, next: unknown) {
  emit('update:modelValue', { ...value.value, [name]: next })
}

function listOf(name: string): unknown[] {
  const current = value.value[name]
  return Array.isArray(current) ? current : []
}

function inputType(field: SchemaField): string {
  if (field.secret) return 'password'
  return field.kind === 'integer' || field.kind === 'number' ? 'number' : 'text'
}

function secretHint(field: SchemaField): string | undefined {
  if (!field.secret) return undefined
  return value.value[field.name] === SECRET_MASK
    ? 'Guardada cifrada. Deixe como está para manter, ou digite uma nova.'
    : 'Guardada cifrada; nunca volta para a tela.'
}

async function validate(): Promise<boolean> {
  const result = await form.value?.validate()
  return result?.valid ?? true
}

defineExpose({ validate })
</script>
